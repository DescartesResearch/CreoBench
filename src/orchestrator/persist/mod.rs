pub mod console;
mod csv;
pub mod interval;
#[cfg(test)]
mod test_utils;
pub mod transactions;
pub use console::ConsolePersister;
pub use interval::IntervalCsvPersister;
pub use transactions::TransactionCsvPersister;

use crate::tracker::IntervalReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Warmup,
    Pause,
    Load,
}

pub struct PhaseClassifier {
    warmup_duration: u64,
    warmup_pause: u64,
    count: u64,
}

impl PhaseClassifier {
    pub fn new(warmup_duration: u32, warmup_pause: u32) -> Self {
        Self {
            warmup_duration: u64::from(warmup_duration),
            warmup_pause: u64::from(warmup_pause),
            count: 0,
        }
    }

    pub fn classify(&mut self) -> Phase {
        let phase = if self.count < self.warmup_duration {
            Phase::Warmup
        } else if self.count < self.warmup_duration + self.warmup_pause {
            Phase::Pause
        } else {
            Phase::Load
        };
        self.count += 1;
        phase
    }
}

#[derive(Debug, thiserror::Error)]
#[error("persister `{name}` failed: {source}")]
pub struct PersistError {
    pub name: &'static str,
    pub source: std::io::Error,
}

pub trait Persister: Send {
    fn persist(&mut self, report: &IntervalReport, phase: Phase) -> Result<(), PersistError>;

    /// Provided hook the writer task calls after each report and once more
    /// when the channel closes. Persisters buffering their writes
    /// (e.g. `BufWriter`) override this to push bytes to the underlying
    /// writer; the default no-op keeps infallible persisters trivial.
    fn flush(&mut self) -> Result<(), PersistError> {
        Ok(())
    }
}

