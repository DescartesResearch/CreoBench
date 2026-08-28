/// Errors that can occur during config deserialization.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid UTF-8 input
    #[error("data must be valid UTF-8: input was valid up to `{0}` bytes")]
    InvalidUtf8(usize),
    /// A YAML parsing error.
    #[error("{0}")]
    Yaml(#[from] yaml_serde::Error),
    /// A CSV parsing error.
    #[error("failed to parse load step {step}: {source}")]
    Csv { step: usize, source: csv::Error },

    #[error(
        "load profile deadlines must be strictly increasing: step `{step}` has deadline `{deadline}`, previous was `{previous}`"
    )]
    NonMonotonicDeadline {
        step: usize,
        deadline: f64,
        previous: f64,
    },

    #[error("load profile must not be empty")]
    EmptyLoadProfile,

    #[error("service registry must not be empty")]
    EmptyServiceRegistry,
}

/// Alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;
