use tokio::sync::mpsc::UnboundedSender;

use crate::http::HttpClient;
use crate::load::{
    ArrivalSchedule, LoadTestTime, RelativeLoadTestTime, ScheduledStep, StartTime, wait_until,
};
use crate::math::rng::RangeRNG;
use crate::tracker::Tracker;
use crate::transaction::{Batch, LoadGeneratorId, Transaction};
use crate::virtual_user;

/// Signals that all arrivals of a step have been dispatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepDone {
    /// Index of the finished step.
    pub index: usize,
    /// Time of the step's last dispatch, or `None` if it had no arrivals.
    pub last_dispatch: Option<RelativeLoadTestTime>,
}

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

    pub fn loadgenerator_id(&self) -> LoadGeneratorId {
        self.loadgenerator_id
    }

    pub fn dispatch_batch(
        &self,
        batch: Batch,
        target_time: RelativeLoadTestTime,
        time_zero: LoadTestTime,
    ) {
        for _ in 0..batch.size() {
            self.dispatch(StartTime::now(time_zero), target_time);
        }
    }

    /// Dispatches a single transaction that starts at `start_time`.
    pub fn dispatch(&self, start_time: StartTime, target_time: RelativeLoadTestTime) {
        self.tracker.log_dispatched(1);
        let pool = self.pool.clone();
        let tracker = self.tracker.clone();
        let transaction = Transaction::new(self.loadgenerator_id, start_time, target_time);
        tokio::spawn(async move {
            let virtual_user = pool.acquire().await;
            let result = virtual_user.send_next_request(pool, transaction).await;
            tracker.log_result(result);
        });
    }

    /// Dispatches each arrival of a Poisson schedule over `steps` at its scheduled time.
    ///
    /// Arrival offsets are relative to `time_zero`. Each transaction starts at its
    /// scheduled arrival, and its target time is the deadline of its step. A
    /// [`StepDone`] is sent for every step, in order, once all of its arrivals
    /// have been dispatched. Returns early if the receiver of `steps_done` is dropped.
    ///
    /// This blocks the calling thread until the last arrival has been dispatched,
    /// busy-waiting for precise timing, so run it with [`tokio::task::spawn_blocking`].
    pub fn run_schedule(
        &self,
        steps: Vec<ScheduledStep>,
        rng: impl rand::Rng,
        time_zero: LoadTestTime,
        steps_done: UnboundedSender<StepDone>,
    ) {
        let target_times: Vec<_> = steps
            .iter()
            .map(|step| RelativeLoadTestTime::new(step.deadline))
            .collect();
        let mut next_step = 0;
        let mut last_dispatch = None;

        for arrival in ArrivalSchedule::new(steps, rng) {
            while next_step < arrival.step {
                let done = StepDone {
                    index: next_step,
                    last_dispatch: last_dispatch.take(),
                };
                if steps_done.send(done).is_err() {
                    return;
                }
                next_step += 1;
            }
            let offset = RelativeLoadTestTime::new(arrival.offset);
            wait_until((time_zero + offset).into());
            self.dispatch(
                StartTime::scheduled(time_zero, offset),
                target_times[arrival.step],
            );
            last_dispatch = Some(time_zero.elapsed());
        }

        for index in next_step..target_times.len() {
            let done = StepDone {
                index,
                last_dispatch: last_dispatch.take(),
            };
            if steps_done.send(done).is_err() {
                return;
            }
        }
    }
}

