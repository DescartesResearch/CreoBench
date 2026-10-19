mod common;

use common::prelude::*;

const SERVICE: &str = "service-1";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn runs_single_generator() {
    run_load_test(1).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn runs_multiple_generators() {
    run_load_test(4).await
}

async fn run_load_test(count: u8) {
    let (orchestrator, generators, config) = load_test(count).await.start();

    let output = orchestrator.start().await.expect_ok();

    assert_intervals(&output, &config, count);

    assert_transactions(&output, count);

    for generator in generators {
        generator.join().await.unwrap();
    }
}

fn assert_intervals(output: &OutputDir, config: &LoadTestConfig, count: u8) {
    let intervals = output.interval_rows();

    for (phase, expected) in [
        ("warmup", config.warmup_duration),
        ("pause", config.warmup_pause),
        ("load", config.interval_count as u32),
    ] {
        let actual = intervals.iter().filter(|row| row.phase == phase).count() as u32;
        assert_eq!(
            actual, expected,
            "phase={phase}, expected={expected}, actual={actual}"
        );
    }
    for (phase, expected) in [
        (
            "warmup",
            config.warmup_duration * config.warmup_rate * count as u32,
        ),
        ("pause", 0),
        ("load", config.total_requests),
    ] {
        let actual = intervals
            .iter()
            .filter(|row| row.phase == phase)
            .map(|row| row.load_level)
            .sum::<u32>();
        assert_eq!(
            actual, expected,
            "phase={phase}, expected={expected}, actual={actual}"
        );
    }
    let paused = intervals
        .iter()
        .filter(|row| row.phase == "pause")
        .map(|row| row.successful_transactions)
        .sum::<u64>();
    assert_eq!(paused, 0);
}

fn assert_transactions(output: &OutputDir, count: u8) {
    let transactions = output.transaction_rows();
    // Arrivals are Poisson-distributed, so only the target load level is exact,
    // not the number of transactions.
    assert!(!transactions.is_empty());
    let successful = output
        .interval_rows()
        .iter()
        .map(|row| row.successful_transactions)
        .sum::<u64>();
    assert_eq!(successful, transactions.len() as u64);
    assert!(transactions.iter().all(|row| row.outcome == "success"));
    assert!(
        transactions
            .iter()
            .all(|row| row.target_time > row.start_time)
    );
    assert!(
        transactions
            .iter()
            .all(|row| row.response_time_ms.is_some())
    );
    // Each generator has a single virtual user that alternates between both
    // requests, so their counts differ by at most one per generator.
    let gets = transactions
        .iter()
        .filter(|row| row.spec_id.unwrap() == 0)
        .count();
    let posts = transactions
        .iter()
        .filter(|row| row.spec_id.unwrap() == 1)
        .count();
    assert!(
        gets.abs_diff(posts) <= count as usize,
        "gets={gets}, posts={posts}"
    );
}

async fn load_test(count: u8) -> LoadTest {
    assert!(count >= 1, "needs at least one generator");
    let builder = LoadTestBuilder::new().await;

    let requests = vec![
        RequestBuilder::r#static("GET", SERVICE, "/get")
            .with_query(vec![("n", "3"), ("q", "search")]),
        RequestBuilder::r#static("POST", SERVICE, "/post")
            .with_headers(vec![("Accept", "application/json")])
            .with_body(serde_json::json!({"key": "value"})),
    ];
    let profile = ProfileBuilder::new()
        .add_step(1.0, count as u32 * 4)
        .add_step(2.0, count as u32 * 6);
    let specs = requests
        .iter()
        .enumerate()
        .map(|(id, r)| Spec {
            id: id as u32,
            method: r.method(),
            path: r.path(),
            service: r.service(),
            body: r.body().cloned(),
            headers: r.headers().to_vec(),
            query: r.query().to_vec(),
        })
        .collect();

    let script = ScriptBuilder::new().with_requests(
        &requests
            .into_iter()
            .map(|request| request.build())
            .collect::<Vec<_>>(),
    );
    let warmup = WarmupBuilder::default()
        .with_duration(2)
        .with_rate(2)
        .with_pause(1);
    let registry = RegistryBuilder::default().with_service(SERVICE, builder.url());

    builder
        .build(script, profile, warmup, registry, count, 1, 5678, specs)
        .await
}
