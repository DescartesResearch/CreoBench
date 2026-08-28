/// Errors that can occur parsing an HTTP request spec.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// The `method` field did not match any of the seven standard HTTP methods.
    #[error("invalid value for field `method`: unknown HTTP method: `{0}`")]
    UnknownMethod(String),
}
