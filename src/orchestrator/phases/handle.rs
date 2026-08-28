use std::sync::Arc;

use tokio::sync::mpsc;

use crate::wire::command::Command;
use crate::wire::report::GeneratorUpdate;

#[derive(Debug, thiserror::Error)]
pub enum SendCommandError {
    #[error("failed to send command to `{address}`: load generator is no longer connected")]
    Disconnected { address: Arc<str> },
}

#[derive(Debug)]
pub struct GeneratorHandle {
    address: Arc<str>,
    receiver: mpsc::Receiver<GeneratorUpdate>,
    command_tx: mpsc::Sender<Command>,
}

impl GeneratorHandle {
    pub(crate) fn new(
        address: Arc<str>,
        receiver: mpsc::Receiver<GeneratorUpdate>,
        command_tx: mpsc::Sender<Command>,
    ) -> Self {
        Self {
            address,
            receiver,
            command_tx,
        }
    }

    pub fn address(&self) -> &Arc<str> {
        &self.address
    }

    pub async fn recv(&mut self) -> Option<GeneratorUpdate> {
        self.receiver.recv().await
    }

    pub async fn send_abort(&self) -> Result<(), SendCommandError> {
        self.command_tx
            .send(Command::Abort)
            .await
            .map_err(|_| SendCommandError::Disconnected {
                address: self.address.clone(),
            })
    }
}
