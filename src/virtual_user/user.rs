//! Virtual user implementation.
//!
//! A virtual user coordinates script execution and HTTP request sending.
//! Each virtual user owns its script runner and HTTP client.

use std::marker::PhantomData;
use std::sync::Arc;

use super::http::{HttpExecuteError, HttpExecutor, UrlError};
use super::{Pool, VirtualUserId};
use crate::http::{HttpClient, RequestError};
use crate::math::rng::RangeRNG;
use crate::script::{self, RequestSpec, ScriptRunner};
use crate::transaction::{
    DroppedCode, FailedCode, SpecId, TimeoutCode, Transaction, TransactionResult,
};
use crate::wire::ServiceRegistry;

#[derive(Debug)]
pub(super) enum Setup {}
#[derive(Debug)]
pub enum Loop {}

pub trait UserState {}
impl UserState for Setup {}
impl UserState for Loop {}

/// A virtual user that executes requests during load test execution.
///
/// Combines a ScriptRunner and an `HttpExecutor` into a single abstraction.
/// Coordinates between script execution and HTTP request sending.
#[derive(Debug)]
pub struct VirtualUser<S: UserState, T: HttpClient> {
    inner: Box<Inner<T>>,
    _marker: PhantomData<S>,
}

#[derive(Debug)]
struct Inner<T: HttpClient> {
    id: VirtualUserId,
    runner: ScriptRunner,
    executor: HttpExecutor<T>,
    service_registry: ServiceRegistry,
}

impl<S: UserState, T: HttpClient> VirtualUser<S, T> {
    /// Returns the virtual user ID.
    pub fn id(&self) -> VirtualUserId {
        self.inner.id
    }
}

impl<T: HttpClient> VirtualUser<Setup, T> {
    /// Creates a new virtual user with the given components.
    pub fn new(
        id: VirtualUserId,
        script_source: Arc<str>,
        http_client: T,
        service_registry: ServiceRegistry,
    ) -> Result<Self, UserError> {
        let runner = ScriptRunner::setup(script_source.as_ref()).map_err(UserError::RunnerSetup)?;
        let executor = HttpExecutor::new(http_client);
        let inner = Box::new(Inner {
            id,
            runner,
            executor,
            service_registry,
        });
        Ok(Self {
            inner,
            _marker: PhantomData,
        })
    }

    pub async fn setup(mut self) -> Result<VirtualUser<Loop, T>, UserError> {
        let Inner {
            runner,
            executor,
            service_registry,
            ..
        } = &mut *self.inner;

        while let Some((spec_id, spec)) = runner.next_setup_spec().map_err(UserError::SetupSpec)? {
            match &spec {
                RequestSpec::Http(http_spec) => {
                    executor
                        .execute(http_spec, service_registry, runner)
                        .await
                        .map_err(|source| fail_setup_with(spec_id, source))?;
                }
            }
        }

        Ok(VirtualUser::<Loop, T> {
            inner: self.inner,
            _marker: PhantomData,
        })
    }
}

fn fail_setup_with(id: SpecId, err: HttpExecuteError) -> UserError {
    match err {
        HttpExecuteError::Url(url_err) => UserError::Url(url_err),
        HttpExecuteError::Timeout { message, .. } => {
            UserError::Http(RequestError::Timeout(message))
        }
        HttpExecuteError::Failed { message, .. } => UserError::Http(RequestError::Failed(message)),
        HttpExecuteError::Status { code, .. } => UserError::Status(code),
        HttpExecuteError::Extract { source, .. } => UserError::Extract { id, source },
    }
}

