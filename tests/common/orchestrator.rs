//! Utilities for spawning an [`Orchestrator`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::{NamedTempFile, TempDir};

use creo_bench::cli::GeneratorAddr;
use creo_bench::config::{LoadProfileConfig, ServiceRegistryConfig, WarmupConfig};
use creo_bench::orchestrator::config::LoadTestConfig;
use creo_bench::orchestrator::io::FromFile;
use creo_bench::orchestrator::runner::LoadTestRunner;
use creo_bench::orchestrator::{self, Error};

use super::csv::{IntervalRow, TransactionRow, interval_rows, transaction_rows};

/// A orchestrator of a load test.
pub struct Orchestrator {
    profile: NamedTempFile,
    script: NamedTempFile,
    warmup: NamedTempFile,
    registry: NamedTempFile,
    output_dir: TempDir,
    generators: Vec<GeneratorAddr>,
    virtual_user_count: u32,
    seed: u64,
    timeout_ms: u64,
}

impl Orchestrator {
    /// Create a new [`Orchestrator`] from the four input files.
    pub fn new(
        script: NamedTempFile,
        profile: NamedTempFile,
        warmup: NamedTempFile,
        registry: NamedTempFile,
        generators: impl IntoIterator<Item = GeneratorAddr>,
    ) -> Self {
        Self {
            profile,
            script,
            warmup,
            registry,
            generators: generators.into_iter().collect(),
            output_dir: tempfile::tempdir().unwrap(),
            virtual_user_count: 100,
            seed: 5,
            timeout_ms: 5000,
        }
    }

    pub fn with_virtual_user_count(mut self, count: u32) -> Self {
        self.virtual_user_count = count;
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub fn output_dir(&self) -> &Path {
        self.output_dir.path()
    }

    /// Start the load test.
    pub async fn start(self) -> LoadTestResult {
        let profile = LoadProfileConfig::from_file(&self.profile).await.unwrap();
        let script = Arc::<str>::from_file(&self.script).await.unwrap();
        let warmup = WarmupConfig::from_file(&self.warmup).await.unwrap();
        let registry = ServiceRegistryConfig::from_file(&self.registry)
            .await
            .unwrap();

        let config = LoadTestConfig {
            profile,
            script,
            registry,
            warmup,
            generators: self.generators,
            output_dir: self.output_dir.path().to_path_buf(),
            virtual_user_count: self.virtual_user_count,
            seed: self.seed,
            timeout_ms: self.timeout_ms,
            overwrite_outputs: true,
        };

        let result = LoadTestRunner::new(config).run().await;

        LoadTestResult {
            result,
            output_dir: self.output_dir,
        }
    }
}

/// The result of a load test.
pub struct LoadTestResult {
    result: Result<(), Error>,
    output_dir: TempDir,
}

impl LoadTestResult {
    /// The temp output directory the [`Orchestrator`] wrote its artifacts into.
    pub fn output_dir(&self) -> &Path {
        self.output_dir.path()
    }

    /// Asserts the load test completed successfully.
    pub fn expect_ok(self) -> OutputDir {
        self.result.unwrap();
        OutputDir {
            dir: self.output_dir,
        }
    }

    /// Asserts the load test failed with an error.
    pub fn expect_err(self) -> orchestrator::Error {
        self.result.unwrap_err()
    }
}

/// The output directory of a successfully load test.
pub struct OutputDir {
    dir: TempDir,
}

impl OutputDir {
    /// The output directory path.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Reads all rows of `interval.csv`.
    pub fn interval_rows(&self) -> Vec<IntervalRow> {
        interval_rows(self.path().join("interval.csv"))
    }

    /// Reads all rows of `transactions.csv`.
    pub fn transaction_rows(&self) -> Vec<TransactionRow> {
        transaction_rows(self.path().join("transactions.csv"))
    }
}
