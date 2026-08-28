mod error;
mod generator_addr;
mod load_generator;
mod orchestrator;

use clap::Parser;

pub use error::Error;
pub use generator_addr::{DEFAULT_GENERATOR_PORT, GeneratorAddr};
pub use load_generator::LoadGeneratorCli;
pub use orchestrator::OrchestratorCli;

pub(crate) fn parse_or_exit<C: Parser>(
    args: impl IntoIterator<Item = String>,
) -> Result<C, clap::Error> {
    match C::try_parse_from(args) {
        Ok(cli) => Ok(cli),
        Err(err)
            if matches!(
                err.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            err.exit()
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    #[test]
    fn parse_or_exit_returns_cli_for_valid_args() {
        let cli = parse_or_exit::<LoadGeneratorCli>(
            ["load-generator", "--listen-port", "9999"]
                .into_iter()
                .map(String::from),
        )
        .unwrap();
        assert_eq!(cli.listen_port, 9999);
    }

    #[test]
    fn parse_or_exit_surfaces_non_help_errors() {
        let err = parse_or_exit::<LoadGeneratorCli>(
            ["load-generator", "--bogus"].into_iter().map(String::from),
        )
        .unwrap_err();
        assert_matches!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}
