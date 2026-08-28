mod common;
use std::assert_matches;

use common::prelude::*;
use creo_bench::orchestrator;
use creo_bench::orchestrator::persist::PersistError;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aborts_on_persist_failure() {
    let (orchestrator, _generators, _) = LoadTestBuilder::new()
        .await
        .build(
            ScriptBuilder::default(),
            ProfileBuilder::new()
                .add_step(1.0, 1)
                .add_step(2.0, 1)
                .add_step(3.0, 1)
                .add_step(4.0, 1),
            WarmupBuilder::default(),
            RegistryBuilder::default(),
            1,
            1,
            43,
            Vec::new(),
        )
        .await
        .start();

    tokio::fs::create_dir(orchestrator.output_dir().join("interval.csv"))
        .await
        .unwrap();
    let err = orchestrator.start().await.expect_err();
    assert_matches!(err, orchestrator::Error::Persist(PersistError { .. }))
}
