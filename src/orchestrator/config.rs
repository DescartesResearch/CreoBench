use std::sync::Arc;

use crate::cli::{GeneratorAddr, OrchestratorCli};
use crate::config::{LoadProfileConfig, ServiceRegistryConfig, WarmupConfig};

use super::Error;
use super::io::FromFile;

#[derive(Debug, Clone)]
pub struct LoadTestConfig {
    pub profile: LoadProfileConfig,
    pub script: Arc<str>,
    pub registry: ServiceRegistryConfig,
    pub warmup: WarmupConfig,
    pub generators: Vec<GeneratorAddr>,
    pub output_dir: std::path::PathBuf,
    pub virtual_user_count: u32,
    pub seed: u64,
    pub timeout_ms: u64,
    pub overwrite_outputs: bool,
}

impl LoadTestConfig {
    pub async fn from_cli(cli: OrchestratorCli) -> Result<Self, Error> {
        if cli.generators.len() > u8::MAX as usize {
            return Err(Error::LoadGeneratorOverflow(cli.generators.len()));
        }

        let profile =
            LoadProfileConfig::from_file(cli.profile.unwrap_or_else(|| "profile.csv".into()))
                .await?;
        let script =
            Arc::<str>::from_file(cli.script.unwrap_or_else(|| "script.lua".into())).await?;
        let registry = ServiceRegistryConfig::from_file(
            cli.registry.unwrap_or_else(|| "registry.yaml".into()),
        )
        .await?;
        let warmup =
            WarmupConfig::from_file(cli.warmup.unwrap_or_else(|| "warmup.yaml".into())).await?;

        Ok(LoadTestConfig {
            profile,
            script,
            registry,
            warmup,
            generators: cli.generators,
            output_dir: cli.output.unwrap_or_else(|| "./results".into()),
            virtual_user_count: cli.virtual_user_count.unwrap_or(100),
            seed: cli.seed.unwrap_or(5),
            timeout_ms: cli.timeout_ms.unwrap_or(0),
            overwrite_outputs: cli.overwrite_outputs,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::LoadTestConfig;
    use crate::cli::OrchestratorCli;
    use crate::orchestrator::{ConfigError, Error};

    fn orchestrator_cli(count: usize) -> OrchestratorCli {
        OrchestratorCli {
            profile: None,
            script: None,
            registry: None,
            warmup: None,
            generators: (0..count)
                .map(|i| format!("10.0.0.{i}:8080").parse().unwrap())
                .collect(),
            output: None,
            virtual_user_count: None,
            timeout_ms: None,
            seed: None,
            overwrite_outputs: false,
        }
    }

    #[tokio::test]
    async fn rejects_generator_count_exceeding_maximum_limit() {
        let result = LoadTestConfig::from_cli(orchestrator_cli(256)).await;
        assert_matches!(result, Err(Error::LoadGeneratorOverflow(256)));
    }

    #[tokio::test]
    async fn accepts_generator_count_at_maximum_limit() {
        let result = LoadTestConfig::from_cli(orchestrator_cli(255)).await;
        assert!(!matches!(result, Err(Error::LoadGeneratorOverflow(_))));
    }

    #[tokio::test]
    async fn from_cli_with_defaults_names_default_paths_on_failure() {
        let result = LoadTestConfig::from_cli(orchestrator_cli(1)).await;
        assert_matches!(
            result,
            Err(Error::Config(ConfigError::FileOpen { path, .. }))
                if path == std::path::Path::new("profile.csv")
        );
    }
}
