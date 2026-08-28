use tokio::net::{TcpListener, TcpStream};

use crate::load_generator::{Error, Result};

use super::ConfigWaitHandle;

pub async fn listen_and_wait(port: u16) -> Result<ConfigWaitHandle<TcpStream>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(Error::Listen)?;
    tracing::info!("Waiting for orchestrator to connect...");
    let (orchestrator, _) = listener.accept().await.map_err(Error::Accept)?;
    tracing::info!("Orchestrator connected!");
    Ok(ConfigWaitHandle::new(orchestrator))
}
