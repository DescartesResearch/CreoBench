use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::virtual_user::{Error, Pool};
use crate::wire::ServiceRegistry;

use super::mock_client::MockHttpClient;

#[derive(Debug, Clone, Copy)]
pub struct PoolBuilder {
    size: u32,
    seed: u64,
}

impl Default for PoolBuilder {
    fn default() -> Self {
        Self {
            size: 1,
            seed: 4321,
        }
    }
}

impl PoolBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub async fn build(
        self,
        script: Arc<str>,
        registry: ServiceRegistry,
        client: MockHttpClient,
    ) -> Result<Pool<MockHttpClient, StdRng>, Error> {
        Pool::with_client_and_rng(
            script,
            registry,
            self.size,
            client,
            StdRng::seed_from_u64(self.seed),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::test_utils::prelude::*;
    use crate::wire::ServiceRegistry;

    fn script() -> Arc<str> {
        let entry = RequestBuilder::default().build();

        ScriptBuilder::new().with_requests(&[entry]).build()
    }

    fn registry() -> ServiceRegistry {
        RegistryBuilder::default().build()
    }

    #[tokio::test]
    async fn pool_builder_default() {
        let pool = PoolBuilder::new()
            .build(script(), registry(), MockClientBuilder::new().build())
            .await
            .unwrap();

        let user = pool.acquire().await;
        assert_eq!(user.id(), 0);
    }

    #[tokio::test]
    async fn pool_builder_with_size() {
        let pool = PoolBuilder::new()
            .with_size(2)
            .with_seed(0)
            .build(script(), registry(), MockClientBuilder::new().build())
            .await
            .unwrap();

        let u1 = pool.acquire().await;
        let u2 = pool.acquire().await;
        let mut ids = [u1.id(), u2.id()];
        ids.sort();
        assert_eq!(ids, [0, 1]);
    }
}
