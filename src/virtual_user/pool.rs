//! Virtual user pool management.
//!
//! Provides pool semantics for acquiring and returning virtual users during load test execution.
//! Pool operations are blocking on acquire and non-blocking on release.

use super::VirtualUser;
use super::VirtualUserId;
use super::user;
use super::user::Loop;
use super::user::UserError;
use crate::http;
use crate::http::{HttpClient, ReqwestHttpClient};
use crate::math::rng::RangeRNG;
use crate::wire::ServiceRegistry;
use parking_lot::Mutex;
use rand::SeedableRng;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Errors that can occur during virtual user pool setup.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to initialize HTTP client: {0}")]
    HttpClient(#[from] http::Error),
    #[error("failed to setup virtual user `{id}`: {source}")]
    VirtualUserSetup {
        id: VirtualUserId,
        source: UserError,
    },
}

/// Shared internal pool state.
#[derive(Debug)]
struct Inner<R: RangeRNG + Send + Sync + 'static, T: HttpClient> {
    /// Available virtual users and RNG instance.
    data: Mutex<(Vec<VirtualUser<user::Loop, T>>, R)>,

    /// Semaphore tracks available users.
    available: Semaphore,
}

/// A pool of virtual users available for load test execution.
///
/// The pool manages virtual user lifecycle including acquisition and release.
/// Pool provides async blocking acquire operations and non-blocking release operations.
/// The pool is thread-safe and can be cloned for concurrent access.
#[derive(Debug)]
pub struct Pool<
    T: HttpClient = ReqwestHttpClient,
    R: RangeRNG + Send + Sync + 'static = rand::rngs::StdRng,
> {
    inner: Arc<Inner<R, T>>,
}

impl<T: HttpClient, R: RangeRNG + Send + Sync + 'static> Clone for Pool<T, R> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Pool<ReqwestHttpClient, rand::rngs::StdRng> {
    /// Creates a new pool by setting up virtual users concurrently.
    ///
    /// # Arguments
    ///
    /// * `script_source` - The Lua script source code
    /// * `service_registry` - The service registry for resolving service URLs
    /// * `virtual_user_count` - The number of virtual users to create
    /// * `timeout` - The timeout in ms
    /// * `seed` - The random number generator seed
    ///
    /// # Returns
    ///
    /// A new `Pool` instance or a `SetupError` if setup fails.
    ///
    /// # Errors
    ///
    /// Returns `SetupError` if:
    /// - Script loading fails
    /// - Any virtual user setup fails
    /// - Network errors occur during setup
    /// - Extract function errors occur
    pub async fn new(
        script_source: Arc<str>,
        service_registry: ServiceRegistry,
        virtual_user_count: u32,
        timeout: u64,
        seed: u64,
    ) -> Result<Self, Error> {
        Self::with_client_and_rng(
            script_source,
            service_registry,
            virtual_user_count,
            ReqwestHttpClient::with_timeout(timeout)?,
            rand::rngs::StdRng::seed_from_u64(seed),
        )
        .await
    }
}

impl<T: HttpClient + Clone + 'static, R: RangeRNG + Send + Sync + 'static> Pool<T, R> {
    pub async fn with_client_and_rng(
        script_source: Arc<str>,
        service_registry: ServiceRegistry,
        virtual_user_count: u32,
        http_client: T,
        rng: R,
    ) -> Result<Self, Error> {
        let setup_tasks = (0..virtual_user_count).map(VirtualUserId::new).map(|id| {
            let script_source = Arc::clone(&script_source);
            let service_registry = service_registry.clone();
            let http_client = http_client.clone();

            tokio::spawn(async move {
                let user = VirtualUser::new(id, script_source, http_client, service_registry)
                    .map_err(|source| Error::VirtualUserSetup { id, source })?;

                user.setup()
                    .await
                    .map_err(|source| Error::VirtualUserSetup { id, source })
            })
        });

        let mut users = Vec::with_capacity(virtual_user_count as usize);

        for task in setup_tasks {
            let user = task.await.expect("task panicked")?;

            users.push(user);
        }

        let permits = users.len();
        Ok(Self {
            inner: Arc::new(Inner {
                data: Mutex::new((users, rng)),
                available: Semaphore::new(permits),
            }),
        })
    }

    /// Acquires a virtual user from the pool.
    ///
    /// Blocks asynchronously until a virtual user is available.
    pub async fn acquire(&self) -> VirtualUser<Loop, T> {
        // Semaphore represents ownership of an available user.
        let permit = self.inner.available.acquire().await.unwrap();

        let user = {
            let (ref mut users, ref mut rng) = *self.inner.data.lock();

            let index = rng.random_range(0..users.len());
            users.swap_remove(index)
        };

        // Do not put permit back yet, since the user is not released back to the pool yet.
        permit.forget();
        user
    }

