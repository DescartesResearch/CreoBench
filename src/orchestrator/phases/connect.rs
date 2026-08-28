use std::sync::Arc;

use super::configure::ConfigureHandle;

#[derive(Debug, thiserror::Error)]
pub enum ConnectAttemptError {
    #[error("connection attempt timed out after {}s", CONNECT_TIMEOUT.as_secs())]
    TimedOut,
    #[error("{source}")]
    Failed { source: std::io::Error },
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("failed to connect to `{addr}` after {attempt} attempts: {source}")]
    Failed {
        addr: Arc<str>,
        attempt: u32,
        source: ConnectAttemptError,
    },
}

pub struct ConnectHandle {
    addr: Arc<str>,
}

const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

impl ConnectHandle {
    pub fn new(addr: Arc<str>) -> Self {
        Self { addr }
    }

    pub async fn try_connect(&self) -> Result<tokio::net::TcpStream, ConnectAttemptError> {
        match tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio::net::TcpStream::connect(self.addr.as_ref()),
        )
        .await
        {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(e)) => Err(ConnectAttemptError::Failed { source: e }),
            Err(_) => Err(ConnectAttemptError::TimedOut),
        }
    }

    pub async fn connect(self) -> Result<ConfigureHandle<tokio::net::TcpStream>, ConnectError> {
        let max_retries = 5;
        let base_delay = std::time::Duration::from_millis(100);
        let max_delay = std::time::Duration::from_secs(5);

        for attempt in 0..=max_retries {
            match self.try_connect().await {
                Ok(stream) => {
                    return Ok(ConfigureHandle::new(self.addr, stream));
                }
                Err(source) if attempt == max_retries => {
                    return Err(ConnectError::Failed {
                        addr: self.addr.clone(),
                        attempt: attempt + 1,
                        source,
                    });
                }
                Err(err) => {
                    tracing::debug!(
                        "Connection attempt {}/{} to `{}` failed: {}. Retrying...",
                        attempt + 1,
                        max_retries + 1,
                        self.addr,
                        err,
                    );

                    let delay = std::cmp::min(base_delay * 2u32.pow(attempt), max_delay);

                    tracing::debug!("Retrying connection to `{}` in {:?}.", self.addr, delay,);

                    tokio::time::sleep(delay).await;
                }
            }
        }

        unreachable!("retry loop always returns");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn disconnected_new_stores_addr() {
        let d = ConnectHandle::new("10.0.0.1:8080".to_string().into());
        assert_eq!(d.addr.as_ref(), "10.0.0.1:8080");
    }

    #[tokio::test]
    async fn disconnected_connect_connects_to_tcp_listener() {
        let listener = TcpListener::bind("localhost:0").await.unwrap();
        let addr: Arc<str> = listener.local_addr().unwrap().to_string().into();

        let d = ConnectHandle::new(addr.clone());
        let connected = d.connect().await.expect("connect should succeed");
        assert_eq!(connected.addr(), addr.as_ref());
    }

    #[tokio::test(start_paused = true)]
    async fn disconnected_connect_returns_error_on_no_listener() {
        // Use an address that is very unlikely to have a listener.
        let d = ConnectHandle::new("127.0.0.1:1".to_string().into());
        let result = d.connect().await;
        assert!(result.is_err());
    }
}
