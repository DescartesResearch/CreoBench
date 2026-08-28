use std::sync::Arc;

use super::handle::GeneratorHandle;

use crate::tracker::IntervalReport;
use crate::wire::report::GeneratorUpdate;

#[derive(Debug, thiserror::Error)]
pub enum CollectError {
    #[error("load generator instance `{address}` disconnected mid-load-test")]
    InstanceCrashed { address: Arc<str> },
    #[error(
        "load generator instance `{address}` reported interval at target_time `{actual}`s, \
         but reference target_time was `{expected}`s"
    )]
    TimeMismatch {
        address: Arc<str>,
        expected: f64,
        actual: f64,
    },
}

pub async fn collect_reports(
    handles: &mut Vec<GeneratorHandle>,
) -> Result<Option<IntervalReport>, CollectError> {
    if handles.is_empty() {
        return Ok(None);
    }

    let mut round_reports: Vec<IntervalReport> = Vec::with_capacity(handles.len());

    let mut i = 0;
    while i < handles.len() {
        match handles[i].recv().await {
            Some(GeneratorUpdate::IntervalReport(r)) => {
                round_reports.push(r);
                i += 1;
            }
            Some(GeneratorUpdate::Finished) => {
                handles.swap_remove(i);
            }
            None => {
                let address = Arc::clone(handles[i].address());
                handles.swap_remove(i);
                return Err(CollectError::InstanceCrashed { address });
            }
        }
    }

    if round_reports.is_empty() {
        return Ok(None);
    }

    let expected = round_reports[0].target_time;
    let load_level: u32 = round_reports.iter().map(|r| r.load_level).sum();
    let final_batch_time = round_reports
        .iter()
        .filter_map(|r| r.final_batch_time)
        .max();

    if let Some((i, report)) = round_reports
        .iter()
        .enumerate()
        .find(|(_, r)| r.target_time != expected)
    {
        return Err(CollectError::TimeMismatch {
            address: Arc::clone(handles[i].address()),
            expected: expected.as_secs_f64(),
            actual: report.target_time.as_secs_f64(),
        });
    }
    let stats = round_reports.into_iter().map(|r| r.stats).sum();

    Ok(Some(IntervalReport::new(
        expected,
        load_level,
        stats,
        final_batch_time,
    )))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::load::RelativeLoadTestTime;
    use crate::orchestrator::phases::GeneratorHandle;
    use crate::tracker::{IntervalReport, IntervalStatistics};
    use crate::wire::report::GeneratorUpdate;

    use super::*;

    fn report_at(t: u64) -> GeneratorUpdate {
        GeneratorUpdate::IntervalReport(IntervalReport {
            target_time: RelativeLoadTestTime::new(Duration::from_secs(t)),
            load_level: 10,
            stats: Default::default(),
            final_batch_time: Some(RelativeLoadTestTime::new(Duration::from_secs(t))),
        })
    }

    fn handle_with(address: &str, messages: Vec<GeneratorUpdate>) -> GeneratorHandle {
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
        let addr: Arc<str> = address.to_string().into();
        for m in messages {
            tx.try_send(m).expect("test message fits in channel buffer");
        }
        drop(tx);
        GeneratorHandle::new(addr, rx, command_tx)
    }

    #[tokio::test]
    async fn collect_empty_handles_returns_none() {
        let mut handles = vec![];
        let result = collect_reports(&mut handles).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn collect_single_receiver_happy_path() {
        let mut handles = vec![handle_with(
            "10.0.0.1:8080",
            vec![
                report_at(1),
                report_at(2),
                report_at(3),
                GeneratorUpdate::Finished,
            ],
        )];

        let r1 = collect_reports(&mut handles)
            .await
            .expect("happy path should not error")
            .expect("handle still alive for first round");
        assert_eq!(
            r1.target_time,
            RelativeLoadTestTime::new(Duration::from_secs(1))
        );
        assert_eq!(handles.len(), 1);

        let r2 = collect_reports(&mut handles)
            .await
            .expect("happy path should not error")
            .expect("handle still alive for second round");
        assert_eq!(
            r2.target_time,
            RelativeLoadTestTime::new(Duration::from_secs(2))
        );

        let r3 = collect_reports(&mut handles)
            .await
            .expect("happy path should not error")
            .expect("handle still alive for third round");
        assert_eq!(
            r3.target_time,
            RelativeLoadTestTime::new(Duration::from_secs(3))
        );

        let r4 = collect_reports(&mut handles).await.expect(
            "explicit `Some(GeneratorUpdate::Finished)` was delivered before the channel closed",
        );
        assert!(
            r4.is_none(),
            "handle delivered `Some(GeneratorUpdate::Finished)` and was removed"
        );
        assert!(handles.is_empty());
    }

    #[tokio::test]
    async fn collect_three_receivers_matching_start_times() {
        let mut handles: Vec<GeneratorHandle> = (0..3)
            .map(|i| {
                let mut messages: Vec<GeneratorUpdate> = (1..=5).map(report_at).collect();
                messages.push(GeneratorUpdate::Finished);
                handle_with(&format!("10.0.0.{}:8080", i + 1), messages)
            })
            .collect();

        for t in 1..=5 {
            let r = collect_reports(&mut handles)
                .await
                .expect("happy path should not error")
                .expect("handles still alive");
            assert_eq!(
                r.target_time,
                RelativeLoadTestTime::new(Duration::from_secs(t))
            );
            assert_eq!(handles.len(), 3);
        }

        let r = collect_reports(&mut handles)
            .await
            .expect("explicit `Some(GeneratorUpdate::Finished)` was delivered by every receiver before the channels closed");
        assert!(r.is_none(), "all handles finished");
        assert!(handles.is_empty());
    }

    #[tokio::test]
    async fn collect_receiver_disconnects_returns_instance_crashed_error() {
        let addr: Arc<str> = "10.0.0.2:8080".to_string().into();
        let mut handles = vec![
            handle_with(
                "10.0.0.1:8080",
                vec![report_at(1), report_at(2), report_at(3)],
            ),
            handle_with("10.0.0.2:8080", vec![report_at(1)]),
        ];

        let r1 = collect_reports(&mut handles)
            .await
            .expect("both handles alive for first round")
            .expect("first round produces a report");
        assert_eq!(handles.len(), 2);
        assert_eq!(
            r1.target_time,
            RelativeLoadTestTime::new(Duration::from_secs(1))
        );

        let err = collect_reports(&mut handles)
            .await
            .expect_err("handle 10.0.0.2:8080 disconnected after its single report");
        match err {
            CollectError::InstanceCrashed { address } => assert_eq!(address, addr),
            other => panic!("expected `InstanceCrashed`, got `{other:?}`"),
        }
        assert!(
            !handles.iter().any(|h| **h.address() == *"10.0.0.2:8080"),
            "disconnected handle must be removed from the pool"
        );
    }

    #[tokio::test]
    async fn collect_time_mismatch_returns_error() {
        let offending: Arc<str> = "10.0.0.2:8080".to_string().into();
        let mut handles = vec![
            handle_with("10.0.0.1:8080", vec![report_at(1)]),
            handle_with("10.0.0.2:8080", vec![report_at(2)]),
        ];
        let result = collect_reports(&mut handles).await;
        match result {
            Err(CollectError::TimeMismatch {
                address,
                expected,
                actual,
            }) => {
                assert_eq!(address, offending);
                assert_eq!(expected, 1.0);
                assert_eq!(actual, 2.0);
            }
            other => panic!("expected structured `TimeMismatch`, got `{other:?}`"),
        }
    }

    #[tokio::test]
    async fn collect_partial_round_reports_are_discarded_on_error() {
        // Handle 1 reports once and disconnects; handle 2 is already gone.
        // The first call must short-circuit with `InstanceCrashed` — the report
        // pulled from handle 1 must not surface as a successful round.
        let mut handles = vec![
            handle_with("10.0.0.1:8080", vec![report_at(1), report_at(2)]),
            handle_with("10.0.0.2:8080", vec![]),
        ];

        let err = collect_reports(&mut handles)
            .await
            .expect_err("handle 2 disconnected");
        assert!(
            matches!(err, CollectError::InstanceCrashed { .. }),
            "the in-progress round's accumulated `round_reports` must be discarded on error, \
             and the failure surfaces as `InstanceCrashed`"
        );
    }

    #[tokio::test]
    async fn collect_aggregates_interval_statistics() {
        let mut stats_a = IntervalStatistics::default();
        stats_a.successful_transactions = 3;
        stats_a.failed_transactions = 1;

        let mut stats_b = IntervalStatistics::default();
        stats_b.successful_transactions = 5;
        stats_b.failed_transactions = 2;
        stats_b.timeout_transactions = 1;
        stats_b.dropped_transactions = 4;

        let report_a = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(1)),
            10,
            stats_a.clone(),
            Some(RelativeLoadTestTime::new(Duration::from_secs(1))),
        );
        let report_b = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(1)),
            20,
            stats_b.clone(),
            Some(RelativeLoadTestTime::new(Duration::from_secs(3))),
        );

        let mut handles = vec![
            handle_with(
                "10.0.0.1:8080",
                vec![GeneratorUpdate::IntervalReport(report_a)],
            ),
            handle_with(
                "10.0.0.2:8080",
                vec![GeneratorUpdate::IntervalReport(report_b)],
            ),
        ];

        let result = collect_reports(&mut handles).await.unwrap().unwrap();

        assert_eq!(result.load_level, 30);
        assert_eq!(
            result.final_batch_time,
            Some(RelativeLoadTestTime::new(Duration::from_secs(3)))
        );

        let expected_stats: IntervalStatistics = [stats_a, stats_b].into_iter().sum();
        assert_eq!(result.stats, expected_stats);
    }
}
