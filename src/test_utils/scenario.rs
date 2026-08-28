use std::sync::Arc;

use rand::rngs::StdRng;

use crate::test_utils::mock_client::MockClientBuilder;
use crate::test_utils::mock_client::MockHttpClient;
use crate::test_utils::pool::PoolBuilder;
use crate::test_utils::registry::RegistryBuilder;
use crate::test_utils::script::ScriptBuilder;
use crate::virtual_user::Pool;
use crate::wire::ServiceRegistry;

#[derive(Default)]
pub struct ScenarioBuilder {
    script: ScriptBuilder,
    registry: RegistryBuilder,
    client: MockClientBuilder,
    pool: PoolBuilder,
}

impl ScenarioBuilder {
    pub fn modify_script(mut self, f: impl FnOnce(ScriptBuilder) -> ScriptBuilder) -> Self {
        self.script = f(self.script);
        self
    }

    pub fn modify_registry(mut self, f: impl FnOnce(RegistryBuilder) -> RegistryBuilder) -> Self {
        self.registry = f(self.registry);
        self
    }

    pub fn modify_client(mut self, f: impl FnOnce(MockClientBuilder) -> MockClientBuilder) -> Self {
        self.client = f(self.client);
        self
    }

    pub fn modify_pool(mut self, f: impl FnOnce(PoolBuilder) -> PoolBuilder) -> Self {
        self.pool = f(self.pool);
        self
    }

    pub async fn build(self) -> Scenario {
        let script = self.script.build();
        let registry = self.registry.build();
        let client = self.client.build();

        let pool = self
            .pool
            .build(script.clone(), registry.clone(), client.clone())
            .await
            .unwrap();

        Scenario {
            script,
            registry,
            client,
            pool,
        }
    }
}

pub struct Scenario {
    pub script: Arc<str>,
    pub registry: ServiceRegistry,
    pub client: MockHttpClient,
    pub pool: Pool<MockHttpClient, StdRng>,
}
