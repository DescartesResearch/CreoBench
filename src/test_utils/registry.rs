use std::collections::BTreeMap;
use std::sync::Arc;

use crate::wire::ServiceRegistry;

use super::script::DEFAULT_TEST_SERVICE;

#[derive(Debug)]
pub struct RegistryBuilder {
    services: BTreeMap<String, String>,
}

impl RegistryBuilder {
    pub fn new() -> Self {
        Self {
            services: Default::default(),
        }
    }

    pub fn with_service(mut self, name: impl Into<String>, address: impl Into<String>) -> Self {
        self.services.insert(name.into(), address.into());
        self
    }

    pub fn build(self) -> ServiceRegistry {
        ServiceRegistry {
            services: Arc::new(self.services),
        }
    }

    /// Serializes this registry into the registry YAML format the loader
    /// parses.
    #[cfg(feature = "test-utils")]
    pub fn to_yaml(&self) -> String {
        yaml_serde::to_string(&self.services).unwrap()
    }
}

pub const DEFAULT_TEST_URL: &str = "http://api.example.com";
impl Default for RegistryBuilder {
    fn default() -> Self {
        Self::new().with_service(DEFAULT_TEST_SERVICE, DEFAULT_TEST_URL)
    }
}
