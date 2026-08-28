use super::{FromBytes, Result};

/// Configuration for the warmup phase that runs before the main load test.
///
/// The warmup gradually ramps up traffic to the target rate to let the
/// system stabilise before the load test begins.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[cfg_attr(any(test, feature = "test-utils"), derive(serde::Serialize, Default))]
pub struct WarmupConfig {
    /// The request rate of the warmup phase.
    pub rate: u32,
    /// The duration of the warmup phase in seconds.
    pub duration: u32,
    /// The duration of the pause between the warmup and load phases.
    pub pause: u32,
}

impl FromBytes for WarmupConfig {
    /// Parses a warmup config from YAML bytes.
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let config: Self = yaml_serde::from_reader(bytes)?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;
    use crate::config::error::Error;

    #[test]
    fn warmup_config_valid_yaml_parses_correctly() {
        let config = WarmupConfig::from_bytes(b"rate: 10\nduration: 30\npause: 5\n").unwrap();
        assert_eq!(config.rate, 10);
        assert_eq!(config.duration, 30);
        assert_eq!(config.pause, 5);
    }

    #[test]
    fn warmup_config_invalid_yaml_returns_error() {
        let result = WarmupConfig::from_bytes(b"rate: not-a-number\nduration: 30\npause: 5\n");
        assert_matches!(result, Err(Error::Yaml(_)));
    }

    #[test]
    fn warmup_config_missing_field_returns_error() {
        let result = WarmupConfig::from_bytes(b"rate: 10\npause: 5\n");
        assert_matches!(result, Err(Error::Yaml(_)));
    }

    #[test]
    fn warmup_config_empty_yaml_returns_error() {
        let result = WarmupConfig::from_bytes(b"");
        assert_matches!(result, Err(Error::Yaml(_)));
    }
}
