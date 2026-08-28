use std::collections::BTreeMap;
use std::sync::Arc;

use crate::config::ServiceRegistryConfig;

#[derive(Debug, Default)]
pub struct ServiceRegistry {
    pub services: Arc<BTreeMap<String, String>>,
}

impl Clone for ServiceRegistry {
    fn clone(&self) -> Self {
        Self {
            services: Arc::clone(&self.services),
        }
    }
}

impl serde::Serialize for ServiceRegistry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.services.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ServiceRegistry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let services = BTreeMap::<String, String>::deserialize(deserializer)?;
        Ok(Self {
            services: Arc::new(services),
        })
    }
}

impl PartialEq for ServiceRegistry {
    fn eq(&self, other: &Self) -> bool {
        self.services == other.services
    }
}

impl From<ServiceRegistryConfig> for ServiceRegistry {
    fn from(cfg: ServiceRegistryConfig) -> Self {
        Self {
            services: Arc::new(cfg.services),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::ServiceRegistryConfig;

    use super::*;

    #[test]
    fn from_cfg_service_registry_preserves_all_fields() {
        let mut services = BTreeMap::new();
        services.insert("auth".to_string(), "https://auth.example.com".to_string());
        services.insert("api".to_string(), "https://api.example.com".to_string());

        let cfg = ServiceRegistryConfig {
            services: services.clone(),
        };
        let wire: ServiceRegistry = cfg.into();
        assert_eq!(*wire.services, services);
    }
}