impl<T: HttpClient + 'static + Clone> VirtualUser<Loop, T> {
    /// Sends the next request from the script's user cycle.
    ///
    /// This method consumes the virtual user and:
    /// 1. Gets the next request spec from the script
    /// 2. Resolves the service URL using the registry
    /// 3. Sends the HTTP request via the transport
    /// 4. Runs the extract function if present
    /// 5. Returns the virtual user to the pool
    /// 6. Returns the transaction result
    pub async fn send_next_request<R: RangeRNG + Send + Sync + 'static>(
        mut self,
        pool: Pool<T, R>,
        transaction: Transaction,
    ) -> TransactionResult {
        let result = self._send_next_request(transaction).await;
        pool.release(self);
        result
    }

    async fn _send_next_request(&mut self, transaction: Transaction) -> TransactionResult {
        let id = self.inner.id;
        let Inner {
            runner,
            executor,
            service_registry,
            ..
        } = &mut *self.inner;

        let elapsed = transaction.start_time().elapsed();
        if !executor.timeout().is_zero() && elapsed > executor.timeout() {
            return transaction.into_dropped(id, DroppedCode::WaitTimeTooLong(elapsed));
        }

        let (revert_handle, spec_id, spec) = match runner.next_user_spec() {
            Ok(t) => t,
            Err(err) => {
                return transaction.into_dropped(id, DroppedCode::Error(err.to_string()));
            }
        };

        let RequestSpec::Http(http_spec) = spec;

        match executor.execute(&http_spec, service_registry, runner).await {
            Ok(service_time) => transaction.into_success(id, spec_id, service_time),
            Err(err) => {
                revert_handle.revert(runner);
                map_execution_error(transaction, id, spec_id, err)
            }
        }
    }
}

fn map_execution_error(
    transaction: Transaction,
    id: VirtualUserId,
    spec_id: SpecId,
    err: HttpExecuteError,
) -> TransactionResult {
    use HttpExecuteError::*;
    match err {
        Timeout {
            message,
            service_time,
        } => transaction.into_timeout(id, spec_id, service_time, TimeoutCode::Error(message)),
        Failed {
            message,
            service_time,
        } => transaction.into_failed(id, spec_id, service_time, FailedCode::Send(message)),
        Status { code, service_time } => {
            transaction.into_failed(id, spec_id, service_time, FailedCode::Status(code))
        }
        Extract {
            source,
            service_time,
        } => transaction.into_failed(
            id,
            spec_id,
            service_time,
            FailedCode::Extract(source.to_string()),
        ),
        Url(url_err) => transaction.into_dropped(id, DroppedCode::Error(url_err.to_string())),
    }
}

