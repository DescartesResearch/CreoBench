use super::phases::{ConfigWaitError, LoadTestError, WaitForCommandError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to parse command line arguments: {0}")]
    Clap(#[from] clap::Error),

    #[error("failed to bind TCP listener: {0}")]
    Listen(#[source] std::io::Error),

    #[error("failed to accept orchestrator: {0}")]
    Accept(#[source] std::io::Error),

    #[error("{0}")]
    Config(#[from] ConfigWaitError),

    #[error("{0}")]
    Command(#[from] WaitForCommandError),

    #[error("received abort command from orchestrator")]
    Abort,

    #[error("{0}")]
    LoadTest(#[from] LoadTestError),
}

pub type Result<T> = std::result::Result<T, Error>;
