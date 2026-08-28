mod configure;
mod connect;
mod ready;

use tokio::io::{AsyncRead, AsyncWrite};

pub use configure::{ConfigWaitError, ConfigWaitHandle};
pub use connect::listen_and_wait;
pub use ready::{LoadTestError, ReadyHandle, WaitForCommandError};

use crate::http::ReqwestHttpClient;

pub async fn run_with_stream<S>(
    stream: S,
) -> Result<ReadyHandle<S, ReqwestHttpClient, rand::rngs::StdRng>, ConfigWaitError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let handle = ConfigWaitHandle::new(stream);
    handle.process_config().await
}

#[cfg(test)]
pub(crate) mod tests {
    use std::assert_matches;
    use std::sync::Arc;

    use futures_util::{SinkExt, StreamExt};
    use tokio_util::codec::Framed;

    use super::*;
    use crate::net::MessageFramer;
    use crate::test_utils::prelude::*;
    use crate::transaction::LoadGeneratorId;
    use crate::wire::configure::{ConfigMessage, ConfigResponse, LoadGeneratorConfig};
    use crate::wire::{LoadProfile, ServiceRegistry, Warmup};

    pub(crate) fn load_generator_config(
        script: impl Into<Arc<str>>,
        virtual_user_count: u32,
    ) -> LoadGeneratorConfig {
        LoadGeneratorConfig {
            profile: LoadProfile { steps: vec![] },
            script: script.into(),
            registry: ServiceRegistry::default(),
            warmup: Warmup {
                rate: 0,
                duration: 0,
                pause: 0,
            },
            virtual_user_count,
            seed: 0,
            timeout_ms: 0,
            load_generator_id: LoadGeneratorId::new(0),
        }
    }

    #[tokio::test]
    async fn run_with_stream_returns_aborted_on_abort_message() {
        let (client, server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let mut framed = Framed::new(
                server,
                MessageFramer::<ConfigMessage, ConfigResponse>::new(),
            );

            framed
                .send(ConfigMessage::Abort)
                .await
                .expect("should send abort");

            let response = framed
                .next()
                .await
                .expect("should receive response")
                .expect("decode ok");

            assert_matches!(response, ConfigResponse::Aborted);
        });

        let err = run_with_stream(client)
            .await
            .expect_err("run_with_stream should abort");

        assert_matches!(err, ConfigWaitError::Aborted);
    }

    #[tokio::test]
    async fn run_with_stream_returns_ready_handle_on_success() {
        let (client, server) = tokio::io::duplex(4096);

        let scenario = ScenarioBuilder::default().build().await;
        let config = load_generator_config(scenario.script, 3);
        let config = ConfigMessage::Config(config.clone());
        tokio::spawn(async move {
            let mut framed = Framed::new(
                server,
                MessageFramer::<ConfigMessage, ConfigResponse>::new(),
            );

            framed.send(config).await.expect("should send config");

            let response = framed
                .next()
                .await
                .expect("should receive response")
                .expect("decode ok");

            assert_matches!(response, ConfigResponse::Ready);
        });

        let _ = run_with_stream(client)
            .await
            .expect("run_with_stream should succeed");
    }
}
