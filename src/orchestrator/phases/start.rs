use std::sync::Arc;

use super::handle::GeneratorHandle;

use crate::net::MessageFramer;
use crate::wire::command::Command;
use crate::wire::report::GeneratorUpdate;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_util::codec::{self, Framed};

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("failed to send command to `{address}`: {source}")]
    SendFailed {
        address: Arc<str>,
        source: <Framer as codec::Encoder<Command>>::Error,
    },
}

type Framer = MessageFramer<Command, GeneratorUpdate>;

#[derive(Debug)]
pub struct StartHandle<S> {
    addr: Arc<str>,
    framed: Framed<S, Framer>,
}

impl<S> StartHandle<S>
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

    pub async fn start(mut self, cmd: Command) -> Result<GeneratorHandle, StartError> {
        self.framed
            .send(cmd)
            .await
            .map_err(|source| StartError::SendFailed {
                address: self.addr.clone(),
                source,
            })?;

        let (tx, rx) = mpsc::channel(1);
        let (command_tx, mut command_rx) = mpsc::channel(1);
        let (mut sink, mut stream) = self.framed.split();
        tokio::spawn(async move {
            while let Some(Ok(payload)) = stream.next().await {
                let is_finished = matches!(payload, GeneratorUpdate::Finished);
                if tx.send(payload).await.is_err() {
                    break;
                }
                if is_finished {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                if sink.send(command).await.is_err() {
                    break;
                }
            }
        });

        Ok(GeneratorHandle::new(self.addr, rx, command_tx))
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::time::Duration;

    use crate::load::RelativeLoadTestTime;
    use crate::tracker::IntervalReport;

    use super::*;

    type LoadGeneratorFramer = MessageFramer<GeneratorUpdate, Command>;

    #[tokio::test]
    async fn configured_new_stores_addr() {
        let stream = tokio::io::duplex(1024).0;
        let c = StartHandle::new("10.0.0.1:8080".to_string().into(), stream);
        assert_eq!(c.addr(), "10.0.0.1:8080");
    }

    #[tokio::test]
    async fn configured_start_sends_start_and_receives_reports_then_finished() {
        let (client, server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let mut framed = Framed::new(server, LoadGeneratorFramer::new());

            let cmd = framed
                .next()
                .await
                .expect("should receive")
                .expect("decode ok");
            assert_eq!(cmd, Command::Start);

            let report1 = IntervalReport {
                target_time: RelativeLoadTestTime::new(Duration::from_secs(1)),
                load_level: 10,
                stats: Default::default(),
                final_batch_time: None,
            };
            framed
                .send(GeneratorUpdate::IntervalReport(report1))
                .await
                .expect("should send report");

            let report2 = IntervalReport {
                target_time: RelativeLoadTestTime::new(Duration::from_secs(2)),
                load_level: 10,
                stats: Default::default(),
                final_batch_time: None,
            };
            framed
                .send(GeneratorUpdate::IntervalReport(report2))
                .await
                .expect("should send report");

            framed
                .send(GeneratorUpdate::Finished)
                .await
                .expect("should send finished");
        });

        let c = StartHandle::new("10.0.0.1:8080".to_string().into(), client);
        let mut handle = c.start(Command::Start).await.expect("start should succeed");
        assert_eq!(**handle.address(), *"10.0.0.1:8080");

        let r1 = handle.recv().await;
        assert_matches!(r1, Some(GeneratorUpdate::IntervalReport(_)));
        let r2 = handle.recv().await;
        assert_matches!(r2, Some(GeneratorUpdate::IntervalReport(_)));

        let finished = handle.recv().await;
        assert_matches!(finished, Some(GeneratorUpdate::Finished));

        let after_finished = handle.recv().await;
        assert!(after_finished.is_none());
    }

    #[tokio::test]
    async fn configured_start_send_abort_reaches_the_peer() {
        let (client, server) = tokio::io::duplex(4096);

        let peer = tokio::spawn(async move {
            let mut framed = Framed::new(server, LoadGeneratorFramer::new());

            let cmd = framed
                .next()
                .await
                .expect("should receive")
                .expect("decode ok");
            assert_eq!(cmd, Command::Start);

            let cmd = framed
                .next()
                .await
                .expect("should receive abort")
                .expect("decode ok");
            assert_eq!(cmd, Command::Abort);
        });

        let c = StartHandle::new("10.0.0.1:8080".to_string().into(), client);
        let handle = c.start(Command::Start).await.expect("start should succeed");
        handle
            .send_abort()
            .await
            .expect("send_abort should succeed");

        peer.await.expect("peer should receive the abort");
    }

    #[tokio::test]
    async fn configured_start_returns_send_failed_when_peer_closes_stream() {
        let (client, server) = tokio::io::duplex(4096);
        drop(server);

        let addr: Arc<str> = "10.0.0.1:8080".to_string().into();
        let c = StartHandle::new(addr.clone(), client);
        let err = c
            .start(Command::Start)
            .await
            .expect_err("start should fail");

        assert_matches!(
            err,
            StartError::SendFailed { address, .. } if address == addr
        );
    }
}
