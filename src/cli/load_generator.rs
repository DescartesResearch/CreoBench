use clap::Parser;

use crate::cli::DEFAULT_GENERATOR_PORT;

#[derive(Debug, Clone, Parser)]
#[command(name = "creo-load", version)]
pub struct LoadGeneratorCli {
    #[arg(long, default_value_t = DEFAULT_GENERATOR_PORT)]
    pub listen_port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};
    use std::assert_matches;

    #[test]
    fn load_generator_cli_parses_listen_port() {
        let cli = LoadGeneratorCli::try_parse_from(["load-generator", "--listen-port", "9999"])
            .expect("parsing should succeed");
        assert_eq!(cli.listen_port, 9999);
    }

    #[test]
    fn load_generator_cli_applies_default_port() {
        let cli = LoadGeneratorCli::try_parse_from(["load-generator"])
            .expect("parsing without flags should succeed");
        assert_eq!(cli.listen_port, DEFAULT_GENERATOR_PORT);
    }

    #[test]
    fn load_generator_cli_errors_on_unknown_flag() {
        let err = LoadGeneratorCli::try_parse_from(["load-generator", "--bogus"]).unwrap_err();
        assert_matches!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn load_generator_cli_help_contains_listen_port() {
        let mut cmd = LoadGeneratorCli::command();
        let help = cmd.render_help().to_string();
        assert!(help.contains("--listen-port"));
        assert!(help.contains(&DEFAULT_GENERATOR_PORT.to_string()));
    }

    #[test]
    fn load_generator_cli_version_does_not_panic() {
        let cmd = LoadGeneratorCli::command();
        let _ = cmd.render_version();
    }
}
