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

/// Virtual users per generator. More than one, so that requests of different
/// virtual users interleave.
const VIRTUAL_USERS: u32 = 4;

async fn run_load_test(count: u8) {
    let (orchestrator, generators, config) = load_test(count).await.start();

    let output = orchestrator.start().await.expect_ok();
    let intervals = output.interval_rows();
    let transactions = output.transaction_rows();

    assert_interval_structure(&intervals, &config);
    assert_results_are_conserved(&intervals, &transactions);
    assert_transactions_succeed(&transactions);
    assert_transactions_start_within_their_step(&transactions);
    assert_transactions_are_sent_after_their_start(&transactions);
    assert_every_generator_and_virtual_user_participates(&transactions, &config);
    assert_virtual_users_alternate_requests(&intervals, &transactions);
    assert_arrival_count_matches_rate(&transactions, &config);
    assert_inputs_are_copied(&output, &config);
    assert_run_metadata_describes_the_run(&output, &config);

    for generator in generators {
        generator.join().await.unwrap();
    }
}

fn rows_of<'a>(intervals: &'a [IntervalRow], phase: &str) -> Vec<&'a IntervalRow> {
    intervals.iter().filter(|row| row.phase == phase).collect()
}

/// Every phase reports one interval per second with the configured load level.
fn assert_interval_structure(intervals: &[IntervalRow], config: &LoadTestConfig) {
    let warmup_level = config.warmup_rate;
    let warmup = rows_of(intervals, "warmup");
    assert_eq!(warmup.len() as u32, config.warmup_duration);
    for (second, row) in (1..).zip(&warmup) {
        assert_eq!(row.target_time, f64::from(second));
        assert_eq!(row.load_level, warmup_level);
    }

    let pause = rows_of(intervals, "pause");
    assert_eq!(pause.len() as u32, config.warmup_pause);
    for (second, row) in (1..).zip(&pause) {
        assert_eq!(row.target_time, f64::from(second));
        assert_eq!(row.load_level, 0);
        assert_eq!(row.final_batch_time, None);
    }

    // Load steps may be followed by drain intervals while transactions complete.
    let load = rows_of(intervals, "load");
    assert!(load.len() >= config.step_counts.len());
    for (second, row) in (1..).zip(&load) {
        assert_eq!(row.target_time, f64::from(second));
    }
    let load_levels: Vec<_> = load.iter().map(|row| row.load_level).collect();
    let (steps, drain) = load_levels.split_at(config.step_counts.len());
    assert_eq!(steps, config.step_counts);
    assert!(
        drain.iter().all(|&level| level == 0),
        "drain levels {drain:?}"
    );
}

/// Every transaction is counted in exactly one interval.
fn assert_results_are_conserved(intervals: &[IntervalRow], transactions: &[TransactionRow]) {
    let counted: u64 = intervals.iter().map(result_count).sum();
    assert_eq!(counted, transactions.len() as u64);
}

fn result_count(row: &IntervalRow) -> u64 {
    row.successful_transactions
        + row.failed_transactions
        + row.timeout_transactions
        + row.dropped_transactions
}

/// Splits transactions into those reported before and during the load phase.
///
/// Both output files are written from the same stream of interval reports,
/// so the transactions of the warmup and pause intervals come first.
fn split_at_load_phase<'a>(
    intervals: &[IntervalRow],
    transactions: &'a [TransactionRow],
) -> (&'a [TransactionRow], &'a [TransactionRow]) {
    let before_load: u64 = intervals
        .iter()
        .filter(|row| row.phase != "load")
        .map(result_count)
        .sum();
    transactions.split_at(before_load as usize)
}

fn assert_transactions_succeed(transactions: &[TransactionRow]) {
    assert!(!transactions.is_empty());
    for row in transactions {
        assert_eq!(row.outcome, "success", "{row:?}");
        assert!(row.response_time_ms.is_some(), "{row:?}");
    }
}

/// All steps last one second, so every transaction starts within the second
/// before its target time.
fn assert_transactions_start_within_their_step(transactions: &[TransactionRow]) {
    for row in transactions {
        assert!(
            row.target_time - 1.0 <= row.start_time && row.start_time < row.target_time,
            "{row:?}"
        );
    }
}

