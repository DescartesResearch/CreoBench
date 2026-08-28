mod error;
pub mod phases;

pub use error::{Error, Result};

use crate::cli::LoadGeneratorCli;
use crate::wire::command::Command;
use tokio::io::{AsyncRead, AsyncWrite};

pub async fn run() -> Result<()> {
    crate::log::setup();

    let cli = crate::cli::parse_or_exit::<LoadGeneratorCli>(std::env::args())?;

    let stream = phases::listen_and_wait(cli.listen_port).await?.into_inner();

    serve(stream).await
}

pub async fn serve<S>(stream: S) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    tracing::info!("Waiting for configuration from orchestrator...");
    let mut ready_handle = phases::run_with_stream(stream).await?;
    tracing::info!("Received configuration and created VU pool!");

    match ready_handle.wait_for_command().await? {
        Command::Start => {
            tracing::info!("Received Start command from orchestrator!");
            ready_handle.warmup().await?;
            ready_handle.load_test().await?;

            // TODO: Output summary
        }
        Command::Abort => {
            return Err(Error::Abort);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use futures_util::{SinkExt, StreamExt};
    use tokio_util::codec::Framed;

    use super::*;
    use crate::net::MessageFramer;
    use crate::test_utils::prelude::*;
    use crate::wire::command::Command;
    use crate::wire::configure::{ConfigMessage, ConfigResponse, LoadGeneratorConfig};
    use crate::wire::report::GeneratorUpdate;

    async fn configure(server: &mut tokio::io::DuplexStream, config: LoadGeneratorConfig) {
        let mut framed = Framed::new(
            server,
            MessageFramer::<ConfigMessage, ConfigResponse>::new(),
        );

        framed
            .send(ConfigMessage::Config(config))
            .await
            .expect("should send config");

        let response = framed
            .next()
            .await
            .expect("should receive response")
            .expect("decode ok");

        assert_matches!(response, ConfigResponse::Ready);

        framed.into_inner();
    }

    #[tokio::test]
    async fn serve_runs_warmup_and_load_test_on_start_command() {
        let (client, server) = tokio::io::duplex(4096);
        let scenario = ScenarioBuilder::default().build().await;
        let config = super::phases::tests::load_generator_config(scenario.script, 3);

        let serve_task = tokio::spawn(async move { serve(client).await });

        let mut server = server;
        configure(&mut server, config).await;

        let mut framed = Framed::new(server, MessageFramer::<Command, GeneratorUpdate>::new());
        framed
            .send(Command::Start)
            .await
            .expect("should send start");

        let update = framed
            .next()
            .await
            .expect("should receive update")
            .expect("decode ok");
        assert_matches!(update, GeneratorUpdate::Finished);

        let result = serve_task.await.expect("serve task should complete");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn serve_returns_abort_error_on_abort_command() {
        let (client, server) = tokio::io::duplex(4096);
        let scenario = ScenarioBuilder::default().build().await;
        let config = super::phases::tests::load_generator_config(scenario.script, 3);

        let serve_task = tokio::spawn(async move { serve(client).await });

        let mut server = server;
        configure(&mut server, config).await;

        let mut framed = Framed::new(server, MessageFramer::<Command, GeneratorUpdate>::new());
        framed
            .send(Command::Abort)
            .await
            .expect("should send abort");

        let result = serve_task
            .await
            .expect("serve task should complete")
            .expect_err("serve should abort");

        assert_matches!(result, Error::Abort);
    }
}
