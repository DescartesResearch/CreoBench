use std::sync::Arc;

use super::start::StartHandle;

use crate::net::MessageFramer;
use crate::wire::configure::{ConfigMessage, ConfigResponse};

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{self, Framed};

#[derive(Debug, thiserror::Error)]
pub enum ConfigureError {
    #[error("failed to send config to `{address}`: {source}")]
    SendFailed {
        address: Arc<str>,
        source: <Framer as codec::Encoder<ConfigMessage>>::Error,
    },
    #[error("unexpected disconnect from `{address}`: did not receive a response")]
    UnexpectedDisconnect { address: Arc<str> },
    #[error("failed to receive a response from `{address}`: {source}")]
    ReceiveError {
        address: Arc<str>,
        source: <Framer as codec::Decoder>::Error,
    },
    #[error("{0}")]
    SetupFailed(String),
    #[error("load generator `{address}` was aborted during configuration")]
    Aborted { address: Arc<str> },
    #[error("unexpected response from `{address}`: {response:?}")]
    UnexpectedResponse {
        address: Arc<str>,
        response: ConfigResponse,
    },
}

type Framer = MessageFramer<ConfigMessage, ConfigResponse>;

#[derive(Debug)]
pub struct ConfigureHandle<S> {
    addr: Arc<str>,
    framed: Framed<S, Framer>,
}

impl<S> ConfigureHandle<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(addr: Arc<str>, stream: S) -> Self {
        let framed = Framed::new(stream, Framer::new());
        Self { addr, framed }
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub async fn configure(
        mut self,
        config: &ConfigMessage,
    ) -> Result<StartHandle<S>, ConfigureError> {
        self.framed
            .send(config.clone())
            .await
            .map_err(|source| ConfigureError::SendFailed {
                address: self.addr.clone(),
                source,
            })?;

        match self.framed.next().await {
            Some(Ok(response)) => match response {
                ConfigResponse::Ready => Ok(StartHandle::new(self.addr, self.framed.into_inner())),
                ConfigResponse::SetupFailed { reason } => Err(ConfigureError::SetupFailed(reason)),
                ConfigResponse::Aborted => Err(ConfigureError::Aborted { address: self.addr }),
            },
            Some(Err(source)) => Err(ConfigureError::ReceiveError {
                address: self.addr,
                source,
            }),
            None => Err(ConfigureError::UnexpectedDisconnect { address: self.addr }),
        }
    }

