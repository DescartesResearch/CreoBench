use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;

use crate::load::RelativeLoadTestTime;
use crate::transaction::TransactionResult;

#[derive(Debug, Clone, Default)]
pub struct Tracker {
    inner: Arc<Inner>,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn log_result(&self, result: TransactionResult) {
        let Inner {
            total, interval, ..
        } = &*self.inner;
        match &result {
            TransactionResult::Success { .. } => {
                total.success.fetch_add(1, Ordering::Relaxed);
                interval.log_result(result);
            }
            TransactionResult::Failed { .. } => {
                total.failed.fetch_add(1, Ordering::Relaxed);
                interval.log_result(result);
            }
            TransactionResult::Timeout { .. } => {
                total.timeout.fetch_add(1, Ordering::Relaxed);
                interval.log_result(result);
            }
            TransactionResult::Dropped { .. } => {
                total.dropped.fetch_add(1, Ordering::Relaxed);
                interval.log_result(result);
            }
        }
    }

    pub fn interval_statistics(&self) -> IntervalStatistics {
        let Inner { interval, .. } = &*self.inner;
        let successful_transactions = interval.counts.success.swap(0, Ordering::Relaxed);
        let failed_transactions = interval.counts.failed.swap(0, Ordering::Relaxed);
        let timeout_transactions = interval.counts.timeout.swap(0, Ordering::Relaxed);
        let dropped_transactions = interval.counts.dropped.swap(0, Ordering::Relaxed);

        let service_time_sum = interval.service_time_sum.swap(0, Ordering::Relaxed);
        let service_time_count = interval.service_time_count.swap(0, Ordering::Relaxed);
        let mut transaction_results = interval.transaction_results.lock();
        let transaction_results =
            std::mem::replace(&mut *transaction_results, Vec::with_capacity(1024));
        IntervalStatistics {
            successful_transactions,
            failed_transactions,
            timeout_transactions,
            dropped_transactions,
            response_time_sum: service_time_sum,
            response_time_count: service_time_count,
            transaction_results,
        }
    }

    pub fn totals(&self) -> TotalCounts {
        let total = &self.inner.total;
        TotalCounts {
            success: total.success.load(Ordering::Relaxed),
            failed: total.failed.load(Ordering::Relaxed),
            timeout: total.timeout.load(Ordering::Relaxed),
            dropped: total.dropped.load(Ordering::Relaxed),
        }
    }

    pub fn log_dispatched(&self, n: u64) {
        self.inner.dispatched.fetch_add(n, Ordering::Relaxed);
    }

