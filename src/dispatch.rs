use crate::http::HttpClient;
use crate::load::{LoadTestTime, RelativeLoadTestTime, StartTime};
use crate::math::rng::RangeRNG;
use crate::tracker::Tracker;
use crate::transaction::{Batch, LoadGeneratorId, Transaction};
use crate::virtual_user;

#[derive(Debug)]
pub struct Dispatcher<T, R>
where
    T: HttpClient + Clone + 'static,
    R: RangeRNG + Send + Sync + 'static,
{
    loadgenerator_id: LoadGeneratorId,
    pool: virtual_user::Pool<T, R>,
    tracker: Tracker,
}

impl<T, R> Dispatcher<T, R>
where
    T: HttpClient + Clone + 'static,
    R: RangeRNG + Send + Sync + 'static,
{
    pub fn new(
        loadgenerator_id: LoadGeneratorId,
        pool: virtual_user::Pool<T, R>,
        tracker: Tracker,
    ) -> Self {
        Self {
            loadgenerator_id,
            pool,
            tracker,
        }
    }
    pub fn dispatch_batch(
        &self,
        batch: Batch,
        target_time: RelativeLoadTestTime,
        time_zero: LoadTestTime,
    ) {
        self.tracker.log_dispatched(batch.size().into());
        for _ in 0..batch.size() {
            let loadgenerator_id = self.loadgenerator_id;
            let pool = self.pool.clone();
            let tracker = self.tracker.clone();
            let transaction =
                Transaction::new(loadgenerator_id, StartTime::now(time_zero), target_time);
            tokio::spawn(async move {
                let virtual_user = pool.acquire().await;
                let result = virtual_user.send_next_request(pool, transaction).await;
                tracker.log_result(result);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::test_utils::prelude::*;
    use crate::transaction::TransactionResult;

    async fn dispatcher_from(
        loadgenerator_id: u8,
        pool_size: u32,
    ) -> (Dispatcher<MockHttpClient, rand::rngs::StdRng>, Tracker) {
        let Scenario { pool, .. } = ScenarioBuilder::default()
            .modify_pool(|p| p.with_size(pool_size))
            .build()
            .await;
        let tracker = Tracker::new();
        let dispatcher = Dispatcher::new(
            LoadGeneratorId::new(loadgenerator_id),
            pool,
            tracker.clone(),
        );
        (dispatcher, tracker)
    }

    async fn wait(tracker: &Tracker) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if tracker.inflight_count() == 0 {
                return;
            }
            if Instant::now() >= deadline {
                panic!(
                    "timed out waiting for inflight_count to reach 0; got {}",
                    tracker.inflight_count()
                );
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    fn id_and_target_time_of(
        result: &TransactionResult,
    ) -> (LoadGeneratorId, RelativeLoadTestTime) {
        match result {
            TransactionResult::Success { metadata, .. }
            | TransactionResult::Failed { metadata, .. }
            | TransactionResult::Timeout { metadata, .. } => {
                (metadata.loadgenerator_id, metadata.target_time)
            }
            TransactionResult::Dropped { metadata, .. } => {
                (metadata.loadgenerator_id, metadata.target_time)
            }
        }
    }

    fn start_time_of(result: &TransactionResult) -> RelativeLoadTestTime {
        match result {
            TransactionResult::Success { metadata, .. }
            | TransactionResult::Failed { metadata, .. }
            | TransactionResult::Timeout { metadata, .. } => metadata.start_time,
            TransactionResult::Dropped { metadata, .. } => metadata.start_time,
        }
    }

    #[tokio::test]
    async fn dispatch_batch_dispatches_all_transactions() {
        let (dispatcher, tracker) = dispatcher_from(1, 8).await;
        let batch = Batch::new(5);
        let target_time = RelativeLoadTestTime::new(Duration::from_secs(1));
        let time_zero = LoadTestTime::now();

        dispatcher.dispatch_batch(batch, target_time, time_zero);
        wait(&tracker).await;
    }

    #[tokio::test]
    async fn dispatch_batch_propagates_loadgenerator_id_and_target_time() {
        let loadgenerator_id = LoadGeneratorId::new(42);
        let (dispatcher, tracker) = dispatcher_from(42, 4).await;
        let batch = Batch::new(4);
        let target_time = RelativeLoadTestTime::new(Duration::from_millis(250));
        let time_zero = LoadTestTime::now();

        dispatcher.dispatch_batch(batch, target_time, time_zero);
        wait(&tracker).await;

        let stats = tracker.interval_statistics();
        let results = stats.results();
        assert_eq!(results.len(), 4);
        for result in results {
            let (id, time) = id_and_target_time_of(result);
            assert_eq!(id, loadgenerator_id);
            assert_eq!(time, target_time);
        }
    }

    #[tokio::test]
    async fn dispatch_batch_logs_results_with_start_time_before_target_time() {
        let (dispatcher, tracker) = dispatcher_from(2, 4).await;
        let batch = Batch::new(3);
        let target_time = RelativeLoadTestTime::new(Duration::from_secs(1));
        let time_zero = LoadTestTime::now();

        dispatcher.dispatch_batch(batch, target_time, time_zero);
        wait(&tracker).await;

        let stats = tracker.interval_statistics();
        assert_eq!(stats.results().len(), 3);
        for result in stats.results() {
            let start_time = start_time_of(result);
            assert!(
                start_time < target_time,
                "expected start_time {start_time:?} < target_time {target_time:?}"
            );
        }
    }

    #[tokio::test]
    async fn dispatch_batch_completes_when_batch_exceeds_pool_size() {
        let (dispatcher, tracker) = dispatcher_from(3, 1).await;
        let batch = Batch::new(10);
        let target_time = RelativeLoadTestTime::new(Duration::from_secs(1));
        let time_zero = LoadTestTime::now();

        dispatcher.dispatch_batch(batch, target_time, time_zero);
        wait(&tracker).await;
    }
}
