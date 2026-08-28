mod common;

use std::assert_matches;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

use common::prelude::*;
use creo_bench::orchestrator::{
    self,
    phases::{CollectError, connect::ConnectError},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aborts_complete_connect_failure() {
    let (orchestrator, generators, _) = load_test(1).await.start();

    let crashed_addr = generators.last().unwrap().listen_addr().to_string();
    drop(generators);

    let err = orchestrator.start().await.expect_err();
    assert_matches!(
        err,
        orchestrator::Error::Connect(ConnectError::Failed { addr, .. }) if &*addr == crashed_addr.as_str()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aborts_partial_connect_failure() {
    let (orchestrator, mut generators, _) = load_test(2).await.start();

    let crashed_addr = generators.last().unwrap().listen_addr().to_string();
    drop(generators.pop().unwrap());

    let err = orchestrator.start().await.expect_err();
    assert_matches!(
        err,
        orchestrator::Error::Connect(ConnectError::Failed { addr, .. }) if &*addr == crashed_addr.as_str()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aborts_mid_load_test_connection_drop() {
    let (orchestrator, mut generators, _) = load_test(2).await.start();
    let crashed_addr = generators.last().unwrap().listen_addr().to_string();
    let intervals_csv = orchestrator.output_dir().join("interval.csv");

    let orchestrator = tokio::spawn(orchestrator.start());

    // Wait for file
    wait_until(|| intervals_csv.exists()).await;

    // Wait for first line
    let mut reader = BufReader::new(File::open(intervals_csv).unwrap()).lines();
    wait_until(|| reader.next().is_some()).await;
    drop(generators.pop().unwrap());

    let err = orchestrator.await.unwrap().expect_err();
    assert_matches!(
        err,
        orchestrator::Error::Collect(CollectError::InstanceCrashed {
            address
        }) if &*address == crashed_addr.as_str()
    );
}

async fn load_test(count: u8) -> LoadTest {
    let builder = LoadTestBuilder::new().await;
    builder
        .build(
            ScriptBuilder::default(),
            ProfileBuilder::new()
                .add_step(1.0, 1)
                .add_step(2.0, 1)
                .add_step(3.0, 1)
                .add_step(4.0, 1),
            WarmupBuilder::default(),
            RegistryBuilder::default(),
            count,
            1,
            43,
            Vec::new(),
        )
        .await
}

const TIMEOUT: Duration = Duration::from_millis(4000);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

async fn wait_until(mut condition: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < TIMEOUT && !condition() {
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