    pub fn inflight_count(&self) -> u64 {
        self.inner
            .dispatched
            .load(Ordering::Relaxed)
            .saturating_sub(self.totals().sum())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TotalCounts {
    pub success: u64,
    pub failed: u64,
    pub timeout: u64,
    pub dropped: u64,
}

impl TotalCounts {
    pub fn sum(&self) -> u64 {
        self.success + self.failed + self.timeout + self.dropped
    }
}

#[derive(Debug, Default)]
struct Inner {
    total: Counts,
    interval: IntervalTracker,
    dispatched: AtomicU64,
}

#[derive(Debug)]
struct IntervalTracker {
    counts: Counts,
    service_time_sum: AtomicU64,
    service_time_count: AtomicU64,
    transaction_results: Mutex<Vec<TransactionResult>>,
}

impl Default for IntervalTracker {
    fn default() -> Self {
        Self {
            counts: Default::default(),
            service_time_sum: Default::default(),
            service_time_count: Default::default(),
            transaction_results: Mutex::new(Vec::with_capacity(1024)),
        }
    }
}

impl IntervalTracker {
    fn log_result(&self, result: TransactionResult) {
        let IntervalTracker {
            counts,
            service_time_sum,
            service_time_count,
            transaction_results,
        } = self;
        match &result {
            TransactionResult::Success { service_time, .. } => {
                counts.success.fetch_add(1, Ordering::Relaxed);
                service_time_sum.fetch_add(service_time.as_millis(), Ordering::Relaxed);
                service_time_count.fetch_add(1, Ordering::Relaxed);
            }
            TransactionResult::Failed { service_time, .. } => {
                counts.failed.fetch_add(1, Ordering::Relaxed);
                service_time_sum.fetch_add(service_time.as_millis(), Ordering::Relaxed);
                service_time_count.fetch_add(1, Ordering::Relaxed);
            }
            TransactionResult::Timeout { service_time, .. } => {
                counts.timeout.fetch_add(1, Ordering::Relaxed);
                service_time_sum.fetch_add(service_time.as_millis(), Ordering::Relaxed);
                service_time_count.fetch_add(1, Ordering::Relaxed);
            }
            TransactionResult::Dropped { .. } => {
                counts.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
        transaction_results.lock().push(result);
    }
}

#[derive(Debug, Default)]
struct Counts {
    success: AtomicU64,
    failed: AtomicU64,
    timeout: AtomicU64,
    dropped: AtomicU64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntervalStatistics {
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub timeout_transactions: u64,
    pub dropped_transactions: u64,
    response_time_sum: u64,
    response_time_count: u64,
    transaction_results: Vec<TransactionResult>,
}

impl IntervalStatistics {
    pub fn results(&self) -> &[TransactionResult] {
        &self.transaction_results
    }

    pub fn average_response_time(&self) -> u64 {
        self.response_time_sum
            .checked_div(self.response_time_count)
            .unwrap_or(0)
    }
}

impl std::ops::Add<Self> for IntervalStatistics {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self.transaction_results.extend(rhs.transaction_results);
        Self {
            successful_transactions: self.successful_transactions + rhs.successful_transactions,
            failed_transactions: self.failed_transactions + rhs.failed_transactions,
            timeout_transactions: self.timeout_transactions + rhs.timeout_transactions,
            dropped_transactions: self.dropped_transactions + rhs.dropped_transactions,
            response_time_sum: self.response_time_sum + rhs.response_time_sum,
            response_time_count: self.response_time_count + rhs.response_time_count,
            transaction_results: self.transaction_results,
        }
    }
}

impl std::iter::Sum<IntervalStatistics> for IntervalStatistics {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = IntervalStatistics>,
    {
        iter.fold(Self::default(), |acc, x| acc + x)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntervalReport {
    pub target_time: RelativeLoadTestTime,
    pub load_level: u32,
    pub stats: IntervalStatistics,
    pub final_batch_time: Option<RelativeLoadTestTime>,
}

impl IntervalReport {
    pub fn new(
        target_time: RelativeLoadTestTime,
        load_level: u32,
        stats: IntervalStatistics,
        final_batch_time: Option<RelativeLoadTestTime>,
    ) -> Self {
        Self {
            target_time,
            load_level,
            stats,
            final_batch_time,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use proptest::prelude::*;

    use super::*;
    use crate::load::{LoadTestTime, ServiceTime, StartTime};
    use crate::test_utils::{impl_proptest_arbitrary, proptest_strategy, round_trip_proptest};
    use crate::transaction::{
        DroppedCode, FailedCode, LoadGeneratorId, SpecId, TimeoutCode, Transaction,
    };
    use crate::virtual_user::VirtualUserId;

    proptest_strategy! {
        interval_statistics_strategy: IntervalStatistics => {
            (
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                proptest::collection::vec(any::<TransactionResult>(), 0..8),
            )
                .prop_map(
                    |(
                        successful_transactions,
                        failed_transactions,
                        timeout_transactions,
                        dropped_transactions,
                        response_time_sum,
                        response_time_count,
                        transaction_results,
                    )| IntervalStatistics {
                        successful_transactions,
                        failed_transactions,
                        timeout_transactions,
                        dropped_transactions,
                        response_time_sum,
                        response_time_count,
                        transaction_results,
                    },
                )
        }
    }

    proptest_strategy! {
        interval_report_strategy: IntervalReport => {
            (
                any::<RelativeLoadTestTime>(),
                any::<u32>(),
                interval_statistics_strategy(),
                proptest::option::of(any::<RelativeLoadTestTime>()),
            )
                .prop_map(
                    |(target_time, load_level, stats, final_batch_time)| IntervalReport {
                        target_time,
                        load_level,
                        stats,
                        final_batch_time,
                    },
                )
        }
    }

    impl_proptest_arbitrary!(IntervalStatistics, interval_statistics_strategy);
    impl_proptest_arbitrary!(IntervalReport, interval_report_strategy);

    round_trip_proptest! {
        IntervalReport,
        interval_report_round_trip_single,
        interval_report_round_trip_multi,
        interval_report_round_trip_stream,
    }

    fn fresh_transaction() -> Transaction {
        Transaction::new(
            crate::transaction::LoadGeneratorId::new(1),
            StartTime::now(LoadTestTime::now()),
            RelativeLoadTestTime::new(Duration::from_millis(200)),
        )
    }

    fn success_result(virtual_user_id: u32, spec_id: usize, response_ms: u64) -> TransactionResult {
        fresh_transaction().into_success(
            VirtualUserId::new(virtual_user_id),
            SpecId::new(spec_id),
            ServiceTime::new(Duration::from_millis(response_ms)),
        )
    }

    fn failed_result(virtual_user_id: u32, spec_id: usize, response_ms: u64) -> TransactionResult {
        fresh_transaction().into_failed(
            VirtualUserId::new(virtual_user_id),
            SpecId::new(spec_id),
            ServiceTime::new(Duration::from_millis(response_ms)),
            FailedCode::Send("test error reason".into()),
        )
    }

    fn timeout_result(virtual_user_id: u32, spec_id: usize, response_ms: u64) -> TransactionResult {
        fresh_transaction().into_timeout(
            VirtualUserId::new(virtual_user_id),
            SpecId::new(spec_id),
            ServiceTime::new(Duration::from_millis(response_ms)),
            TimeoutCode::Error("test timeout".into()),
        )
    }

    fn dropped_result(virtual_user_id: u32) -> TransactionResult {
        fresh_transaction().into_dropped(
            VirtualUserId::new(virtual_user_id),
            DroppedCode::Error("test dropped reason".into()),
        )
    }

    #[test]
    fn log_result_measured_variants_increment_their_counts() {
        let tracker = Tracker::new();

        tracker.log_result(success_result(0, 0, 50));
        tracker.log_result(success_result(1, 0, 100));
        tracker.log_result(failed_result(2, 1, 75));
        tracker.log_result(failed_result(3, 1, 25));
        tracker.log_result(failed_result(4, 1, 125));
        tracker.log_result(timeout_result(5, 2, 200));
        tracker.log_result(dropped_result(15));

        let stats = tracker.interval_statistics();

        assert_eq!(stats.successful_transactions, 2);
        assert_eq!(stats.failed_transactions, 3);
        assert_eq!(stats.timeout_transactions, 1);
        assert_eq!(stats.dropped_transactions, 1);
        assert_eq!(stats.transaction_results.len(), 7);
    }

    #[test]
    fn average_response_time_excludes_dropped_transactions() {
        let tracker = Tracker::new();

        tracker.log_result(success_result(0, 0, 100));
        tracker.log_result(success_result(1, 1, 200));
        tracker.log_result(failed_result(2, 2, 300));
        tracker.log_result(timeout_result(3, 3, 400));
        tracker.log_result(dropped_result(4));
        tracker.log_result(dropped_result(5));

        let stats = tracker.interval_statistics();

        assert_eq!(stats.average_response_time(), 250);
        assert_eq!(stats.dropped_transactions, 2);
        assert_eq!(stats.transaction_results.len(), 6);
    }

    #[test]
    fn average_response_time_returns_weighted_average_across_intervals() {
        let tracker = Tracker::new();

        for _ in 0..10 {
            tracker.log_result(success_result(0, 0, 100));
        }
        let first = tracker.interval_statistics();

        for _ in 0..5 {
            tracker.log_result(success_result(0, 0, 400));
        }
        let second = tracker.interval_statistics();

        let combined = first + second;
        assert_eq!(combined.average_response_time(), 200);
    }

    #[test]
    fn average_response_time_returns_zero_when_only_dropped_transactions_were_measured() {
        let tracker = Tracker::new();

        tracker.log_result(dropped_result(0));
        tracker.log_result(dropped_result(1));
        tracker.log_result(dropped_result(2));

        let stats = tracker.interval_statistics();
        assert_eq!(stats.dropped_transactions, 3);
        assert_eq!(stats.average_response_time(), 0);
    }

    #[test]
    fn sum_of_interval_statistics_matches_add_result() {
        let tracker = Tracker::new();

        for _ in 0..3 {
            tracker.log_result(success_result(0, 0, 50));
        }
        for _ in 0..2 {
            tracker.log_result(failed_result(0, 0, 250));
        }
        let first = tracker.interval_statistics();

        for _ in 0..4 {
            tracker.log_result(timeout_result(0, 0, 75));
        }
        tracker.log_result(dropped_result(0));
        let second = tracker.interval_statistics();

        let summed: IntervalStatistics = [first.clone(), second.clone()].into_iter().sum();
        let added = first + second;

        assert_eq!(
            summed.successful_transactions,
            added.successful_transactions
        );
        assert_eq!(summed.failed_transactions, added.failed_transactions);
        assert_eq!(summed.timeout_transactions, added.timeout_transactions);
        assert_eq!(summed.dropped_transactions, added.dropped_transactions);
        assert_eq!(
            summed.average_response_time(),
            added.average_response_time()
        );
        assert_eq!(summed.transaction_results.len(), 10);
    }

    #[test]
    fn interval_statistics_resets_statistics() {
        let tracker = Tracker::new();

        tracker.log_result(success_result(0, 0, 10));
        tracker.log_result(failed_result(1, 1, 20));
        tracker.log_result(timeout_result(2, 2, 30));
        tracker.log_result(dropped_result(3));

        let stats = tracker.interval_statistics();

        assert_eq!(stats.successful_transactions, 1);
        assert_eq!(stats.failed_transactions, 1);
        assert_eq!(stats.timeout_transactions, 1);
        assert_eq!(stats.dropped_transactions, 1);
        assert_eq!(stats.transaction_results.len(), 4);

        let stats = tracker.interval_statistics();
        assert_eq!(stats.successful_transactions, 0);
        assert_eq!(stats.failed_transactions, 0);
        assert_eq!(stats.timeout_transactions, 0);
        assert_eq!(stats.dropped_transactions, 0);
        assert_eq!(stats.response_time_sum, 0);
        assert_eq!(stats.response_time_count, 0);
        assert!(stats.transaction_results.is_empty());
    }

    #[test]
    fn empty_tracker_returns_empty_totals_and_statistics() {
        let tracker = Tracker::new();

        let stats = tracker.interval_statistics();

        assert_eq!(tracker.totals(), TotalCounts::default());
        assert_eq!(stats.successful_transactions, 0);
        assert_eq!(stats.failed_transactions, 0);
        assert_eq!(stats.timeout_transactions, 0);
        assert_eq!(stats.dropped_transactions, 0);
        assert_eq!(stats.response_time_sum, 0);
        assert_eq!(stats.response_time_count, 0);
        assert!(stats.transaction_results.is_empty());
    }

    #[test]
    fn totals_persists_across_intervals() {
        let tracker = Tracker::new();

        tracker.log_result(success_result(0, 0, 10));
        tracker.log_result(failed_result(1, 1, 20));
        tracker.log_result(success_result(2, 2, 30));

        assert_eq!(
            tracker.totals(),
            TotalCounts {
                success: 2,
                failed: 1,
                timeout: 0,
                dropped: 0,
            }
        );

        tracker.interval_statistics();
        assert_eq!(
            tracker.totals(),
            TotalCounts {
                success: 2,
                failed: 1,
                timeout: 0,
                dropped: 0,
            }
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_log_result_accumulates_atomically() {
        let tracker = Tracker::new();
        let tasks = 8;
        let per_task = 1_000;

        let mut handles = Vec::with_capacity(tasks);
        for task_id in 0..tasks {
            let tracker = tracker.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..per_task {
                    let variant = (task_id + i) % 4;
                    let result = match variant {
                        0 => success_result(0, 0, 1),
                        1 => failed_result(0, 0, 1),
                        2 => timeout_result(0, 0, 1),
                        _ => dropped_result(0),
                    };
                    tracker.log_result(result);
                }
            }));
        }
        for handle in handles {
            handle.await.expect("task panicked");
        }

        let total_logged = tasks * per_task;
        let totals = tracker.totals();
        let sum = totals.sum();
        assert_eq!(sum, total_logged as u64);
        let stats = tracker.interval_statistics();
        assert_eq!(stats.transaction_results.len(), total_logged);
        let sum = stats.successful_transactions
            + stats.failed_transactions
            + stats.timeout_transactions
            + stats.dropped_transactions;
        assert_eq!(sum, total_logged as u64)
    }

    #[test]
    fn total_counts_sum_returns_sum_of_all_variants() {
        let counts = TotalCounts {
            success: 1,
            failed: 2,
            timeout: 3,
            dropped: 4,
        };
        assert_eq!(counts.sum(), 10);
        assert_eq!(TotalCounts::default().sum(), 0);
    }

    #[test]
    fn log_dispatched_bumps_inflight_count_by_n() {
        let tracker = Tracker::new();

        tracker.log_dispatched(5);

        assert_eq!(tracker.inflight_count(), 5);
    }

    #[test]
    fn log_dispatched_and_log_result_cancel() {
        let tracker = Tracker::new();

        tracker.log_dispatched(5);
        tracker.log_result(success_result(0, 0, 10));
        tracker.log_result(failed_result(1, 1, 20));
        tracker.log_result(dropped_result(2));

        assert_eq!(tracker.inflight_count(), 2);
    }

    #[test]
    fn inflight_count_is_zero_before_dispatch() {
        let tracker = Tracker::new();
        assert_eq!(tracker.inflight_count(), 0);
    }

    #[test]
    fn inflight_count_reflects_dispatched_before_results_log() {
        let tracker = Tracker::new();

        tracker.log_dispatched(5);
        assert_eq!(tracker.inflight_count(), 5);

        for i in 0..5 {
            tracker.log_result(success_result(i, 0, 10));
        }
        assert_eq!(tracker.inflight_count(), 0);
    }

    #[test]
    fn inflight_count_saturates_when_totals_exceed_dispatched() {
        let tracker = Tracker::new();
        let target_time = RelativeLoadTestTime::new(Duration::from_secs(1));
        let time_zero = LoadTestTime::now();

        // Pre-load an out-of-band completed result without a matching dispatched
        let transaction = Transaction::new(
            LoadGeneratorId::new(6),
            StartTime::now(time_zero),
            target_time,
        );
        tracker.log_result(transaction.into_success(
            VirtualUserId::new(0),
            SpecId::new(0),
            ServiceTime::new(Duration::from_millis(5)),
        ));

        assert_eq!(
            tracker.inflight_count(),
            0,
            "out-of-order log_result must not wrap inflight"
        );
    }
}
