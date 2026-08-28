use url::Url;

use std::time::Duration;

use crate::http::{self, HttpClient, HttpResponse};
use crate::load::{ServiceTime, ServiceTimeMeasurement};
use crate::script::{HTTPStaticRequestSpec, Response, ScriptRunner};
use crate::wire::ServiceRegistry;

use super::{CookieJar, HttpExecuteError, UrlError};

/// An executor for sending a HTTP request as defined by a [`HTTPStaticRequestSpec`].
///
/// Use [`Self::execute`] for sending the HTTP request.
/// The executor automatically stores and sends cookies using a [`CookieJar`].
#[derive(Debug)]
pub struct HttpExecutor<T: HttpClient> {
    /// The HTTP client for sending requests
    client: T,
    /// The [`CookieJar`] for storing and retrieving cookies
    jar: CookieJar,
}

impl<T: HttpClient> HttpExecutor<T> {
    /// Construct a new executor from the given [`HttpClient`].
    pub fn new(client: T) -> Self {
        Self {
            client,
            jar: CookieJar::new(),
        }
    }

    /// Return the underlying [`HttpClient`]'s timeout.
    pub fn timeout(&self) -> Duration {
        self.client.timeout()
    }

    /// Execute a HTTP request as defined by the given [`HTTPStaticRequestSpec`].
    ///
    /// This method measures the response time of the sent request using
    /// [`ServiceTimeMeasurement`].
    ///
    /// In general, sending a HTTP request involves four steps:
    ///
    /// 1. Resolve the URL defined in the [`HTTPStaticRequestSpec`]
    /// 2. Send the HTTP request including cookies matching the resolved URL and store any received
    ///    Cookies.
    /// 3. Check the status code of the response for success
    /// 4. Run [`ScriptRunner::run_http_extract`] on successful responses
    pub async fn execute(
        &mut self,
        spec: &HTTPStaticRequestSpec,
        registry: &ServiceRegistry,
        runner: &ScriptRunner,
    ) -> Result<ServiceTime, HttpExecuteError> {
        let url = resolve_url(spec, registry)?;

        let service_time_start = ServiceTimeMeasurement::now();
        let response = self.send_with_cookies(&url, spec).await;
        let service_time = service_time_start.elapsed();

        let response = match response {
            Ok(response) => response,
            Err(http::RequestError::Timeout(message)) => {
                return Err(HttpExecuteError::Timeout {
                    message,
                    service_time,
                });
            }
            Err(http::RequestError::Failed(message)) => {
                return Err(HttpExecuteError::Failed {
                    message,
                    service_time,
                });
            }
        };

        if response.status >= 400 {
            return Err(HttpExecuteError::Status {
                code: response.status,
                service_time,
            });
        }

        let script_response = Response::new(response.status, response.headers, response.body);
        runner
            .run_http_extract(spec, script_response)
            .map_err(|source| HttpExecuteError::Extract {
                source,
                service_time,
            })?;

        Ok(service_time)
    }

    /// Send the HTTP request including cookies matching the given URL (if any).
    /// Stores received cookies in the [`CookieJar`] (if any).
    async fn send_with_cookies(
        &mut self,
        url: &Url,
        spec: &HTTPStaticRequestSpec,
    ) -> Result<HttpResponse, http::RequestError> {
        let cookie = self.jar.cookie(url);
        let response = self
            .client
            .send(
                url.as_str(),
                spec.method,
                spec.headers.iter().chain(cookie.as_ref()),
                spec.body.as_ref(),
            )
            .await;
        if let Ok(ref r) = response {
            self.jar.store_cookies(&r.headers, url);
        }
        response
    }
}

