pub mod config;
mod distribute;
mod error;
pub mod io;
pub mod persist;
pub mod phases;
pub mod runner;

pub use config::LoadTestConfig;
pub use distribute::{distribute_count, distribute_profile};
pub use error::{ConfigError, Error, OutputError, Result};
pub use phases::ConnectHandle;

use crate::cli::OrchestratorCli;

pub async fn run() -> Result<()> {
    crate::log::setup();

    let cli = crate::cli::parse_or_exit::<OrchestratorCli>(std::env::args())?;

    let config = LoadTestConfig::from_cli(cli).await?;

    runner::LoadTestRunner::new(config).run().await
}
