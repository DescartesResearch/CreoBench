use crate::load::ServiceTime;
use crate::script;

/// Errors that can occur while resolving a request URL against the
/// [`ServiceRegistry`][`crate::wire::ServiceRegistry`].
#[derive(Debug, thiserror::Error)]
pub enum UrlError {
    /// The script's `service` field does not name a registered service.
    #[error("service `{0}` is not in the service registry")]
    ServiceNotFound(String),

    /// The base URL for the service in the registry is not a parseable URL.
    #[error("invalid URL for service `{service}`: {source}")]
    InvalidUrl {
        service: String,
        #[source]
        source: url::ParseError,
    },
}

/// Errors that can occur while executing a HTTP request.
#[derive(Debug, thiserror::Error)]
pub enum HttpExecuteError {
    /// URL resolution against the service registry failed
    #[error("failed to resolve request URL: {0}")]
    Url(#[from] UrlError),

    /// The HTTP request timed out before receiving a response
    #[error("HTTP request timed out: {message}")]
    Timeout {
        message: String,
        service_time: ServiceTime,
    },

    /// The HTTP request failed
    #[error("HTTP request failed: {message}")]
    Failed {
        message: String,
        service_time: ServiceTime,
    },

    /// The HTTP response contained a non-successful status code
    #[error("response contains non-successful status `{code}`")]
    Status {
        code: u16,
        service_time: ServiceTime,
    },

    /// The [`ScriptRunner::run_http_extract`][`script::ScriptRunner::run_http_extract`]
    /// returned an error
    #[error("failed to script extract function: {source}")]
    Extract {
        source: script::Error,
        service_time: ServiceTime,
    },
}