pub async fn writer_loop(
    mut receiver: tokio::sync::mpsc::Receiver<IntervalReport>,
    mut classifier: PhaseClassifier,
    mut persisters: Vec<Box<dyn Persister>>,
    error_tx: tokio::sync::oneshot::Sender<PersistError>,
) {
    while let Some(report) = receiver.recv().await {
        let phase = classifier.classify();
        for persister in &mut persisters {
            if let Err(err) = persister.persist(&report, phase) {
                let _ = error_tx.send(err);
                return;
            }
        }
        for persister in &mut persisters {
            if let Err(err) = persister.flush() {
                let _ = error_tx.send(err);
                return;
            }
        }
    }
    for persister in &mut persisters {
        if let Err(err) = persister.flush() {
            let _ = error_tx.send(err);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::{mpsc, oneshot};

    use super::*;
    use crate::load::RelativeLoadTestTime;
    use crate::orchestrator::persist::test_utils::{
        dropped_result, failed_result, stats_with, success_result, timeout_result,
    };
    use crate::tracker::{IntervalReport, IntervalStatistics};
    use crate::transaction::{DroppedCode, FailedCode, TimeoutCode, TransactionResult};

    fn measured_report(target_secs: u64, load_level: u32) -> IntervalReport {
        IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(target_secs)),
            load_level,
            IntervalStatistics::default(),
            Some(RelativeLoadTestTime::new(Duration::from_secs(target_secs))),
        )
    }

    fn idle_report(target_secs: u64) -> IntervalReport {
        IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(target_secs)),
            0,
            IntervalStatistics::default(),
            None,
        )
    }

    fn results_report(
        target_secs: u64,
        load_level: u32,
        results: Vec<TransactionResult>,
    ) -> IntervalReport {
        IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(target_secs)),
            load_level,
            stats_with(&results),
            Some(RelativeLoadTestTime::new(Duration::from_secs(target_secs))),
        )
    }

    fn drain_report(target_secs: u64) -> IntervalReport {
        IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(target_secs)),
            0,
            stats_with(&[dropped_result(
                3,
                1900,
                target_secs * 1000,
                DroppedCode::Error("batch behind, dropping".into()),
            )]),
            None,
        )
    }

    #[test]
    fn first_rounds_before_warmup_duration_are_warmup() {
        let mut classifier = PhaseClassifier::new(3, 2);
        assert_eq!(classifier.classify(), Phase::Warmup);
        assert_eq!(classifier.classify(), Phase::Warmup);
        assert_eq!(classifier.classify(), Phase::Warmup);
    }

    #[test]
    fn round_at_warmup_duration_boundary_is_pause() {
        let mut classifier = PhaseClassifier::new(3, 2);
        for _ in 0..3 {
            classifier.classify();
        }
        assert_eq!(classifier.classify(), Phase::Pause);
    }

    #[test]
    fn rounds_within_warmup_pause_are_pause() {
        let mut classifier = PhaseClassifier::new(3, 2);
        for _ in 0..3 {
            classifier.classify();
        }
        assert_eq!(classifier.classify(), Phase::Pause);
        assert_eq!(classifier.classify(), Phase::Pause);
    }

    #[test]
    fn round_at_warmup_plus_pause_boundary_is_load() {
        let mut classifier = PhaseClassifier::new(3, 2);
        for _ in 0..5 {
            classifier.classify();
        }
        assert_eq!(classifier.classify(), Phase::Load);
    }

    #[test]
    fn zero_duration_warmup_classifies_all_rounds_as_load() {
        let mut classifier = PhaseClassifier::new(0, 0);
        assert_eq!(classifier.classify(), Phase::Load);
        assert_eq!(classifier.classify(), Phase::Load);
    }

    #[test]
    fn zero_pause_skips_the_pause_phase() {
        let mut classifier = PhaseClassifier::new(2, 0);
        assert_eq!(classifier.classify(), Phase::Warmup);
        assert_eq!(classifier.classify(), Phase::Warmup);
        assert_eq!(classifier.classify(), Phase::Load);
    }

    struct FailingPersister;

    impl Persister for FailingPersister {
        fn persist(&mut self, _report: &IntervalReport, _phase: Phase) -> Result<(), PersistError> {
            Err(PersistError {
                name: "stub",
                source: std::io::Error::other("disk full"),
            })
        }
    }

    struct LateFlushFailingPersister {
        flushes: u8,
    }

    impl Persister for LateFlushFailingPersister {
        fn persist(&mut self, _report: &IntervalReport, _phase: Phase) -> Result<(), PersistError> {
            Ok(())
        }

        fn flush(&mut self) -> Result<(), PersistError> {
            self.flushes += 1;
            if self.flushes > 1 {
                Err(PersistError {
                    name: "stub",
                    source: std::io::Error::other("disk full"),
                })
            } else {
                Ok(())
            }
        }
    }

    #[ignore = "refactor"]
    #[tokio::test]
    async fn writer_task_renders_scripted_stream_spanning_all_phases() {
        let dir = std::env::temp_dir().join(format!("creo-persist-writer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = std::fs::File::create(dir.join("console.txt")).unwrap();
        let interval_file = std::fs::File::create(dir.join("interval.csv")).unwrap();
        let transactions_file = std::fs::File::create(dir.join("transactions.csv")).unwrap();

        let (report_tx, report_rx) = mpsc::channel(2);
        let (error_tx, error_rx) = oneshot::channel();
        let persisters: Vec<Box<dyn Persister>> = vec![
            Box::new(ConsolePersister::new(file, 2, 1)),
            Box::new(IntervalCsvPersister::new(interval_file).unwrap()),
            Box::new(TransactionCsvPersister::new(transactions_file).unwrap()),
        ];
        let task = tokio::spawn(writer_loop(
            report_rx,
            PhaseClassifier::new(2, 1),
            persisters,
            error_tx,
        ));

        let stream = vec![
            measured_report(1, 10),
            measured_report(2, 10),
            idle_report(1),
            results_report(
                1,
                100,
                vec![
                    success_result(0, 0, 250, 1000, 100),
                    failed_result(1, 1, 500, 1000, 200, FailedCode::Status(503)),
                    timeout_result(
                        2,
                        2,
                        750,
                        1000,
                        300,
                        TimeoutCode::Error("deadline exceeded".into()),
                    ),
                ],
            ),
            drain_report(2),
        ];
        for report in stream {
            report_tx.send(report).await.unwrap();
        }
        drop(report_tx);
        task.await.expect("writer task panicked");
        assert!(error_rx.await.is_err(), "no persister error expected");

        let output = std::fs::read_to_string(dir.join("console.txt")).unwrap();
        assert_eq!(
            output,
            "TARGET=-2.0s; LOAD=10; #SUCC=0; #FAIL=0; #TO=0; #DROP=0; AVG ST=0ms\n\
             TARGET=-1.0s; LOAD=10; #SUCC=0; #FAIL=0; #TO=0; #DROP=0; AVG ST=0ms\n\
             TARGET=0.0s; LOAD=0; #SUCC=0; #FAIL=0; #TO=0; #DROP=0; AVG ST=0ms\n\
             TARGET=1.0s; LOAD=100; #SUCC=1; #FAIL=1; #TO=1; #DROP=0; AVG ST=200ms\n\
             TARGET=2.0s; LOAD=0; #SUCC=0; #FAIL=0; #TO=0; #DROP=1; AVG ST=0ms\n"
        );
        let interval = std::fs::read_to_string(dir.join("interval.csv")).unwrap();
        assert_eq!(
            interval,
            "target_time,load_level,successful_transactions,failed_transactions,timeout_transactions,dropped_transactions,avg_service_time,final_batch_time,phase\n\
             1,10,0,0,0,0,0,1,warmup\n\
             2,10,0,0,0,0,0,2,warmup\n\
             1,0,0,0,0,0,0,,pause\n\
             1,100,1,1,1,0,200,1,load\n\
             2,0,0,0,0,1,0,,load\n"
        );
        let transactions = std::fs::read_to_string(dir.join("transactions.csv")).unwrap();
        assert_eq!(
            transactions,
            "target_time,start_time,load_generator_id,virtual_user_id,spec_id,response_time_ms,outcome,reason\n\
             1,0.25,1,0,0,100,success,\n\
             1,0.5,1,1,1,200,failed,Status(503)\n\
             1,0.75,1,2,2,300,timeout,\"Error(\"\"deadline exceeded\"\")\"\n\
             2,1.9,1,3,,,dropped,\"Error(\"\"batch behind, dropping\"\")\"\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn writer_task_signals_persist_error_and_exits() {
        let (report_tx, report_rx) = mpsc::channel(2);
        let (error_tx, error_rx) = oneshot::channel();
        let persisters: Vec<Box<dyn Persister>> = vec![Box::new(FailingPersister)];
        let task = tokio::spawn(writer_loop(
            report_rx,
            PhaseClassifier::new(2, 1),
            persisters,
            error_tx,
        ));

        report_tx.send(measured_report(1, 10)).await.unwrap();

        let err = error_rx.await.expect("writer task must signal the error");
        assert_eq!(err.name, "stub");
        assert_eq!(err.source.kind(), std::io::ErrorKind::Other);

        let outcome = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("task must exit");
        outcome.expect("writer task panicked");
    }

    #[tokio::test]
    async fn writer_task_signals_final_flush_error_and_exits() {
        let (report_tx, report_rx) = mpsc::channel(2);
        let (error_tx, error_rx) = oneshot::channel();
        let persisters: Vec<Box<dyn Persister>> =
            vec![Box::new(LateFlushFailingPersister { flushes: 0 })];
        let task = tokio::spawn(writer_loop(
            report_rx,
            PhaseClassifier::new(2, 1),
            persisters,
            error_tx,
        ));

        report_tx.send(measured_report(1, 10)).await.unwrap();
        drop(report_tx);

        let err = error_rx.await.expect("writer task must signal the error");
        assert_eq!(err.name, "stub");

        let outcome = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("task must exit");
        outcome.expect("writer task panicked");
    }
}