// Errors that can occur during setting up a single [`VirtualUser`].
#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("failed to setup script runtime: {0}")]
    RunnerSetup(#[source] script::Error),
    #[error("{0}")]
    SetupSpec(#[source] script::Error),

    #[error("failed to resolve request URL: {0}")]
    Url(#[from] UrlError),

    #[error("{0}")]
    Http(#[from] RequestError),

    #[error("setup request returned non-successful status `{0}`")]
    Status(u16),

    #[error("failed to run extract function for setup request `{id}`: {source}")]
    Extract { id: SpecId, source: script::Error },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::HttpResponse;
    use crate::load::{LoadTestTime, RelativeLoadTestTime, ResponseTime, StartTime};
    use crate::script::HttpMethod;
    use crate::test_utils::prelude::*;
    use crate::transaction::{LoadGeneratorId, Transaction};
    use std::assert_matches;
    use std::time::Duration;

    fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }

    #[tokio::test]
    async fn virtual_user_sends_setup_request() {
        let setup_request = RequestBuilder::default().build();
        let script = ScriptBuilder::default()
            .with_setup(&[setup_request])
            .build();
        let client = MockClientBuilder::default().build();
        let registry = RegistryBuilder::default().build();

        let user =
            VirtualUser::new(VirtualUserId::new(1), script, client.clone(), registry).unwrap();

        let _user = user.setup().await.unwrap();

        let requests = client.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url,
            format!("{DEFAULT_TEST_URL}{DEFAULT_TEST_PATH}")
        );
        assert_eq!(requests[0].method, HttpMethod::Get);
    }

    #[tokio::test]
    async fn virtual_user_handles_query_parameters_in_setup_requests() {
        let setup_request = RequestBuilder::default()
            .with_query(vec![("q", "test"), ("limit", "10")])
            .build();
        let script = ScriptBuilder::default()
            .with_setup(&[setup_request])
            .build();
        let client = MockClientBuilder::default().build();
        let registry = RegistryBuilder::default().build();

        let user =
            VirtualUser::new(VirtualUserId::new(1), script, client.clone(), registry).unwrap();

        let _user = user.setup().await.unwrap();

        let requests = client.requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].url.contains("?"));
        assert!(requests[0].url.contains("q=test"));
        assert!(requests[0].url.contains("&"));
        assert!(requests[0].url.contains("limit=10"));
    }

    #[tokio::test]
    async fn virtual_user_errors_on_invalid_service_in_setup_requests() {
        let setup_request = RequestBuilder::default().with_service("missing").build();
        let script = ScriptBuilder::default()
            .with_setup(&[setup_request])
            .build();
        let client = MockClientBuilder::default().build();
        let registry = RegistryBuilder::default().build();

        let user = VirtualUser::new(VirtualUserId::new(1), script, client, registry).unwrap();

        let err = user.setup().await.unwrap_err();

        assert_matches!(err, UserError::Url(inner) if matches!(inner, UrlError::ServiceNotFound(_)));
    }

    #[tokio::test]
    async fn virtual_user_errors_on_invalid_base_url_in_service_registry() {
        let setup_request = RequestBuilder::default().build();
        let script = ScriptBuilder::default()
            .with_setup(&[setup_request])
            .build();
        let client = MockClientBuilder::default().build();
        let registry = RegistryBuilder::new()
            .with_service(DEFAULT_TEST_SERVICE, "not a valid url")
            .build();

        let user = VirtualUser::new(VirtualUserId::new(1), script, client, registry).unwrap();

        let err = user.setup().await.unwrap_err();

        assert_matches!(err, UserError::Url(inner) if matches!(&inner, UrlError::InvalidUrl{service ,..} if service == DEFAULT_TEST_SERVICE));
    }

    #[tokio::test]
    async fn virtual_user_construction_fails_with_invalid_lua_script() {
        let err = VirtualUser::new(
            VirtualUserId::new(1),
            "not valid lua {{{".into(),
            MockClientBuilder::new().build(),
            RegistryBuilder::default().build(),
        )
        .unwrap_err();

        assert_matches!(err, UserError::RunnerSetup(_));
    }

    #[tokio::test]
    async fn virtual_user_setup_with_multiple_setup_requests() {
        let setup_1 = RequestBuilder::default().with_path("/first").build();
        let setup_2 = RequestBuilder::default()
            .with_method("POST")
            .with_path("/second")
            .build();
        let script = ScriptBuilder::default()
            .with_setup(&[setup_1, setup_2])
            .build();
        let client = MockClientBuilder::default().build();
        let registry = RegistryBuilder::default().build();

        let user =
            VirtualUser::new(VirtualUserId::new(1), script, client.clone(), registry).unwrap();

        let _user = user.setup().await.unwrap();

        let requests = client.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].url, format!("{DEFAULT_TEST_URL}/first"));
        assert_eq!(requests[1].url, format!("{DEFAULT_TEST_URL}/second"));
        assert_eq!(requests[1].method, HttpMethod::Post);
    }

    #[tokio::test]
    async fn virtual_user_setup_fails_on_4xx_status_code() {
        let setup_request = RequestBuilder::default().build();
        let script = ScriptBuilder::default()
            .with_setup(&[setup_request])
            .build();
        let client = MockClientBuilder::default().with_status(418).build();
        let registry = RegistryBuilder::default().build();

        let user = VirtualUser::new(VirtualUserId::new(1), script, client, registry).unwrap();

        let err = user.setup().await.unwrap_err();

        assert_matches!(err, UserError::Status(418));
    }

    #[tokio::test]
    async fn virtual_user_setup_fails_on_5xx_status_code() {
        let setup_request = RequestBuilder::default().build();
        let script = ScriptBuilder::default()
            .with_setup(&[setup_request])
            .build();
        let client = MockClientBuilder::default().with_status(503).build();
        let registry = RegistryBuilder::default().build();

        let user = VirtualUser::new(VirtualUserId::new(1), script, client, registry).unwrap();

        let err = user.setup().await.unwrap_err();

        assert_matches!(err, UserError::Status(503));
    }

    #[tokio::test]
    async fn send_next_request_succeeds_on_2xx_status_code() {
        let Scenario { pool, .. } = ScenarioBuilder::default().build().await;
        let user = pool.acquire().await;
        let transaction = sample_transaction();

        let result = user.send_next_request(pool, transaction).await;

        match result {
            TransactionResult::Success { metadata, .. } => {
                assert_eq!(metadata.spec_id, SpecId::new(0));
                assert_eq!(metadata.virtual_user_id, VirtualUserId::new(0));
                assert_eq!(metadata.loadgenerator_id, LoadGeneratorId::new(7));
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn send_next_request_drops_with_wait_time_too_long() {
        let Scenario { pool, .. } = ScenarioBuilder::default()
            .modify_client(|c| c.with_timeout_duration(Duration::from_millis(1)))
            .build()
            .await;
        let user = pool.acquire().await;
        let transaction = sample_transaction();

        tokio::time::sleep(Duration::from_millis(5)).await;

        let result = user.send_next_request(pool, transaction).await;

        match result {
            TransactionResult::Dropped {
                metadata,
                code,
                response_time,
            } => {
                assert_matches!(code, DroppedCode::WaitTimeTooLong(_));
                assert_eq!(metadata.virtual_user_id, VirtualUserId::new(0));
                assert_eq!(metadata.loadgenerator_id, LoadGeneratorId::new(7));
                assert!(response_time > ResponseTime::new(Duration::from_millis(5)));
            }
            other => panic!("expected Dropped, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn send_next_request_drops_on_script_errors() {
        let request =
            RequestBuilder::dynamic(r#"function(store) error("runner failure") end"#).build();
        let Scenario { pool, .. } = ScenarioBuilder::default()
            .modify_script(|s| s.with_requests(&[request]))
            .build()
            .await;
        let user = pool.acquire().await;
        let transaction = sample_transaction();

        let result = user.send_next_request(pool, transaction).await;

        match result {
            TransactionResult::Dropped { metadata, code, .. } => {
                match code {
                    DroppedCode::Error(msg) => {
                        assert!(msg.contains("dynamic request spec"));
                    }
                    other => panic!("expected DroppedCode::Error, got {other:?}"),
                }
                assert_eq!(metadata.virtual_user_id, VirtualUserId::new(0));
                assert_eq!(metadata.loadgenerator_id, LoadGeneratorId::new(7));
            }
            other => panic!("expected Dropped, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn send_next_request_reverts_cursor_on_failed_status() {
        let failure_response = HttpResponse {
            status: 500,
            headers: vec![],
            body: None,
        };
        let success_response = HttpResponse {
            status: 200,
            headers: vec![],
            body: None,
        };
        let Scenario { pool, .. } = ScenarioBuilder::default()
            .modify_client(|c| {
                c.with_response_sequence([
                    MockResponse::ok(failure_response),
                    MockResponse::ok(success_response),
                ])
            })
            .build()
            .await;

        let user = pool.acquire().await;
        let attempt1 = user
            .send_next_request(pool.clone(), sample_transaction())
            .await;
        let first_id = match attempt1 {
            TransactionResult::Failed {
                ref metadata,
                ref code,
                ..
            } => {
                assert_matches!(code, FailedCode::Status(500));
                metadata.spec_id
            }
            other => panic!("expected Failed, got {other:?}"),
        };

        let user = pool.acquire().await;
        let attempt2 = user.send_next_request(pool, sample_transaction()).await;
        let second_id = match attempt2 {
            TransactionResult::Success { ref metadata, .. } => metadata.spec_id,
            other => panic!("expected Success, got {other:?}"),
        };

        assert_eq!(second_id, first_id);
    }

    #[tokio::test]
    async fn send_next_request_reverts_cursor_on_failed_send() {
        let success_response = HttpResponse {
            status: 200,
            headers: vec![],
            body: None,
        };
        let Scenario { pool, .. } = ScenarioBuilder::default()
            .modify_client(|c| {
                c.with_response_sequence([
                    MockResponse::failed("connection refused"),
                    MockResponse::ok(success_response),
                ])
            })
            .build()
            .await;

        let user = pool.acquire().await;
        let attempt1 = user
            .send_next_request(pool.clone(), sample_transaction())
            .await;
        let first_id = match attempt1 {
            TransactionResult::Failed {
                ref metadata,
                ref code,
                ..
            } => {
                assert_matches!(code, FailedCode::Send(_));
                metadata.spec_id
            }
            other => panic!("expected Failed, got {other:?}"),
        };

        let user = pool.acquire().await;
        let attempt2 = user.send_next_request(pool, sample_transaction()).await;
        let second_id = match attempt2 {
            TransactionResult::Success { ref metadata, .. } => metadata.spec_id,
            other => panic!("expected Success, got {other:?}"),
        };

        assert_eq!(second_id, first_id);
    }

    #[tokio::test]
    async fn send_next_request_reverts_cursor_on_failed_extract() {
        let request1 = RequestBuilder::default()
            .with_extract(
                r#"function(store, response) if store:get("calledOnce") then return end store:set("calledOnce", true) error("extract failure") end"#,
            )
            .build();
        let request2 = RequestBuilder::default().build();
        let Scenario { pool, .. } = ScenarioBuilder::default()
            .modify_script(|s| s.with_requests(&[request1, request2]))
            .build()
            .await;

        let user = pool.acquire().await;
        let attempt1 = user
            .send_next_request(pool.clone(), sample_transaction())
            .await;
        let first_id = match attempt1 {
            TransactionResult::Failed {
                ref metadata,
                ref code,
                ..
            } => {
                assert_matches!(code, FailedCode::Extract(_));
                metadata.spec_id
            }
            other => panic!("expected Failed, got {other:?}"),
        };

        let user = pool.acquire().await;
        let attempt2 = user.send_next_request(pool, sample_transaction()).await;
        let second_id = match attempt2 {
            TransactionResult::Success { ref metadata, .. } => metadata.spec_id,
            other => panic!("expected Success on retry, got {other:?}"),
        };

        assert_eq!(second_id, first_id);
    }

    #[tokio::test]
    async fn send_next_request_reverts_cursor_on_timeout() {
        let success_response = HttpResponse {
            status: 200,
            headers: vec![],
            body: None,
        };
        let Scenario { pool, .. } = ScenarioBuilder::default()
            .modify_client(|c| {
                c.with_response_sequence([
                    MockResponse::timeout("request timeout"),
                    MockResponse::ok(success_response),
                ])
            })
            .build()
            .await;

        let user = pool.acquire().await;
        let attempt1 = user
            .send_next_request(pool.clone(), sample_transaction())
            .await;
        let first_id = match attempt1 {
            TransactionResult::Timeout {
                ref metadata,
                ref code,
                ..
            } => {
                assert_eq!(code, &TimeoutCode::Error("request timeout".to_string()));
                metadata.spec_id
            }
            other => panic!("expected Timeout, got {other:?}"),
        };

        let user = pool.acquire().await;
        let attempt2 = user.send_next_request(pool, sample_transaction()).await;
        let second_id = match attempt2 {
            TransactionResult::Success { ref metadata, .. } => metadata.spec_id,
            other => panic!("expected Success, got {other:?}"),
        };

        assert_eq!(second_id, first_id);
    }

    fn sample_transaction() -> Transaction {
        Transaction::new(
            LoadGeneratorId::new(7),
            StartTime::now(LoadTestTime::now()),
            RelativeLoadTestTime::new(Duration::from_millis(500)),
        )
    }

    #[tokio::test]
    async fn send_next_request_reverts_cursor_on_missing_service() {
        let request1 = RequestBuilder::default().with_service("missing").build();
        let request2 = RequestBuilder::default().with_path("/second").build();
        let Scenario { pool, client, .. } = ScenarioBuilder::default()
            .modify_script(|s| s.with_requests(&[request1, request2]))
            .build()
            .await;

        for _ in 0..2 {
            let user = pool.acquire().await;
            let attempt = user
                .send_next_request(pool.clone(), sample_transaction())
                .await;
            match attempt {
                TransactionResult::Dropped { ref code, .. } => match code {
                    DroppedCode::Error(msg) => {
                        assert!(msg.contains("is not in the service registry"))
                    }
                    other => panic!("expected DroppedCode::Error, got {other:?}"),
                },
                other => panic!("expected Dropped, got {other:?}"),
            }
        }

        assert_eq!(client.requests().len(), 0);
    }

    #[tokio::test]
    async fn setup_set_cookie_is_sent_on_subsequent_user_loop_request() {
        let setup_request = RequestBuilder::default().with_path("/login").build();
        let Scenario { pool, client, .. } = ScenarioBuilder::default()
            .modify_script(|s| s.with_setup(&[setup_request]))
            .modify_client(|c| {
                c.with_response_headers(vec![("Set-Cookie".to_string(), "sid=abc123".to_string())])
            })
            .build()
            .await;

        let user = pool.acquire().await;
        let transaction = sample_transaction();
        let result = user.send_next_request(pool, transaction).await;

        assert_matches!(result, TransactionResult::Success { .. });

        let recorded = client.requests();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].url, format!("{DEFAULT_TEST_URL}/login"));
        assert_eq!(
            recorded[1].url,
            format!("{DEFAULT_TEST_URL}{DEFAULT_TEST_PATH}")
        );

        assert_eq!(header_value(&recorded[0].headers, "Cookie"), None);

        assert_eq!(
            header_value(&recorded[1].headers, "Cookie"),
            Some("sid=abc123".into())
        );
    }

    #[tokio::test]
    async fn concurrent_vus_have_isolated_cookie_jars() {
        let setup_for_a = HttpResponse {
            status: 200,
            headers: vec![("Set-Cookie".to_string(), "sid=alpha".to_string())],
            body: None,
        };
        let setup_for_b = HttpResponse {
            status: 200,
            headers: vec![("Set-Cookie".to_string(), "sid=bravo".to_string())],
            body: None,
        };
        let user_loop_response = HttpResponse {
            status: 200,
            headers: vec![],
            body: None,
        };
        let setup_request = RequestBuilder::default().with_path("/login").build();
        let Scenario { pool, client, .. } = ScenarioBuilder::default()
            .modify_script(|s| s.with_setup(&[setup_request]))
            .modify_pool(|p| p.with_size(2))
            .modify_client(|c| {
                c.with_responses([
                    setup_for_a,
                    setup_for_b,
                    user_loop_response.clone(),
                    user_loop_response,
                ])
            })
            .build()
            .await;

        let user_a = pool.acquire().await;
        let user_b = pool.acquire().await;

        let result_a = user_a
            .send_next_request(pool.clone(), sample_transaction())
            .await;
        let result_b = user_b.send_next_request(pool, sample_transaction()).await;

        assert_matches!(result_a, TransactionResult::Success { .. });
        assert_matches!(result_b, TransactionResult::Success { .. });

        let recorded = client.requests();
        assert_eq!(recorded.len(), 4);

        let user_loop_cookies: Vec<String> = recorded[2..]
            .iter()
            .filter_map(|r| header_value(&r.headers, "Cookie"))
            .collect();
        assert_eq!(user_loop_cookies.len(), 2);

        let cookie_a = &user_loop_cookies[0];
        let cookie_b = &user_loop_cookies[1];
        assert!(cookie_a.contains("sid=alpha") || cookie_a.contains("sid=bravo"));
        assert!(cookie_b.contains("sid=alpha") || cookie_b.contains("sid=bravo"));
        assert_ne!(cookie_a, cookie_b);
    }
}