impl<T, R> Clone for Dispatcher<T, R>
where
    T: HttpClient + Clone + 'static,
    R: RangeRNG + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            loadgenerator_id: self.loadgenerator_id,
            pool: self.pool.clone(),
            tracker: self.tracker.clone(),
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
    async fn dispatch_keeps_scheduled_start_time() {
        let (dispatcher, tracker) = dispatcher_from(4, 2).await;
        let time_zero = LoadTestTime::now();
        let offsets = [
            Duration::ZERO,
            Duration::from_micros(250),
            Duration::from_millis(3),
        ];
        let target_time = RelativeLoadTestTime::new(Duration::from_secs(1));

        for offset in offsets {
            let start_time = StartTime::scheduled(time_zero, RelativeLoadTestTime::new(offset));
            dispatcher.dispatch(start_time, target_time);
        }
        wait(&tracker).await;

        let stats = tracker.interval_statistics();
        let mut start_times: Vec<_> = stats.results().iter().map(start_time_of).collect();
        start_times.sort();
        let expected: Vec<_> = offsets.into_iter().map(RelativeLoadTestTime::new).collect();
        assert_eq!(start_times, expected);
    }

    fn step(index: usize, deadline: Duration, rate: f64) -> ScheduledStep {
        ScheduledStep {
            index,
            deadline,
            rate,
        }
    }

    fn rng() -> rand::rngs::StdRng {
        rand::SeedableRng::seed_from_u64(3)
    }

    async fn run_schedule(
        dispatcher: &Dispatcher<MockHttpClient, rand::rngs::StdRng>,
        steps: Vec<ScheduledStep>,
        time_zero: LoadTestTime,
    ) -> Vec<StepDone> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let dispatcher = dispatcher.clone();
        tokio::task::spawn_blocking(move || {
            dispatcher.run_schedule(steps, rng(), time_zero, sender)
        })
        .await
        .unwrap();

        let mut done = Vec::new();
        while let Some(step_done) = receiver.recv().await {
            done.push(step_done);
        }
        done
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_schedule_dispatches_every_arrival_at_its_scheduled_time() {
        let (dispatcher, tracker) = dispatcher_from(5, 64).await;
        let steps = vec![
            step(0, Duration::from_millis(100), 2000.0),
            step(1, Duration::from_millis(200), 500.0),
        ];
        let expected: Vec<_> = ArrivalSchedule::new(steps.clone(), rng()).collect();
        let time_zero = LoadTestTime::now();

        run_schedule(&dispatcher, steps, time_zero).await;
        wait(&tracker).await;

        let stats = tracker.interval_statistics();
        let mut actual: Vec<_> = stats
            .results()
            .iter()
            .map(|result| (start_time_of(result), id_and_target_time_of(result).1))
            .collect();
        actual.sort();
        let expected: Vec<_> = expected
            .into_iter()
            .map(|arrival| {
                let deadline = Duration::from_millis(100 * (arrival.step as u64 + 1));
                (
                    RelativeLoadTestTime::new(arrival.offset),
                    RelativeLoadTestTime::new(deadline),
                )
            })
            .collect();
        assert_eq!(actual, expected);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_schedule_reports_every_step_in_order() {
        let (dispatcher, tracker) = dispatcher_from(6, 8).await;
        let steps = vec![
            step(0, Duration::from_millis(50), 1000.0),
            step(1, Duration::from_millis(100), 0.0),
            step(2, Duration::from_millis(150), 1000.0),
            step(3, Duration::from_millis(200), 0.0),
        ];

        let done = run_schedule(&dispatcher, steps, LoadTestTime::now()).await;
        wait(&tracker).await;

        let indices: Vec<_> = done.iter().map(|d| d.index).collect();
        assert_eq!(indices, vec![0, 1, 2, 3]);
        assert!(done[0].last_dispatch.is_some());
        assert_eq!(done[1].last_dispatch, None);
        assert!(done[2].last_dispatch.is_some());
        assert_eq!(done[3].last_dispatch, None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_schedule_dispatches_overdue_arrivals_immediately() {
        let (dispatcher, tracker) = dispatcher_from(7, 64).await;
        let steps = vec![step(0, Duration::from_millis(100), 1000.0)];
        // The whole schedule lies in the past.
        let time_zero = LoadTestTime::now();
        tokio::time::sleep(Duration::from_millis(200)).await;

        let before = Instant::now();
        run_schedule(&dispatcher, steps, time_zero).await;

        assert!(before.elapsed() < Duration::from_millis(50));
        wait(&tracker).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_schedule_stops_when_receiver_is_dropped() {
        let (dispatcher, tracker) = dispatcher_from(8, 8).await;
        let steps = vec![
            step(0, Duration::from_millis(10), 0.0),
            step(1, Duration::from_secs(60), 1000.0),
        ];
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        drop(receiver);

        let before = Instant::now();
        tokio::task::spawn_blocking(move || {
            dispatcher.run_schedule(steps, rng(), LoadTestTime::now(), sender)
        })
        .await
        .unwrap();

        assert!(before.elapsed() < Duration::from_secs(1));
        assert_eq!(tracker.inflight_count(), 0);
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
