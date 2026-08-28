use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("invalid IPv6 address format, expected [IPv6] or [IPv6]:PORT, but got `{0}`")]
    InvalidIPv6Format(String),

    #[error("invalid port: {0}")]
    InvalidPort(String),
}
