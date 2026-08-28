//! Utilities for running an HTTP server in tests and asserting on the
//! requests it receives.
use std::time::Duration;

use mockito::{Matcher, Mock};

/// An HTTP server for use in tests, with configurable routes and
/// request/response expectations.
///
/// Routes are registered with [`route`](Self::route). Any expectations set
/// on them (via [`Route::expect`]) are checked when `HttpTestServer` is
/// dropped, so a missed or unexpected request will fail the test even
/// without an explicit assertion.
#[derive(Debug)]
pub struct HttpTestServer {
    mocks: Vec<Mock>,
    server: mockito::Server,
}

impl HttpTestServer {
    /// Starts a new server, ready to accept routes.
    pub async fn new() -> Self {
        let server = mockito::Server::new_with_opts_async(mockito::ServerOpts {
            assert_on_drop: true,
            ..Default::default()
        })
        .await;
        Self {
            server,
            mocks: Vec::default(),
        }
    }

    /// Returns the base URL of the server.
    pub fn url(&self) -> String {
        self.server.url()
    }

    /// Starts building a route that matches requests to `path` using
    /// `method`.
    ///
    /// The route has no effect until [`Route::create`] is called.
    pub fn route(&'_ mut self, method: &str, path: &str) -> Route<'_> {
        let mock = self.server.mock(method, path);
        Route {
            mock,
            server: self,
            body: None,
            delay: None,
        }
    }
}

/// A HTTP server route.
///
/// Created with [`HttpTestServer::route`] and registered with
/// [`Route::create`].
pub struct Route<'a> {
    mock: Mock,
    server: &'a mut HttpTestServer,
    body: Option<Vec<u8>>,
    delay: Option<Duration>,
}

impl Route<'_> {
    /// Requires the request to carry the header `field: value`.
    pub fn matching_header(mut self, field: &str, value: &str) -> Self {
        self.mock = self.mock.match_header(field, value);
        self
    }

    /// Requires the request to carry the given query.
    pub fn matching_query(mut self, query: &[(&'static str, &'static str)]) -> Self {
        if query.is_empty() {
            return self;
        }
        let query = query
            .iter()
            .map(|(key, value)| Matcher::UrlEncoded(key.to_string(), value.to_string()))
            .collect();

        self.mock = self.mock.match_query(Matcher::AllOf(query));
        self
    }

    /// Sets the response status code.
    pub fn with_status(mut self, status: usize) -> Self {
        self.mock = self.mock.with_status(status);
        self
    }

    /// Sets the response body.
    ///
    /// Can be combined with [`with_delay`](Self::with_delay) to
    /// inject a response delay.
    pub fn with_body(mut self, body: &serde_json::Value) -> Self {
        self.body = Some(serde_json::to_string(body).unwrap().into_bytes());
        self
    }

    /// Sets a response header.
    pub fn with_header(mut self, field: &str, value: &str) -> Self {
        self.mock = self.mock.with_header(field, value);
        self
    }

    /// Delays delivery of the response body by `delay`.
    ///
    /// The response head is sent immediately; the body — whatever was set
    /// via [`with_body`](Self::with_body) — is sent only after the delay
    /// elapses, so the response as a whole completes no sooner than `delay`.
    ///
    /// Note: the delay currently blocks the thread handling the request.
    #[allow(dead_code)]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// Registers the route on the server.
    ///
    /// After this call, the server will match and respond to requests
    /// according to the matchers and response set on this route. Any
    /// expectations set via [`expect`](Self::expect) are checked when the
    /// owning [`HttpTestServer`] is dropped, which also covers the case of
    /// never receiving a matching request.
    pub fn create(mut self) {
        let body = self.body;
        if let Some(ref body) = body {
            self.mock = self.mock.match_body(std::str::from_utf8(body).unwrap());
        }

        match self.delay {
            Some(delay) => {
                let body = body.unwrap_or_default();
                self.mock = self.mock.with_chunked_body(move |writer| {
                    std::thread::sleep(delay);
                    std::io::Write::write_all(writer, &body)
                })
            }
            None => {
                if let Some(body) = body {
                    self.mock = self.mock.with_body(body);
                };
            }
        };
        self.mock = self.mock.expect_at_least(1);
        self.server.mocks.push(self.mock.create());
    }
}