    /// Releases a virtual user back into the pool.
    pub fn release(&self, user: VirtualUser<Loop, T>) {
        {
            let (ref mut users, _) = *self.inner.data.lock();

            users.push(user);
        }

        self.inner.available.add_permits(1);
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;
    use crate::test_utils::prelude::*;

    async fn pool_from(size: u32) -> Pool<MockHttpClient> {
        let Scenario { pool, .. } = ScenarioBuilder::default()
            .modify_pool(|p| p.with_size(size))
            .build()
            .await;
        pool
    }

    async fn try_pool_from(script: Arc<str>, count: u32) -> Result<Pool<MockHttpClient>, Error> {
        PoolBuilder::new()
            .with_size(count)
            .build(
                script,
                RegistryBuilder::default().build(),
                MockClientBuilder::new().build(),
            )
            .await
    }

    #[tokio::test]
    async fn pool_acquire_returns_user_with_expected_id() {
        let pool = pool_from(1).await;

        let user = pool.acquire().await;

        assert_eq!(user.id(), 0);
    }

    #[tokio::test]
    async fn pool_acquire_release_cycle() {
        let pool = pool_from(1).await;

        let user = pool.acquire().await;
        assert_eq!(user.id(), 0);
        pool.release(user);

        let user = pool.acquire().await;
        assert_eq!(user.id(), 0);
    }

    #[tokio::test]
    async fn pool_acquire_release_multiple_users() {
        let pool = pool_from(3).await;

        let u1 = pool.acquire().await;
        let u2 = pool.acquire().await;
        let u3 = pool.acquire().await;

        let mut ids = [u1.id(), u2.id(), u3.id()];
        ids.sort();
        assert_eq!(ids, [0, 1, 2]);

        let id = u1.id();
        pool.release(u1);
        let u4 = pool.acquire().await;
        assert_eq!(u4.id(), id);
    }

    #[tokio::test]
    async fn pool_clone_shares_state() {
        let pool = pool_from(1).await;
        let pool2 = pool.clone();

        let user = pool2.acquire().await;
        let id = user.id();

        tokio::time::timeout(std::time::Duration::from_millis(10), pool.acquire())
            .await
            .unwrap_err();
        tokio::time::timeout(std::time::Duration::from_millis(10), pool2.acquire())
            .await
            .unwrap_err();

        pool.release(user);

        let user = pool2.acquire().await;

        assert_eq!(user.id(), id);
    }

    #[tokio::test]
    async fn pool_acquire_concurrently_returns_unique_users() {
        let pool = pool_from(10).await;

        let mut handles = Vec::new();
        for _ in 0..10 {
            let pool = pool.clone();
            handles.push(tokio::spawn(async move { pool.acquire().await }));
        }

        let mut ids: Vec<_> = Vec::with_capacity(handles.len());
        for handle in handles {
            ids.push(handle.await.unwrap().id());
        }
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 10);
    }

    #[tokio::test]
    async fn pool_setup_returns_error_for_invalid_script() {
        let err = try_pool_from("invalid lua {{{".into(), 1)
            .await
            .unwrap_err();

        match err {
            Error::VirtualUserSetup { id, source } => {
                assert_eq!(id, 0);
                assert!(matches!(source, UserError::RunnerSetup(_)));
            }
            err => panic!("unexpected err: {:?}", err),
        }
    }

    #[tokio::test]
    async fn pool_setup_returns_error_for_extract_failure() {
        let setup_entry = RequestBuilder::default()
            .with_extract(r#"function(store, response) error("extract failed intentionally") end"#)
            .build();
        let request_entry = RequestBuilder::default().build();
        let script = ScriptBuilder::new()
            .with_setup(&[setup_entry])
            .with_requests(&[request_entry])
            .build();
        let err = try_pool_from(script, 1).await.unwrap_err();

        match err {
            Error::VirtualUserSetup { id, source } => {
                assert_eq!(id, 0);
                assert_matches!(source, UserError::Extract { .. });
            }
            err => panic!("unexpected err: {:?}", err),
        }
    }

    #[tokio::test]
    async fn pool_setup_returns_error_for_missing_service() {
        let setup_entry = RequestBuilder::default()
            .with_service("nonexistent")
            .build();
        let request_entry = RequestBuilder::default().build();
        let script = ScriptBuilder::new()
            .with_setup(&[setup_entry])
            .with_requests(&[request_entry])
            .build();
        let err = try_pool_from(script, 1).await.unwrap_err();

        match err {
            Error::VirtualUserSetup { id, source } => {
                assert_eq!(id, 0);
                assert!(matches!(source, UserError::Url(_)));
            }
            err => panic!("unexpected err: {:?}", err),
        }
    }
}
