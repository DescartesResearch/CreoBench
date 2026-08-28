use std::collections::BTreeMap;

use super::{Error, FromBytes, Result};

/// A mapping of service names to their addresses, loaded from config.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
#[serde(transparent)]
pub struct ServiceRegistryConfig {
    /// Map of service name → address (e.g. `"auth" → "https://auth.example.com"`).
    pub services: BTreeMap<String, String>,
}

impl FromBytes for ServiceRegistryConfig {
    /// Parses a service registry from YAML bytes.
    ///
    /// Expects a mapping of service names to base URLs; must not be empty.
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let services: Self = yaml_serde::from_reader(bytes)?;
        services.validate()?;
        Ok(services)
    }
}

impl ServiceRegistryConfig {
    fn validate(&self) -> Result<()> {
        if self.services.is_empty() {
            return Err(Error::EmptyServiceRegistry);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    #[test]
    fn service_registry_valid_yaml_parses_correctly() {
        let registry = ServiceRegistryConfig::from_bytes(
            b"auth: https://auth.example.com\napi: https://api.example.com\n",
        )
        .unwrap();
        assert_eq!(registry.services.len(), 2);
        assert_eq!(
            registry.services.get("auth").unwrap(),
            "https://auth.example.com"
        );
        assert_eq!(
            registry.services.get("api").unwrap(),
            "https://api.example.com"
        );
    }

    #[test]
    fn service_registry_empty_yaml_returns_error() {
        let result = ServiceRegistryConfig::from_bytes(b"");
        assert_matches!(result, Err(Error::EmptyServiceRegistry));
    }
}
