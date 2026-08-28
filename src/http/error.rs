#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Client(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("{0}")]
    Timeout(String),
    #[error("{0}")]
    Failed(String),
}
