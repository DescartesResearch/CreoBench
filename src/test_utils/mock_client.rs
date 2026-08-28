use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use crate::http;
use crate::http::{HttpClient, HttpResponse};
use crate::script::HttpMethod;

#[derive(Debug, Clone, PartialEq)]
pub struct RecordedRequest {
    pub url: String,
    pub method: HttpMethod,
    pub headers: Vec<(String, String)>,
    pub body: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub enum MockResponse {
    Ok(HttpResponse),
    Timeout(String),
    Failed(String),
}

impl MockResponse {
    pub fn ok(response: HttpResponse) -> Self {
        Self::Ok(response)
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    pub fn failed(msg: impl Into<String>) -> Self {
        Self::Failed(msg.into())
    }
}

impl Default for MockResponse {
    fn default() -> Self {
        Self::Ok(HttpResponse {
            status: 200,
            headers: vec![],
            body: None,
        })
    }
}

#[derive(Debug, Default)]
struct Inner {
    requests: Vec<RecordedRequest>,
    response: MockResponse,
    timeout: Duration,
    response_queue: VecDeque<MockResponse>,
    response_delay: Duration,
}

impl MockResponse {
    fn ok_mut(&mut self) -> &mut HttpResponse {
        if let Self::Ok(response) = self {
            return response;
        }
        *self = Self::default();
        match self {
            Self::Ok(response) => response,
            _ => unreachable!("just initialized to Ok"),
        }
    }
}

#[derive(Debug, Default)]
pub struct MockClientBuilder {
    inner: Inner,
}

impl MockClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_status(mut self, status: u16) -> Self {
        self.inner.response.ok_mut().status = status;
        self
    }

    pub fn with_timeout_error(mut self, msg: impl Into<String>) -> Self {
        self.inner.response = MockResponse::Timeout(msg.into());
        self
    }

    pub fn with_failed_error(mut self, msg: impl Into<String>) -> Self {
        self.inner.response = MockResponse::Failed(msg.into());
        self
    }

    pub fn with_timeout_duration(mut self, duration: Duration) -> Self {
        self.inner.timeout = duration;
        self
    }

    pub fn with_response_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.inner.response.ok_mut().headers = headers;
        self
    }

    pub fn with_responses<I>(self, responses: I) -> Self
    where
        I: IntoIterator<Item = HttpResponse>,
    {
        self.with_response_sequence(responses.into_iter().map(MockResponse::Ok))
    }

    pub fn with_response_sequence<I>(mut self, responses: I) -> Self
    where
        I: IntoIterator<Item = MockResponse>,
    {
        self.inner.response_queue = responses.into_iter().collect();
        self
    }

    pub fn with_response_delay(mut self, delay: Duration) -> Self {
        self.inner.response_delay = delay;
        self
    }

    pub fn build(self) -> MockHttpClient {
        MockHttpClient {
            inner: Arc::new(Mutex::new(self.inner)),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MockHttpClient {
    inner: Arc<Mutex<Inner>>,
}

impl MockHttpClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.inner.lock().unwrap().requests.clone()
    }
}

impl HttpClient for MockHttpClient {
    async fn send<'a>(
        &self,
        url: &str,
        method: HttpMethod,
        headers: impl IntoIterator<Item = &'a (String, String)> + Send + 'a,
        body: Option<&serde_json::Value>,
    ) -> Result<HttpResponse, http::RequestError> {
        let (next_response, delay) = {
            let mut inner = self.inner.lock().unwrap();
            inner.requests.push(RecordedRequest {
                url: url.to_owned(),
                method,
                headers: headers
                    .into_iter()
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .collect(),
                body: body.cloned(),
            });
            let next = inner
                .response_queue
                .pop_front()
                .unwrap_or_else(|| inner.response.clone());
            (next, inner.response_delay)
        };

        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }

        match next_response {
            MockResponse::Ok(response) => Ok(response),
            MockResponse::Timeout(msg) => Err(http::RequestError::Timeout(msg)),
            MockResponse::Failed(msg) => Err(http::RequestError::Failed(msg)),
        }
    }

    fn timeout(&self) -> Duration {
        self.inner.lock().unwrap().timeout
    }
}

impl HttpClient for Arc<MockHttpClient> {
    async fn send<'a>(
        &self,
        url: &str,
        method: HttpMethod,
        headers: impl IntoIterator<Item = &'a (String, String)> + Send + 'a,
        body: Option<&serde_json::Value>,
    ) -> Result<HttpResponse, http::RequestError> {
        MockHttpClient::send(self, url, method, headers, body).await
    }

    fn timeout(&self) -> Duration {
        MockHttpClient::timeout(self)
    }
}
