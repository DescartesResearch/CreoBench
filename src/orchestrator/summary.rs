use crate::orchestrator::persist::{Phase, PhaseClassifier};
use crate::tracker::IntervalReport;
use crate::transaction::{DroppedCode, TransactionResult};

/// Transactions of a phase and how many of them were dropped, by reason.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PhaseDrops {
    pub transactions: u64,
    pub no_virtual_user: u64,
    pub late: u64,
    pub error: u64,
}

/// The dropped transactions of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DropReport {
    pub load: PhaseDrops,
    /// Warmup and pause.
    pub warmup: PhaseDrops,
}

impl PhaseDrops {
    fn total(&self) -> u64 {
        self.no_virtual_user + self.late + self.error
    }

    fn record(&mut self, code: &DroppedCode) {
        match code {
            DroppedCode::NoVirtualUser => self.no_virtual_user += 1,
            DroppedCode::Late => self.late += 1,
            DroppedCode::Error(_) => self.error += 1,
        }
    }

    fn breakdown(&self) -> String {
        [
            (self.no_virtual_user, "without a free virtual user"),
            (self.late, "sent too late"),
            (self.error, "after script errors"),
        ]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, reason)| format!("{count} {reason}"))
        .collect::<Vec<_>>()
        .join(", ")
    }

    fn hints(&self) -> String {
        [
            (
                self.no_virtual_user,
                "increase --virtual-user-count or check whether the system under test slowed down",
            ),
            (
                self.late,
                "give the load generators more CPU or split the load across more instances",
            ),
            (
                self.error,
                "fix the script errors listed in transactions.csv",
            ),
        ]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(_, hint)| hint)
        .collect::<Vec<_>>()
        .join("; ")
    }
}

/// Summarizes dropped transactions over a load test.
///
/// Dropped transactions mean that the load generators did not deliver the
/// scheduled load, so any drop in the load phase invalidates the run.
pub struct DropSummary {
    classifier: PhaseClassifier,
    load: PhaseDrops,
    warmup: PhaseDrops,
}

impl DropSummary {
    pub fn new(warmup_duration: u32, warmup_pause: u32) -> Self {
        Self {
            classifier: PhaseClassifier::new(warmup_duration, warmup_pause),
            load: PhaseDrops::default(),
            warmup: PhaseDrops::default(),
        }
    }

    /// Records the next interval report, in the order the reports arrive.
    pub fn record(&mut self, report: &IntervalReport) {
        let results = report.stats.results();
        let counts = match self.classifier.classify() {
            Phase::Load => &mut self.load,
            Phase::Warmup | Phase::Pause => &mut self.warmup,
        };
        counts.transactions += results.len() as u64;
        for result in results {
            if let TransactionResult::Dropped { code, .. } = result {
                counts.record(code);
            }
        }
    }

    /// Whether no load-phase transaction was dropped.
    pub fn is_valid(&self) -> bool {
        self.load.total() == 0
    }

    /// The dropped transactions, or `None` if no transaction was dropped.
    pub fn report(&self) -> Option<DropReport> {
        (self.load.total() + self.warmup.total() > 0).then_some(DropReport {
            load: self.load,
            warmup: self.warmup,
        })
    }

    /// Logs the summary, warning about every dropped transaction.
    pub fn log(&self) {
        if self.load.total() > 0 {
            tracing::warn!(
                "The run is invalid: {} of {} load-phase transactions were dropped ({}), so the \
                 load generators did not deliver the scheduled load. To fix this, {}.",
                self.load.total(),
                self.load.transactions,
                self.load.breakdown(),
                self.load.hints(),
            );
        }
        if self.warmup.total() > 0 {
            tracing::warn!(
                "Dropped {} of {} warmup transactions ({}), so the warmup may not have prepared \
                 the system under test as intended. To fix this, {}.",
                self.warmup.total(),
                self.warmup.transactions,
                self.warmup.breakdown(),
                self.warmup.hints(),
            );
        }
        if self.load.total() == 0 && self.warmup.total() == 0 {
            tracing::info!("No transactions were dropped.");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::load::RelativeLoadTestTime;
    use crate::orchestrator::persist::test_utils::{dropped_result, stats_with, success_result};
    use crate::test_utils::prelude::LogCapture;

    fn report(results: Vec<TransactionResult>) -> IntervalReport {
        IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(1)),
            10,
            stats_with(&results),
            None,
        )
    }

    fn logged_summary(reports: Vec<IntervalReport>) -> String {
        let logs = LogCapture::start();
        // One warmup and one pause interval, then load.
        let mut summary = DropSummary::new(1, 1);
        for report in &reports {
            summary.record(report);
        }
        summary.log();
        logs.contents()
    }

    #[test]
    fn reports_no_drops() {
        let logs = logged_summary(vec![
            report(vec![success_result(0, 0, 100, 1000, 10)]),
            report(vec![]),
            report(vec![success_result(0, 0, 100, 1000, 10)]),
        ]);

        assert!(logs.contains("No transactions were dropped."), "{logs}");
        assert!(!logs.contains("invalid"), "{logs}");
    }

    #[test]
    fn marks_run_with_load_phase_drops_as_invalid() {
        let logs = logged_summary(vec![
            report(vec![]),
            report(vec![]),
            report(vec![
                success_result(0, 0, 100, 1000, 10),
                dropped_result(1, 200, 1000, DroppedCode::NoVirtualUser),
                dropped_result(2, 300, 1000, DroppedCode::NoVirtualUser),
                dropped_result(3, 400, 1000, DroppedCode::Late),
            ]),
        ]);

        assert!(logs.contains("The run is invalid"), "{logs}");
        assert!(
            logs.contains("3 of 4 load-phase transactions were dropped"),
            "{logs}"
        );
        assert!(
            logs.contains("2 without a free virtual user, 1 sent too late"),
            "{logs}"
        );
        assert!(logs.contains("--virtual-user-count"), "{logs}");
        assert!(logs.contains("more CPU"), "{logs}");
        assert!(!logs.contains("script errors"), "{logs}");
    }

    #[test]
    fn reports_warmup_drops_without_invalidating_the_run() {
        let logs = logged_summary(vec![
            report(vec![dropped_result(1, 200, 1000, DroppedCode::Late)]),
            report(vec![]),
            report(vec![success_result(0, 0, 100, 1000, 10)]),
        ]);

        assert!(
            logs.contains("Dropped 1 of 1 warmup transactions"),
            "{logs}"
        );
        assert!(!logs.contains("invalid"), "{logs}");
        assert!(!logs.contains("No transactions were dropped."), "{logs}");
    }
}
