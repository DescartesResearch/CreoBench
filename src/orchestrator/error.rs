use std::path::PathBuf;

use super::phases::connect::ConnectError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to parse command line arguments: {0}")]
    Clap(#[from] clap::Error),

    #[error("{0}")]
    Config(#[from] ConfigError),

    #[error("{0}")]
    Output(#[from] OutputError),

    #[error("{0}")]
    Connect(#[from] ConnectError),

    #[error("{0}")]
    Configure(#[from] super::phases::configure::ConfigureError),

    #[error("{0}")]
    Start(#[from] super::phases::start::StartError),

    #[error("{0}")]
    Collect(#[from] super::phases::collect::CollectError),

    #[error("{0}")]
    Persist(#[from] super::persist::PersistError),

    #[error("load test aborted")]
    Abort,

    #[error(
        "the orchestrator can only drive `{max}` load generator instances, but `{0}` were given", max=u8::MAX
    )]
    LoadGeneratorOverflow(usize),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to open `{path}`: {source}")]
    FileOpen {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config `{path}`: {source}")]
    ParseError {
        path: PathBuf,
        source: crate::config::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("failed to create output directory `{path}`: {source}")]
    CreateOutputDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read output directory `{path}`: {source}")]
    DirectoryRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("output path `{0}` exists but is not a directory")]
    NotADirectory(PathBuf),
    #[error(
        "output directory `{0}` is not empty: pass the `--overwrite-outputs` if you wish to ignore non-empty output directories"
    )]
    NotEmpty(PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;
