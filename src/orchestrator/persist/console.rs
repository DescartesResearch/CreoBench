use std::io::{BufWriter, Write};

use crate::tracker::IntervalReport;

use super::{PersistError, Persister, Phase};

pub struct ConsolePersister<W: Write> {
    writer: BufWriter<W>,
    warmup_duration: u32,
    warmup_pause: u32,
}

impl<W: Write> ConsolePersister<W> {
    pub fn new(writer: W, warmup_duration: u32, warmup_pause: u32) -> Self {
        Self {
            writer: BufWriter::new(writer),
            warmup_duration,
            warmup_pause,
        }
    }

    pub fn format_round(&self, report: &IntervalReport, phase: Phase) -> String {
        let target_secs = report.target_time.as_secs_f64();
        let display_target = match phase {
            Phase::Warmup => {
                target_secs - (f64::from(self.warmup_duration) + f64::from(self.warmup_pause))
            }
            Phase::Pause => target_secs - f64::from(self.warmup_pause),
            Phase::Load => target_secs,
        };
        let stats = &report.stats;
        format!(
            "TARGET={:.1}s; LOAD={}; #SUCC={}; #FAIL={}; #TO={}; #DROP={}; AVG ST={}ms",
            display_target,
            report.load_level,
            stats.successful_transactions,
            stats.failed_transactions,
            stats.timeout_transactions,
            stats.dropped_transactions,
            stats.average_response_time(),
        )
    }
}

#[cfg(test)]
impl<W: Write + std::fmt::Debug> ConsolePersister<W> {
    fn into_inner(self) -> W {
        self.writer
            .into_inner()
            .expect("in-memory console writer must flush")
    }
}

impl<W: Write + Send> Persister for ConsolePersister<W> {
    fn persist(&mut self, report: &IntervalReport, phase: Phase) -> Result<(), PersistError> {
        let _ = writeln!(self.writer, "{}", self.format_round(report, phase));
        Ok(())
    }

    fn flush(&mut self) -> Result<(), PersistError> {
        let _ = self.writer.flush();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::load::RelativeLoadTestTime;
    use crate::orchestrator::persist::test_utils::stats_with_measured_transactions;
    use crate::tracker::{IntervalReport, IntervalStatistics};

    #[test]
    fn format_round_warmup_round_with_traffic_produces_negative_target() {
        let stats = stats_with_measured_transactions(&[100, 100, 100], &[]);
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(1)),
            10,
            stats,
            Some(RelativeLoadTestTime::new(Duration::from_secs(1))),
        );

        let persister = ConsolePersister::new(Vec::new(), 5, 2);
        let rendered = persister.format_round(&report, Phase::Warmup);

        assert_eq!(
            rendered,
            "TARGET=-6.0s; LOAD=10; #SUCC=3; #FAIL=0; #TO=0; #DROP=0; AVG ST=100ms"
        );
    }

    #[test]
    fn format_round_warmup_pause_round_has_zero_counts_and_zero_avg_rt() {
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(6)),
            0,
            IntervalStatistics::default(),
            None,
        );

        let persister = ConsolePersister::new(Vec::new(), 5, 2);
        let rendered = persister.format_round(&report, Phase::Pause);

        assert_eq!(
            rendered,
            "TARGET=4.0s; LOAD=0; #SUCC=0; #FAIL=0; #TO=0; #DROP=0; AVG ST=0ms"
        );
    }

    #[test]
    fn format_round_load_round_with_mixed_counts() {
        let stats = stats_with_measured_transactions(&[100, 100], &[100]);
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(8)),
            100,
            stats,
            Some(RelativeLoadTestTime::new(Duration::from_secs(8))),
        );

        let persister = ConsolePersister::new(Vec::new(), 5, 2);
        let rendered = persister.format_round(&report, Phase::Load);

        assert_eq!(
            rendered,
            "TARGET=8.0s; LOAD=100; #SUCC=2; #FAIL=1; #TO=0; #DROP=0; AVG ST=100ms"
        );
    }

    #[test]
    fn format_round_empty_avg_case_renders_zero_milliseconds() {
        let stats = IntervalStatistics::default();
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(8)),
            100,
            stats,
            Some(RelativeLoadTestTime::new(Duration::from_secs(8))),
        );

        let persister = ConsolePersister::new(Vec::new(), 5, 2);
        let rendered = persister.format_round(&report, Phase::Load);

        assert_eq!(
            rendered,
            "TARGET=8.0s; LOAD=100; #SUCC=0; #FAIL=0; #TO=0; #DROP=0; AVG ST=0ms"
        );
    }

    #[test]
    fn console_persister_emits_line_through_injected_writer() {
        let stats = stats_with_measured_transactions(&[100, 100], &[]);
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(8)),
            100,
            stats,
            Some(RelativeLoadTestTime::new(Duration::from_secs(8))),
        );

        let mut persister = ConsolePersister::new(Vec::new(), 5, 2);
        persister.persist(&report, Phase::Load).unwrap();

        let output = String::from_utf8(persister.into_inner()).unwrap();
        assert_eq!(
            output,
            "TARGET=8.0s; LOAD=100; #SUCC=2; #FAIL=0; #TO=0; #DROP=0; AVG ST=100ms\n"
        );
    }
}