/// A request is sent no earlier than its scheduled start, and no later than
/// its start plus its response time, which spans from start to completion.
fn assert_transactions_are_sent_after_their_start(transactions: &[TransactionRow]) {
    for row in transactions {
        let send_time = row.send_time.expect("successful transactions are sent");
        // Response times are whole milliseconds, so allow for rounding.
        let latest = row.start_time + (row.response_time_ms.unwrap() + 1) as f64 / 1000.0;
        assert!(
            row.start_time <= send_time && send_time <= latest,
            "{row:?}"
        );
    }
}

fn assert_every_generator_and_virtual_user_participates(
    transactions: &[TransactionRow],
    config: &LoadTestConfig,
) {
    let participants: std::collections::BTreeSet<_> = transactions
        .iter()
        .filter_map(|row| Some((row.load_generator_id, row.virtual_user_id?)))
        .collect();
    let expected: std::collections::BTreeSet<_> = (0..config.generator_count)
        .flat_map(|generator| (0..config.virtual_user_count).map(move |user| (generator, user)))
        .collect();
    assert_eq!(participants, expected);
}

/// Each virtual user executes its behavior loop in order, so its requests
/// alternate between both specs.
///
/// Rows are not written in execution order, since a virtual user can start its
/// next request before the previous result is logged. A virtual user sends one
/// request at a time, so ordering its rows by send time restores the execution
/// order. Send times are relative to the start of their phase, so each phase is
/// checked separately.
fn assert_virtual_users_alternate_requests(
    intervals: &[IntervalRow],
    transactions: &[TransactionRow],
) {
    let (before_load, load) = split_at_load_phase(intervals, transactions);
    for phase in [before_load, load] {
        let mut by_user = std::collections::BTreeMap::<_, Vec<_>>::new();
        for row in phase {
            by_user
                .entry((row.load_generator_id, row.virtual_user_id))
                .or_default()
                .push(row);
        }
        for rows in by_user.values_mut() {
            rows.sort_by(|a, b| a.send_time.unwrap().total_cmp(&b.send_time.unwrap()));
            for pair in rows.windows(2) {
                assert_ne!(pair[0].spec_id, pair[1].spec_id, "{pair:?}");
            }
        }
    }
}

/// The output directory contains the input files, byte for byte.
fn assert_inputs_are_copied(output: &OutputDir, config: &LoadTestConfig) {
    for (name, contents) in &config.inputs {
        let copy = std::fs::read_to_string(output.path().join(name)).unwrap();
        assert_eq!(&copy, contents, "{name}");
    }
}

fn assert_run_metadata_describes_the_run(output: &OutputDir, config: &LoadTestConfig) {
    let json = std::fs::read(output.path().join("run.json")).unwrap();
    let run: serde_json::Value = serde_json::from_slice(&json).unwrap();

    assert_eq!(run["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(run["valid"], true);
    assert_eq!(run["virtual_user_count"], config.virtual_user_count);
    assert_eq!(run["seed"], config.seed);
    let generators = run["generators"].as_array().unwrap();
    assert_eq!(generators.len(), usize::from(config.generator_count));
    for (id, generator) in generators.iter().enumerate() {
        assert_eq!(generator["id"], id);
        assert_eq!(generator["version"], env!("CARGO_PKG_VERSION"));
        assert!(generator["cores"].as_u64().unwrap() >= 1);
        assert!(generator["worker_threads"].as_u64().unwrap() >= 1);
    }

    assert!(run.get("drops").is_none(), "{run}");
}

/// Arrivals are Poisson-distributed, so the number of transactions is random.
/// It must lie within five standard deviations of its expected value.
fn assert_arrival_count_matches_rate(transactions: &[TransactionRow], config: &LoadTestConfig) {
    let warmup = config.warmup_duration * config.warmup_rate;
    let expected = f64::from(warmup + config.total_requests);
    let actual = transactions.len() as f64;
    assert!(
        (actual - expected).abs() < 5.0 * expected.sqrt(),
        "expected about {expected} transactions, got {actual}"
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
        .add_step(1.0, count as u32 * 200)
        .add_step(2.0, count as u32 * 300);
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
        .with_rate(count as u32 * 50)
        .with_pause(1);
    let registry = RegistryBuilder::default().with_service(SERVICE, builder.url());

    builder
        .build(
            script,
            profile,
            warmup,
            registry,
            count,
            VIRTUAL_USERS,
            5678,
            specs,
        )
        .await
}
