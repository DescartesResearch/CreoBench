//! Utilities for spawning [`GeneratorInstance`]s.
use tokio::net::TcpListener;

use creo_bench::cli::GeneratorAddr;
use creo_bench::load_generator;

/// A load generator instance.
///
/// Dropping the instance aborts the generator task, if it is still running,
/// effectively crashing the instance.
pub struct GeneratorInstance {
    addr: GeneratorAddr,
    join: Option<tokio::task::JoinHandle<load_generator::Result<()>>>,
}

impl GeneratorInstance {
    /// Spawns a new load generator instance
    pub async fn spawn() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener
            .local_addr()
            .expect("listener must have a local address")
            .to_string()
            .parse()
            .expect("listener address must parse as a GeneratorAddr");
        let join = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            load_generator::serve(stream).await
        });
        Self {
            addr,
            join: Some(join),
        }
    }

    /// The listen address the load generator accepts connections on
    pub fn listen_addr(&self) -> &GeneratorAddr {
        &self.addr
    }

    /// Waits for the load generator instance to complete.
    pub async fn join(mut self) -> load_generator::Result<()> {
        let join = self.join.take().unwrap();
        join.await.unwrap()
    }
}

impl Drop for GeneratorInstance {
    fn drop(&mut self) {
        if let Some(join) = &self.join {
            join.abort();
        }
    }
}
