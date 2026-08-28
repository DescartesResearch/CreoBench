use bytes::Bytes;

use crate::script::HttpMethod;
mod error;
pub use error::{Error, RequestError};

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// HTTP response returned by [`HttpClient`] implementations.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<Bytes>,
}

/// Trait for HTTP client abstraction.
pub trait HttpClient: Send + Sync {
    fn send<'a>(
        &self,
        url: &str,
        method: HttpMethod,
        headers: impl IntoIterator<Item = &'a (String, String)> + Send + 'a,
        body: Option<&serde_json::Value>,
    ) -> impl Future<Output = Result<HttpResponse, RequestError>> + Send;

    fn timeout(&self) -> std::time::Duration;
}

/// HttpClient implementation using [`reqwest::Client`].
#[derive(Debug, Default, Clone)]
pub struct ReqwestHttpClient {
    client: reqwest::Client,
    timeout: std::time::Duration,
}

impl ReqwestHttpClient {
    pub fn with_timeout(timeout: u64) -> Result<Self, Error> {
        let mut builder = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .no_proxy()
            .pool_idle_timeout(None)
            .tcp_nodelay(true);
        let timeout = std::time::Duration::from_millis(timeout);
        if timeout != std::time::Duration::ZERO {
            builder = builder.timeout(timeout);
        }
        Ok(Self {
            client: builder
                .build()
                .map_err(|err| Error::Client(err.to_string()))?,
            timeout,
        })
    }
}

impl HttpClient for ReqwestHttpClient {
    async fn send<'a>(
        &self,
        url: &str,
        method: HttpMethod,
        headers: impl IntoIterator<Item = &'a (String, String)> + Send + 'a,
        body: Option<&serde_json::Value>,
    ) -> Result<HttpResponse, RequestError> {
        let mut request = match method {
            HttpMethod::Get => self.client.get(url),
            HttpMethod::Post => self.client.post(url),
            HttpMethod::Put => self.client.put(url),
            HttpMethod::Delete => self.client.delete(url),
            HttpMethod::Patch => self.client.patch(url),
            HttpMethod::Head => self.client.head(url),
            HttpMethod::Options => self.client.request(reqwest::Method::OPTIONS, url),
        };

        for (key, value) in headers {
            request = request.header(key, value);
        }

        if let Some(body) = body {
            request = request.json(body);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(err) if err.is_timeout() => return Err(RequestError::Timeout(err.to_string())),
            Err(err) => return Err(RequestError::Failed(err.to_string())),
        };
        let status = response.status().as_u16();

        let response_headers = response
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|v| (k.as_str().to_string(), v.to_string()))
            })
            .collect();

        let response_body = match response.bytes().await {
            Ok(body) if body.is_empty() => None,
            Ok(body) => Some(body),
            Err(err) if err.is_timeout() => return Err(RequestError::Timeout(err.to_string())),
            Err(err) => return Err(RequestError::Failed(err.to_string())),
        };

        Ok(HttpResponse {
            status,
            headers: response_headers,
            body: response_body,
        })
    }

    fn timeout(&self) -> std::time::Duration {
        self.timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore = "Ignore for now, httpbin may return 503 and fail this tests unrelated to a broken implementation"]
    #[tokio::test]
    async fn reqwest_transport_sends_http_request() {
        let transport = ReqwestHttpClient::default();

        let url = "https://httpbin.org/get";
        let method = HttpMethod::Get;
        let headers = &[("X-Test-Header".to_string(), "test-value".to_string())];
        let body = None;

        let response = transport.send(url, method, headers, body).await.unwrap();

        assert_eq!(response.status, 200);
        assert!(!response.headers.is_empty());
    }

    #[ignore = "Ignore for now, httpbin may return 503 and fail this tests unrelated to a broken implementation"]
    #[tokio::test]
    async fn reqwest_transport_handles_post_request() {
        let transport = ReqwestHttpClient::default();

        let json = serde_json::json!({"test_key": "test_value"});
        let url = "https://httpbin.org/post";
        let method = HttpMethod::Post;
        let headers = &[("Content-Type".to_string(), "application/json".to_string())];

        let response = transport
            .send(url, method, headers, Some(&json))
            .await
            .unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.is_some());
        let body_bytes = response.body.unwrap();
        let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body_json["json"], json);
    }
}
