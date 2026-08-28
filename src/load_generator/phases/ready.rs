use futures_util::{SinkExt, StreamExt};
use rand::SeedableRng;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{self, Framed};

use crate::dispatch::Dispatcher;
use crate::http::HttpClient;
use crate::load::{
    LoadInterval, LoadIntervalPacer, LoadStepDuration, LoadStepStart, LoadTestTime, WaitTime,
};
use crate::math::exponential::DefaultExponentialSampler;
use crate::math::rng::RangeRNG;
use crate::net::MessageFramer;
use crate::tracker::{IntervalReport, IntervalStatistics, Tracker};
use crate::wire::command::Command;
use crate::wire::report::GeneratorUpdate;
use crate::wire::{LoadProfile, LoadStepDeadline, Warmup};

type Framer = MessageFramer<GeneratorUpdate, Command>;

#[derive(Debug, thiserror::Error)]
pub enum WaitForCommandError {
    #[error("unexpected disconnect from orchestrator: did not receive a command")]
    UnexpectedDisconnect,
    #[error("failed to receive command from orchestrator: {source}")]
    ReceiveError {
        source: <Framer as codec::Decoder>::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum LoadTestError {
    #[error("failed to send message to orchestrator: {0}")]
    SendError(#[from] <Framer as codec::Encoder<GeneratorUpdate>>::Error),
}

#[derive(Debug)]
pub struct ReadyState {
    pub(super) profile: LoadProfile,
    pub(super) warmup: Warmup,
    pub(super) seed: u64,
}

#[derive(Debug)]
// TODO: Use type state pattern
pub struct ReadyHandle<S, T, R>
where
    T: HttpClient + Clone + 'static,
    R: RangeRNG + Send + Sync + 'static,
{
    pub state: ReadyState,
    pub framed: Framed<S, Framer>,
    pub dispatcher: Dispatcher<T, R>,
    pub tracker: Tracker,
}

impl<S, T, R> ReadyHandle<S, T, R>
where
    T: HttpClient + Clone + 'static,
    R: RangeRNG + Send + Sync + 'static,
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(
        state: ReadyState,
        stream: S,
        dispatcher: Dispatcher<T, R>,
        tracker: Tracker,
    ) -> Self {
        let framed = Framed::new(stream, Framer::new());
        Self {
            state,
            framed,
            dispatcher,
            tracker,
        }
    }

    pub async fn wait_for_command(&mut self) -> Result<Command, WaitForCommandError> {
        match self.framed.next().await {
            Some(Ok(cmd)) => Ok(cmd),
            Some(Err(source)) => Err(WaitForCommandError::ReceiveError { source }),
            None => Err(WaitForCommandError::UnexpectedDisconnect),
        }
    }

    pub async fn warmup(&mut self) -> Result<(), LoadTestError> {
        let time_zero = LoadTestTime::now();
        for deadline in (1..=self.state.warmup.duration).map(LoadStepDeadline::from_secs) {
            let step_start = LoadStepStart::now();
            let target_time = (time_zero + deadline).duration_since(time_zero);
            let duration = LoadStepDuration::until_deadline(step_start, time_zero, deadline);
            let interval = LoadInterval::new(duration, self.state.warmup.rate);
            let mut pacer = LoadIntervalPacer::new(
                interval,
                DefaultExponentialSampler::new(rand::rngs::StdRng::seed_from_u64(self.state.seed)),
            );

            while let Some((batch, wait)) = pacer.next(step_start.elapsed()) {
                self.dispatcher
                    .dispatch_batch(batch, target_time, time_zero);

                if wait != WaitTime::ZERO {
                    tokio::time::sleep(wait.as_duration()).await;
                }
            }
            let final_batch_time = time_zero.elapsed();
            let wait = pacer.remaining(step_start.elapsed());
            if wait != WaitTime::ZERO {
                tokio::time::sleep(wait.as_duration()).await;
            }

            let stats = self.tracker.interval_statistics();

            let report = IntervalReport::new(
                target_time,
                self.state.warmup.rate,
                stats,
                Some(final_batch_time),
            );

            self.framed
                .send(GeneratorUpdate::IntervalReport(report))
                .await?;
        }

        let pause_zero = LoadTestTime::now();
        for deadline in (1..=self.state.warmup.pause).map(LoadStepDeadline::from_secs) {
            let target_time = pause_zero + deadline;
            tokio::time::sleep(target_time - LoadStepStart::now()).await;

            let report = IntervalReport::new(
                target_time.duration_since(pause_zero),
                0,
                IntervalStatistics::default(),
                None,
            );
            self.framed
                .send(GeneratorUpdate::IntervalReport(report))
                .await?;
        }

        Ok(())
    }

    pub async fn load_test(&mut self) -> Result<(), LoadTestError> {
        let time_zero = LoadTestTime::now();
        for step in &self.state.profile.steps {
            let step_start = LoadStepStart::now();
            let target_time = (time_zero + step.deadline).duration_since(time_zero);
            let duration = LoadStepDuration::until_deadline(step_start, time_zero, step.deadline);
            let interval = LoadInterval::new(duration, step.count);
            let mut pacer = LoadIntervalPacer::new(
                interval,
                DefaultExponentialSampler::new(rand::rngs::StdRng::seed_from_u64(self.state.seed)),
            );

            while let Some((batch, wait)) = pacer.next(step_start.elapsed()) {
                self.dispatcher
                    .dispatch_batch(batch, target_time, time_zero);
                if wait != WaitTime::ZERO {
                    tokio::time::sleep(wait.as_duration()).await;
                }
            }
            let final_batch_time = time_zero.elapsed();
            let wait = pacer.remaining(step_start.elapsed());
            if wait != WaitTime::ZERO {
                tokio::time::sleep(wait.as_duration()).await;
            }

            let stats = self.tracker.interval_statistics();

            let report =
                IntervalReport::new(target_time, step.count, stats, Some(final_batch_time));

            self.framed
                .send(GeneratorUpdate::IntervalReport(report))
                .await?;
        }

        while self.tracker.inflight_count() > 0 {
            let elapsed = time_zero.elapsed();
            let target_time =
                time_zero + (LoadStepDeadline::from_secs(1) + elapsed.as_secs_f64().floor());
            tokio::time::sleep(target_time - LoadStepStart::now()).await;
            let report = IntervalReport::new(
                target_time.duration_since(time_zero),
                0,
                self.tracker.interval_statistics(),
                None,
            );
            self.framed
                .send(GeneratorUpdate::IntervalReport(report))
                .await?;
        }

        self.framed.send(GeneratorUpdate::Finished).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::time::Duration;

    use futures_util::StreamExt;
    use tokio::io::DuplexStream;
    use tokio_util::codec::Framed;

    use super::*;
    use crate::load::RelativeLoadTestTime;
    use crate::test_utils::prelude::*;
    use crate::transaction::LoadGeneratorId;
    use crate::wire::LoadStep;

    async fn handle_with(
        profile: LoadProfile,
        warmup: Warmup,
        pool_size: u32,
        stream: DuplexStream,
        delay: Option<u64>,
    ) -> ReadyHandle<DuplexStream, MockHttpClient, rand::rngs::StdRng> {
        let Scenario { pool, .. } = ScenarioBuilder::default()
            .modify_pool(|p| p.with_size(pool_size))
            .modify_client(|c| {
                c.with_timeout_duration(Duration::from_secs(60))
                    .with_response_delay(Duration::from_millis(delay.unwrap_or_default()))
            })
            .build()
            .await;
        let tracker = Tracker::new();
        let dispatcher = Dispatcher::new(LoadGeneratorId::new(0), pool, tracker.clone());
        let state = ReadyState {
            profile,
            warmup,
            seed: 0,
        };
        ReadyHandle::new(state, stream, dispatcher, tracker)
    }

    type Orchestrator = Framed<DuplexStream, MessageFramer<Command, GeneratorUpdate>>;

    fn orchestrator(stream: DuplexStream) -> Orchestrator {
        Framed::new(stream, MessageFramer::<Command, GeneratorUpdate>::new())
    }

    async fn recv_interval_report(
        framed: &mut Framed<DuplexStream, MessageFramer<Command, GeneratorUpdate>>,
    ) -> IntervalReport {
        match recv_msg(framed).await {
            GeneratorUpdate::IntervalReport(report) => report,
            other => panic!("expected IntervalReport, got {other:?}"),
        }
    }

    async fn recv_msg(
        framed: &mut Framed<DuplexStream, MessageFramer<Command, GeneratorUpdate>>,
    ) -> GeneratorUpdate {
        framed.next().await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn load_test_sends_one_interval_report_per_profile_step() {
        let (client, server) = tokio::io::duplex(20);

        let profile = LoadProfile {
            steps: vec![
                LoadStep {
                    deadline: LoadStepDeadline::from_secs(1),
                    count: 3,
                },
                LoadStep {
                    deadline: LoadStepDeadline::from_secs(2),
                    count: 7,
                },
            ],
        };
        let warmup = Warmup {
            rate: 0,
            duration: 0,
            pause: 0,
        };
        let mut handle = handle_with(profile, warmup, 8, client, None).await;

        let task = tokio::spawn(async move { handle.load_test().await });

        let mut orchestrator = orchestrator(server);
        let r1 = recv_interval_report(&mut orchestrator).await;
        let r2 = recv_interval_report(&mut orchestrator).await;
        let finished = recv_msg(&mut orchestrator).await;

        task.await.unwrap().unwrap();

        assert_eq!(
            r1.target_time,
            RelativeLoadTestTime::new(Duration::from_secs(1))
        );
        assert_eq!(r1.load_level, 3);
        assert!(r1.final_batch_time.is_some(),);

        assert_eq!(
            r2.target_time,
            RelativeLoadTestTime::new(Duration::from_secs(2))
        );
        assert_eq!(r2.load_level, 7);
        assert!(r2.final_batch_time.is_some());
        assert_matches!(finished, GeneratorUpdate::Finished);
    }

    #[tokio::test]
    async fn warmup_emits_warmup_reports_then_pause_reports() {
        let (client, server) = tokio::io::duplex(10);

        let profile = LoadProfile { steps: vec![] };
        let warmup = Warmup {
            rate: 5,
            duration: 3,
            pause: 1,
        };
        let mut handle = handle_with(profile, warmup, 4, client, None).await;

        let task = tokio::spawn(async move { handle.warmup().await });

        let mut orchestrator = orchestrator(server);
        let wr1 = recv_interval_report(&mut orchestrator).await;
        let wr2 = recv_interval_report(&mut orchestrator).await;
        let wr3 = recv_interval_report(&mut orchestrator).await;
        let pr1 = recv_interval_report(&mut orchestrator).await;

        task.await.unwrap().unwrap();

        for (target_time, report) in (1..4).zip([wr1, wr2, wr3]) {
            assert_eq!(
                report.target_time,
                RelativeLoadTestTime::new(Duration::from_secs(target_time))
            );
            assert_eq!(report.load_level, 5);
            assert!(report.final_batch_time.is_some());
        }

        assert_eq!(
            pr1.target_time,
            RelativeLoadTestTime::new(Duration::from_secs(1))
        );
        assert_eq!(pr1.load_level, 0);
        assert!(pr1.final_batch_time.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn load_test_emits_drain_reports_at_1_second_intervals_while_inflight() {
        let (client, server) = tokio::io::duplex(1024);

        let profile = LoadProfile {
            steps: vec![LoadStep {
                deadline: LoadStepDeadline::from_secs(1),
                count: 5,
            }],
        };
        let warmup = Warmup {
            rate: 0,
            duration: 0,
            pause: 0,
        };

        let mut handle = handle_with(profile, warmup, 4, client, Some(2500)).await;

        let task = tokio::spawn(async move { handle.load_test().await });

        let mut framed = Framed::new(server, MessageFramer::<Command, GeneratorUpdate>::new());

        let load_report = recv_interval_report(&mut framed).await;
        assert_eq!(load_report.load_level, 5);
        let last_load_target = load_report.target_time;

        let mut drain_reports = Vec::new();
        let mut last_seen: Option<RelativeLoadTestTime> = None;
        let finished = loop {
            match recv_msg(&mut framed).await {
                GeneratorUpdate::IntervalReport(report) => {
                    if let Some(prev) = last_seen {
                        assert_eq!(report.target_time.as_secs_f64() - prev.as_secs_f64(), 1.0);
                    } else {
                        // First drain must sit on the second mark after the last load mark.
                        let expected_first = RelativeLoadTestTime::new(Duration::from_secs(
                            last_load_target.as_secs_f64() as u64 + 1,
                        ));
                        assert_eq!(report.target_time, expected_first);
                    }
                    assert_eq!(report.load_level, 0);
                    assert!(report.final_batch_time.is_none());
                    last_seen = Some(report.target_time);
                    drain_reports.push(report);
                }
                GeneratorUpdate::Finished => break GeneratorUpdate::Finished,
            }
        };

        assert_matches!(finished, GeneratorUpdate::Finished);

        task.await.unwrap().unwrap();

        assert!(drain_reports.len() >= 2);

        // At least one drain report should capture completions (the one that covers
        // the transition to zero).
        let any_drain_has_stats = drain_reports.iter().any(|r| !r.stats.results().is_empty());
        assert!(any_drain_has_stats);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn load_test_skips_drain_reports_when_tasks_complete_before_next_second_mark() {
        let (client, server) = tokio::io::duplex(1024);

        let profile = LoadProfile {
            steps: vec![LoadStep {
                deadline: LoadStepDeadline::from_secs(1),
                count: 5,
            }],
        };
        let warmup = Warmup {
            rate: 0,
            duration: 0,
            pause: 0,
        };
        let mut handle = handle_with(profile, warmup, 8, client, None).await;

        let task = tokio::spawn(async move { handle.load_test().await });

        let mut framed = Framed::new(server, MessageFramer::<Command, GeneratorUpdate>::new());
        let load_report = recv_interval_report(&mut framed).await;
        let finished = recv_msg(&mut framed).await;

        task.await.unwrap().unwrap();

        assert_eq!(load_report.load_level, 5);
        assert_matches!(finished, GeneratorUpdate::Finished);
    }
}