fn resolve_url(spec: &HTTPStaticRequestSpec, registry: &ServiceRegistry) -> Result<Url, UrlError> {
    let base_url = registry
        .services
        .get(&spec.service)
        .ok_or_else(|| UrlError::ServiceNotFound(spec.service.clone()))?;

    let mut url = if spec.query.is_empty() {
        Url::parse(base_url).map_err(|source| UrlError::InvalidUrl {
            service: spec.service.clone(),
            source,
        })?
    } else {
        Url::parse_with_params(base_url, spec.query.iter()).map_err(|source| {
            UrlError::InvalidUrl {
                service: spec.service.clone(),
                source,
            }
        })?
    };
    url.set_path(&spec.path);

    Ok(url)
}

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::time::Duration;

    use url::Url;

    use crate::load::ServiceTime;
    use crate::script::RequestSpec;
    use crate::test_utils::prelude::*;

    use super::*;

    async fn executor_fixture(
        builder: ScenarioBuilder,
    ) -> (
        ScriptRunner,
        HTTPStaticRequestSpec,
        ServiceRegistry,
        MockHttpClient,
    ) {
        let Scenario {
            script,
            registry,
            client,
            ..
        } = builder.build().await;
        let (runner, spec) = runner_and_spec(&script);
        (runner, spec, registry, client)
    }

    fn runner_and_spec(source: &str) -> (ScriptRunner, HTTPStaticRequestSpec) {
        let mut runner = ScriptRunner::setup(source).unwrap();
        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        (runner, http_spec)
    }

    fn recorded_url(client: &MockHttpClient) -> String {
        let mut requests = client.requests();
        assert_eq!(
            requests.len(),
            1,
            "expected exactly one recorded request, but got {}",
            requests.len()
        );
        requests.pop().unwrap().url
    }

    #[tokio::test]
    async fn execute_includes_base_url_and_path() {
        let (runner, spec, registry, client) = executor_fixture(ScenarioBuilder::default()).await;
        let mut executor = HttpExecutor::new(client.clone());

        let _ = executor.execute(&spec, &registry, &runner).await.unwrap();

        assert_eq!(
            recorded_url(&client),
            format!("{DEFAULT_TEST_URL}{DEFAULT_TEST_PATH}")
        );
    }

    #[tokio::test]
    async fn execute_includes_query() {
        let request = RequestBuilder::default()
            .with_query(vec![("q", "test"), ("limit", "10")])
            .build();
        let (runner, spec, registry, client) = executor_fixture(
            ScenarioBuilder::default().modify_script(|s| s.with_requests(&[request])),
        )
        .await;
        let mut executor = HttpExecutor::new(client.clone());

        let _ = executor.execute(&spec, &registry, &runner).await.unwrap();

        let url = recorded_url(&client);
        assert!(url.contains("q=test"));
        assert!(url.contains("limit=10"));
    }

    #[tokio::test]
    async fn execute_errors_on_unknown_service() {
        let (runner, spec, registry, client) =
            executor_fixture(ScenarioBuilder::default().modify_registry(|_| {
                RegistryBuilder::new().with_service("other", "http://other.example.com")
            }))
            .await;
        let mut executor = HttpExecutor::new(client.clone());

        let err = executor
            .execute(&spec, &registry, &runner)
            .await
            .unwrap_err();

        assert_matches!(
            err,
            HttpExecuteError::Url(UrlError::ServiceNotFound(name)) if name == DEFAULT_TEST_SERVICE
        );
        assert!(client.requests().is_empty());
    }

    #[tokio::test]
    async fn execute_errors_on_invalid_base_url() {
        let (runner, spec, registry, client) = executor_fixture(
            ScenarioBuilder::default()
                .modify_registry(|r| r.with_service(DEFAULT_TEST_SERVICE, "not a valid url")),
        )
        .await;
        let mut executor = HttpExecutor::new(client.clone());

        let err = executor
            .execute(&spec, &registry, &runner)
            .await
            .unwrap_err();

        assert_matches!(
            err,
            HttpExecuteError::Url(UrlError::InvalidUrl { service, .. }) if service == DEFAULT_TEST_SERVICE
        );
        assert!(client.requests().is_empty());
    }

    #[tokio::test]
    async fn execute_errors_on_timeout() {
        let (runner, spec, registry, client) = executor_fixture(
            ScenarioBuilder::default().modify_client(|c| c.with_timeout_error("request timeout")),
        )
        .await;
        let mut executor = HttpExecutor::new(client.clone());

        let err = executor
            .execute(&spec, &registry, &runner)
            .await
            .unwrap_err();

        match err {
            HttpExecuteError::Timeout {
                message,
                service_time,
            } => {
                assert_eq!(message, "request timeout");
                assert_ne!(service_time, ServiceTime::new(Duration::ZERO));
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn execute_errors_on_failure() {
        let (runner, spec, registry, client) = executor_fixture(
            ScenarioBuilder::default().modify_client(|c| c.with_failed_error("connection refused")),
        )
        .await;
        let mut executor = HttpExecutor::new(client);

        let err = executor
            .execute(&spec, &registry, &runner)
            .await
            .unwrap_err();

        match err {
            HttpExecuteError::Failed {
                message,
                service_time,
            } => {
                assert_eq!(message, "connection refused");
                assert_ne!(service_time, ServiceTime::new(Duration::ZERO));
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn execute_succeeds_on_2xx_status_code() {
        let (runner, spec, registry, client) = executor_fixture(ScenarioBuilder::default()).await;
        let mut executor = HttpExecutor::new(client);

        let service_time = executor.execute(&spec, &registry, &runner).await.unwrap();

        assert_ne!(service_time, ServiceTime::new(Duration::ZERO));
    }

    #[tokio::test]
    async fn execute_errors_on_4xx_status_code() {
        let (runner, spec, registry, client) =
            executor_fixture(ScenarioBuilder::default().modify_client(|c| c.with_status(404)))
                .await;
        let mut executor = HttpExecutor::new(client);

        let err = executor
            .execute(&spec, &registry, &runner)
            .await
            .unwrap_err();

        match err {
            HttpExecuteError::Status { code, service_time } => {
                assert_eq!(code, 404);
                assert_ne!(service_time, ServiceTime::new(Duration::ZERO));
            }
            other => panic!("expected Status, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn execute_errors_on_5xx_status_code() {
        let (runner, spec, registry, client) =
            executor_fixture(ScenarioBuilder::default().modify_client(|c| c.with_status(503)))
                .await;
        let mut executor = HttpExecutor::new(client);

        let err = executor
            .execute(&spec, &registry, &runner)
            .await
            .unwrap_err();

        match err {
            HttpExecuteError::Status { code, service_time } => {
                assert_eq!(code, 503);
                assert_ne!(service_time, ServiceTime::new(Duration::ZERO));
            }
            other => panic!("expected Status, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn execute_errors_on_extract_failure() {
        let request = RequestBuilder::default()
            .with_extract(r#"function(store, response) error("failure") end"#)
            .build();
        let (runner, spec, registry, client) = executor_fixture(
            ScenarioBuilder::default().modify_script(|s| s.with_requests(&[request])),
        )
        .await;
        let mut executor = HttpExecutor::new(client);

        let err = executor
            .execute(&spec, &registry, &runner)
            .await
            .unwrap_err();

        match err {
            HttpExecuteError::Extract {
                source,
                service_time,
            } => {
                assert!(source.to_string().contains("failure"));
                assert_ne!(service_time, ServiceTime::new(Duration::ZERO));
            }
            other => panic!("expected Extract, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn execute_captures_cookies() {
        let (runner, spec, registry, client) =
            executor_fixture(ScenarioBuilder::default().modify_client(|c| {
                c.with_response_headers(vec![("Set-Cookie".to_string(), "sid=abc123".to_string())])
            }))
            .await;
        let mut executor = HttpExecutor::new(client);

        let _ = executor.execute(&spec, &registry, &runner).await.unwrap();

        let cookie = executor
            .jar
            .cookie(&Url::parse(registry.services.get(DEFAULT_TEST_SERVICE).unwrap()).unwrap())
            .unwrap();
        assert_eq!(cookie, ("Cookie".to_string(), "sid=abc123".to_string()));
    }

    #[tokio::test]
    async fn execute_captures_set_cookie_on_4xx_status() {
        let (runner, spec, registry, client) =
            executor_fixture(ScenarioBuilder::default().modify_client(|c| {
                c.with_status(401).with_response_headers(vec![(
                    "Set-Cookie".to_string(),
                    "sid=abc123".to_string(),
                )])
            }))
            .await;
        let mut executor = HttpExecutor::new(client);

        let err = executor
            .execute(&spec, &registry, &runner)
            .await
            .unwrap_err();
        assert_matches!(err, HttpExecuteError::Status { code: 401, .. });

        let cookie = executor
            .jar
            .cookie(&Url::parse(registry.services.get(DEFAULT_TEST_SERVICE).unwrap()).unwrap())
            .unwrap();
        assert_eq!(cookie, ("Cookie".to_string(), "sid=abc123".to_string()));
    }
}