    pub async fn send_abort(mut self) -> Result<(), ConfigureError> {
        self.framed
            .send(ConfigMessage::Abort)
            .await
            .map_err(|source| ConfigureError::SendFailed {
                address: self.addr.clone(),
                source,
            })?;

        match self.framed.next().await {
            Some(Ok(response)) => match response {
                ConfigResponse::Aborted => Ok(()),
                unexpected => Err(ConfigureError::UnexpectedResponse {
                    address: self.addr,
                    response: unexpected,
                }),
            },
            Some(Err(source)) => Err(ConfigureError::ReceiveError {
                address: self.addr,
                source,
            }),
            None => Err(ConfigureError::UnexpectedDisconnect { address: self.addr }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::LoadGeneratorId;
    use crate::wire::configure::LoadGeneratorConfig;
    use crate::wire::{LoadProfile, ServiceRegistry, Warmup};
    use std::assert_matches;

    type LoadGeneratorFramer = MessageFramer<ConfigResponse, ConfigMessage>;

    #[tokio::test]
    async fn connected_new_stores_addr() {
        let c = ConfigureHandle::new(
            "10.0.0.1:8080".to_string().into(),
            tokio::io::duplex(1024).0,
        );
        assert_eq!(c.addr(), "10.0.0.1:8080");
    }

    #[tokio::test]
    async fn connected_configure_sends_configure_and_receives_ready_response() {
        let (client, server) = tokio::io::duplex(1024);

        let config = ConfigMessage::Config(LoadGeneratorConfig {
            profile: LoadProfile { steps: vec![] },
            script: String::new().into(),
            registry: ServiceRegistry::default(),
            warmup: Warmup {
                rate: 0,
                duration: 0,
                pause: 0,
            },
            virtual_user_count: 0,
            seed: 0,
            timeout_ms: 0,
            load_generator_id: LoadGeneratorId::new(0),
        });
        {
            let config = config.clone();
            tokio::spawn(async move {
                let mut framed = Framed::new(server, LoadGeneratorFramer::new());

                let decoded_config = framed
                    .next()
                    .await
                    .expect("should receive message")
                    .expect("decode ok");
                assert_eq!(config, decoded_config);

                framed
                    .send(ConfigResponse::Ready)
                    .await
                    .expect("should send response");
            });
        }

        let c = ConfigureHandle::new("10.0.0.1:8080".to_string().into(), client);
        let configured = c
            .configure(&config)
            .await
            .expect("configure should succeed");
        assert_eq!(configured.addr(), "10.0.0.1:8080");
    }

    #[tokio::test]
    async fn connected_send_abort_returns_ok_on_aborted_response() {
        let (client, server) = tokio::io::duplex(1024);

        tokio::spawn(async move {
            let mut framed = Framed::new(server, LoadGeneratorFramer::new());

            let decoded = framed
                .next()
                .await
                .expect("should receive message")
                .expect("decode ok");
            assert_eq!(decoded, ConfigMessage::Abort);

            framed
                .send(ConfigResponse::Aborted)
                .await
                .expect("should send response");
        });

        let c = ConfigureHandle::new("10.0.0.1:8080".to_string().into(), client);
        let result = c.send_abort().await;
        assert_matches!(result, Ok(()));
    }

    #[tokio::test]
    async fn connected_send_abort_returns_unexpected_response_on_ready_response() {
        let (client, server) = tokio::io::duplex(1024);

        tokio::spawn(async move {
            let mut framed = Framed::new(server, LoadGeneratorFramer::new());

            let decoded = framed
                .next()
                .await
                .expect("should receive message")
                .expect("decode ok");
            assert_eq!(decoded, ConfigMessage::Abort);

            framed
                .send(ConfigResponse::Ready)
                .await
                .expect("should send response");
        });

        let c = ConfigureHandle::new("10.0.0.1:8080".to_string().into(), client);
        let err = c.send_abort().await.unwrap_err();
        assert_matches!(
            err,
            ConfigureError::UnexpectedResponse {
                response: ConfigResponse::Ready,
                ..
            }
        );
    }

    #[tokio::test]
    async fn connected_send_abort_returns_unexpected_disconnect_on_drop() {
        let (client, server) = tokio::io::duplex(1024);

        tokio::spawn(async move {
            let mut framed = Framed::new(server, LoadGeneratorFramer::new());

            let _decoded = framed
                .next()
                .await
                .expect("should receive message")
                .expect("decode ok");
        });

        let c = ConfigureHandle::new("10.0.0.1:8080".to_string().into(), client);
        let err = c.send_abort().await.unwrap_err();
        assert_matches!(err, ConfigureError::UnexpectedDisconnect { .. });
    }

    #[tokio::test]
    async fn connected_configure_receives_setup_failed_response() {
        let (client, server) = tokio::io::duplex(1024);

        let config = ConfigMessage::Config(LoadGeneratorConfig {
            profile: LoadProfile { steps: vec![] },
            script: String::new().into(),
            registry: ServiceRegistry::default(),
            warmup: Warmup {
                rate: 0,
                duration: 0,
                pause: 0,
            },
            virtual_user_count: 0,
            seed: 0,
            timeout_ms: 0,
            load_generator_id: LoadGeneratorId::new(0),
        });
        {
            let config = config.clone();
            tokio::spawn(async move {
                let mut framed = Framed::new(server, LoadGeneratorFramer::new());

                let decoded_config = framed
                    .next()
                    .await
                    .expect("should receive message")
                    .expect("decode ok");
                assert_eq!(config, decoded_config);

                framed
                    .send(ConfigResponse::SetupFailed {
                        reason: "connection refused".to_string(),
                    })
                    .await
                    .expect("should send response");
            });
        }

        let c = ConfigureHandle::new("10.0.0.1:8080".to_string().into(), client);
        let err = c.configure(&config).await.unwrap_err();
        assert_matches!(
            err,
            ConfigureError::SetupFailed(reason) if reason == "connection refused"
        );
    }
}
