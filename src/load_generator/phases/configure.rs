use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{self, Framed};

use super::ready::{ReadyHandle, ReadyState};
use crate::dispatch::Dispatcher;
use crate::http::ReqwestHttpClient;
use crate::net::MessageFramer;
use crate::tracker::Tracker;
use crate::virtual_user::{self, Pool};
use crate::wire::LoadGeneratorConfig;
use crate::wire::configure::{ConfigMessage, ConfigResponse};

#[derive(Debug, thiserror::Error)]
pub enum ConfigWaitError {
    #[error("failed to send config response to orchestrator: {0}")]
    SendFailed(<Framer as codec::Encoder<ConfigResponse>>::Error),
    #[error("unexpected disconnect from orchestrator: did not receive a load-generator config")]
    UnexpectedDisconnect,
    #[error("failed to receive load-generator config from: {source}")]
    ReceiveError {
        source: <Framer as codec::Decoder>::Error,
    },
    #[error("setup request failed for virtual users: {source}")]
    SetupFailed { source: virtual_user::Error },
    #[error("orchestrator sent abort signal during configuration")]
    Aborted,
}

type Framer = MessageFramer<ConfigResponse, ConfigMessage>;

#[derive(Debug)]
pub struct ConfigWaitHandle<S> {
    framed: Framed<S, Framer>,
}

impl<S> ConfigWaitHandle<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(stream: S) -> Self {
        let framed = Framed::new(stream, Framer::new());
        Self { framed }
    }

    pub fn into_inner(self) -> S {
        self.framed.into_inner()
    }

    pub async fn process_config(
        mut self,
    ) -> Result<ReadyHandle<S, ReqwestHttpClient, rand::rngs::StdRng>, ConfigWaitError> {
        let msg = match self.framed.next().await {
            Some(Ok(msg)) => msg,
            Some(Err(source)) => {
                return Err(ConfigWaitError::ReceiveError { source });
            }
            None => {
                return Err(ConfigWaitError::UnexpectedDisconnect);
            }
        };

        let LoadGeneratorConfig {
            profile,
            script,
            registry,
            virtual_user_count,
            load_generator_id,
            warmup,
            seed,
            timeout_ms,
        } = match msg {
            ConfigMessage::Config(config) => config,
            ConfigMessage::Abort => {
                self.framed
                    .send(ConfigResponse::Aborted)
                    .await
                    .map_err(ConfigWaitError::SendFailed)?;
                return Err(ConfigWaitError::Aborted);
            }
        };

        let pool = match Pool::new(script, registry, virtual_user_count, timeout_ms, seed).await {
            Ok(pool) => pool,
            Err(source) => {
                let reason = source.to_string();
                self.framed
                    .send(ConfigResponse::SetupFailed { reason })
                    .await
                    .map_err(ConfigWaitError::SendFailed)?;
                return Err(ConfigWaitError::SetupFailed { source });
            }
        };

        self.framed
            .send(ConfigResponse::Ready)
            .await
            .map_err(ConfigWaitError::SendFailed)?;

        let inner = self.framed.into_inner();

        let tracker = Tracker::new();
        let dispatcher = Dispatcher::new(load_generator_id, pool, tracker.clone());

        Ok(ReadyHandle::new(
            ReadyState {
                profile,
                warmup,
                seed,
            },
            inner,
            dispatcher,
            tracker,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;
    use crate::test_utils::prelude::*;

    #[tokio::test]
    async fn process_config_returns_ready_handle_on_success() {
        let (client, server) = tokio::io::duplex(4096);

        let scenario = ScenarioBuilder::default().build().await;
        let load_generator_config = super::super::tests::load_generator_config(scenario.script, 3);

        let config = ConfigMessage::Config(load_generator_config.clone());
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

        let handle = ConfigWaitHandle::new(client);
        let _ = handle
            .process_config()
            .await
            .expect("process_config should succeed");
    }

    #[tokio::test]
    async fn process_config_returns_setup_failed_on_invalid_script() {
        let (client, server) = tokio::io::duplex(4096);

        let config = ConfigMessage::Config(super::super::tests::load_generator_config(
            "not valid lua",
            1,
        ));

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

            assert_matches!(response, ConfigResponse::SetupFailed { .. });
        });

        let handle = ConfigWaitHandle::new(client);
        let err = handle
            .process_config()
            .await
            .expect_err("process_config should fail");

        assert_matches!(err, ConfigWaitError::SetupFailed { .. });
    }

    #[tokio::test]
    async fn process_config_returns_setup_failed_on_extract_error() {
        let (client, server) = tokio::io::duplex(4096);

        let setup_entry = RequestBuilder::r#static("POST", "auth", "/login")
            .with_extract("function(store, response) error(\"extract failed intentionally\") end")
            .build();
        let request_entry = RequestBuilder::r#static("POST", "service-1", "/create").build();

        let script = ScriptBuilder::new()
            .with_setup(&[setup_entry])
            .with_requests(&[request_entry])
            .build();

        let config = ConfigMessage::Config(super::super::tests::load_generator_config(script, 1));

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

            assert_matches!(response, ConfigResponse::SetupFailed { .. });
        });

        let handle = ConfigWaitHandle::new(client);
        let err = handle
            .process_config()
            .await
            .expect_err("process_config should fail");

        assert_matches!(err, ConfigWaitError::SetupFailed { .. });
    }

    #[tokio::test]
    async fn process_config_returns_unexpected_disconnect() {
        let (client, server) = tokio::io::duplex(4096);
        let handle = ConfigWaitHandle::new(client);
        drop(server);

        let err = handle
            .process_config()
            .await
            .expect_err("process_config should fail");

        assert_matches!(err, ConfigWaitError::UnexpectedDisconnect);
    }

    #[tokio::test]
    async fn process_config_returns_aborted_on_abort_message() {
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

        let handle = ConfigWaitHandle::new(client);
        let err = handle
            .process_config()
            .await
            .expect_err("process_config should abort");

        assert_matches!(err, ConfigWaitError::Aborted);
    }
}
