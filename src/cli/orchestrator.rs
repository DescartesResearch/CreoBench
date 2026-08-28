use clap::Parser;
use std::path::PathBuf;

use super::GeneratorAddr;

#[derive(Debug, Clone, Parser)]
#[command(name = "creo-orch", version)]
pub struct OrchestratorCli {
    #[arg(short = 'p', long)]
    pub profile: Option<PathBuf>,

    #[arg(short = 'l', long)]
    pub script: Option<PathBuf>,

    #[arg(short = 'r', long)]
    pub registry: Option<PathBuf>,

    #[arg(short = 'w', long)]
    pub warmup: Option<PathBuf>,

    #[arg(short = 'g', long = "generator", required = true)]
    pub generators: Vec<GeneratorAddr>,

    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    #[arg(short = 'u', long, value_parser = clap::value_parser!(u32).range(1..))]
    pub virtual_user_count: Option<u32>,

    #[arg(short = 't', long = "timeout")]
    pub timeout_ms: Option<u64>,

    #[arg(short = 's', long)]
    pub seed: Option<u64>,

    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub overwrite_outputs: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::assert_matches;
    use std::path::Path;

    #[test]
    fn defaults_optional_flags_to_none() {
        let cli = OrchestratorCli::try_parse_from(["orchestrator", "--generator", "10.0.0.1:8080"])
            .expect("parsing with only required flag should succeed");

        assert_eq!(cli.profile, None);
        assert_eq!(cli.script, None);
        assert_eq!(cli.registry, None);
        assert_eq!(cli.warmup, None);
        assert_eq!(
            cli.generators,
            vec!["10.0.0.1:8080".parse::<GeneratorAddr>().unwrap()]
        );
        assert_eq!(cli.output, None);
        assert_eq!(cli.virtual_user_count, None);
        assert_eq!(cli.timeout_ms, None);
        assert_eq!(cli.seed, None);
        assert!(!cli.overwrite_outputs);
    }

    #[test]
    fn parses_all_flags_correctly() {
        let cli = OrchestratorCli::try_parse_from([
            "orchestrator",
            "--profile",
            "my-profile.yaml",
            "--script",
            "my-script.lua",
            "--registry",
            "my-registry.yaml",
            "--warmup",
            "my-warmup.yaml",
            "--generator",
            "10.0.0.1:8080",
            "--generator",
            "10.0.0.2:8080",
            "--output",
            "/tmp/results",
            "--virtual-user-count",
            "50",
            "--timeout",
            "30000",
            "--seed",
            "42",
            "--overwrite-outputs",
        ])
        .expect("parsing should succeed");

        assert_eq!(
            cli.profile,
            Some(Path::new("my-profile.yaml").to_path_buf())
        );
        assert_eq!(cli.script, Some(Path::new("my-script.lua").to_path_buf()));
        assert_eq!(
            cli.registry,
            Some(Path::new("my-registry.yaml").to_path_buf())
        );
        assert_eq!(cli.warmup, Some(Path::new("my-warmup.yaml").to_path_buf()));
        assert_eq!(
            cli.generators,
            vec![
                "10.0.0.1:8080".parse::<GeneratorAddr>().unwrap(),
                "10.0.0.2:8080".parse::<GeneratorAddr>().unwrap(),
            ]
        );
        assert_eq!(cli.output, Some(Path::new("/tmp/results").to_path_buf()));
        assert_eq!(cli.virtual_user_count, Some(50));
        assert_eq!(cli.timeout_ms, Some(30000));
        assert_eq!(cli.seed, Some(42));
        assert!(cli.overwrite_outputs);
    }

    #[test]
    fn requires_generator_argument() {
        let err = OrchestratorCli::try_parse_from(["orchestrator"]).unwrap_err();
        assert_matches!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn accepts_multiple_generator_arguments() {
        let cli = OrchestratorCli::try_parse_from([
            "orchestrator",
            "--generator",
            "10.0.0.1:8080",
            "--generator",
            "10.0.0.2:8080",
            "--generator",
            "10.0.0.3:8080",
        ])
        .expect("multiple generators should be accepted");
        assert_eq!(cli.generators.len(), 3);
        assert_eq!(
            cli.generators[0],
            "10.0.0.1:8080".parse::<GeneratorAddr>().unwrap()
        );
        assert_eq!(
            cli.generators[1],
            "10.0.0.2:8080".parse::<GeneratorAddr>().unwrap()
        );
        assert_eq!(
            cli.generators[2],
            "10.0.0.3:8080".parse::<GeneratorAddr>().unwrap()
        );
    }

    #[test]
    fn fails_on_unknown_flag() {
        let err = OrchestratorCli::try_parse_from([
            "orchestrator",
            "--generator",
            "10.0.0.1:8080",
            "--bogus",
        ])
        .unwrap_err();
        assert_matches!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}
