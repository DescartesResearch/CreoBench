use futures_util::{SinkExt, StreamExt};
use rand::SeedableRng;
use rand::rngs::StdRng;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::codec::{self, Framed};

use crate::dispatch::{Dispatcher, StepDone};
use crate::http::HttpClient;
use crate::load::{LoadStepStart, LoadTestTime, RelativeLoadTestTime, ScheduledStep};
use crate::math::rng::RangeRNG;
use crate::net::MessageFramer;
use crate::tracker::{IntervalReport, IntervalStatistics, Tracker};
use crate::transaction::LoadGeneratorId;
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

#[derive(Debug, Clone, Copy)]
enum Phase {
    Warmup = 0,
    Load = 1,
}

/// Random number generator for the arrivals of one phase on one load generator.
///
/// Every combination of seed, load generator, and phase yields an independent
/// stream, so the arrivals of all load generators together again form a Poisson
/// process. [`SeedableRng::seed_from_u64`] scrambles its input, so neighbouring
/// values give unrelated streams.
fn arrival_rng(seed: u64, loadgenerator_id: LoadGeneratorId, phase: Phase) -> StdRng {
    let id = u64::from(loadgenerator_id.get());
    StdRng::seed_from_u64(seed ^ (id << 32) ^ ((phase as u64) << 48))
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
        let warmup = self.state.warmup;
        let steps = ScheduledStep::from_warmup(&warmup);
        let load_levels = vec![warmup.rate; steps.len()];
        self.run_phase(steps, load_levels, Phase::Warmup).await?;

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

    /// Runs one phase of Poisson arrivals over `steps` and reports each step at its deadline.
    ///
    /// Arrivals are dispatched on a blocking thread, see [`Dispatcher::run_schedule`].
    /// Returns the time zero of the phase.
    async fn run_phase(
        &mut self,
        steps: Vec<ScheduledStep>,
        load_levels: Vec<u32>,
        phase: Phase,
    ) -> Result<LoadTestTime, LoadTestError> {
        let deadlines: Vec<_> = steps
            .iter()
            .map(|step| RelativeLoadTestTime::new(step.deadline))
            .collect();
        let rng = arrival_rng(self.state.seed, self.dispatcher.loadgenerator_id(), phase);
        let (sender, mut steps_done) = mpsc::unbounded_channel();
        let dispatcher = self.dispatcher.clone();
        let time_zero = LoadTestTime::now();
        let pacer = tokio::task::spawn_blocking(move || {
            dispatcher.run_schedule(steps, rng, time_zero, sender);
        });

        while let Some(StepDone {
            index,
            last_dispatch,
        }) = steps_done.recv().await
        {
            let deadline = deadlines[index];
            tokio::time::sleep_until(Instant::from_std((time_zero + deadline).into())).await;
            let report = IntervalReport::new(
                deadline,
                load_levels[index],
                self.tracker.interval_statistics(),
                last_dispatch,
            );
            self.framed
                .send(GeneratorUpdate::IntervalReport(report))
                .await?;
        }

        pacer.await.expect("pacer should not panic");
        Ok(time_zero)
    }

    pub async fn load_test(&mut self) -> Result<(), LoadTestError> {
        let steps = ScheduledStep::from_profile(&self.state.profile);
        let load_levels = self.state.profile.steps.iter().map(|s| s.count).collect();
        let time_zero = self.run_phase(steps, load_levels, Phase::Load).await?;

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

    #[test]
    fn arrival_rng_differs_per_load_generator_and_phase() {
        use rand::RngExt;

        let first = |id, phase| arrival_rng(5, LoadGeneratorId::new(id), phase).random::<u64>();

        assert_eq!(first(0, Phase::Load), first(0, Phase::Load));
        assert_ne!(first(0, Phase::Load), first(1, Phase::Load));
        assert_ne!(first(0, Phase::Load), first(0, Phase::Warmup));
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
            rate: 50,
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
            assert_eq!(report.load_level, 50);
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
