use cookie_store::CookieStore;
use url::Url;

#[derive(Debug, Clone)]
pub struct CookieJar {
    store: CookieStore,
}

impl Default for CookieJar {
    fn default() -> Self {
        Self {
            store: CookieStore::new_with_public_suffix(None),
        }
    }
}

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cookie(&self, url: &Url) -> Option<(String, String)> {
        use std::fmt::Write;

        let mut iter = self.store.matches(url).into_iter();
        let first = iter.next()?;

        let mut value = String::new();
        let _ = write!(value, "{}", first.encoded());
        for cookie in iter {
            let _ = write!(value, "; {}", cookie.encoded());
        }
        Some(("Cookie".to_string(), value))
    }

    pub fn store_cookies(&mut self, headers: &[(String, String)], url: &Url) {
        for (name, value) in headers {
            if name.eq_ignore_ascii_case("set-cookie")
                && let Err(err) = self.store.parse(value, url)
            {
                tracing::warn!(
                    error = %err,
                    url = %url,
                    "failed to parse Set-Cookie header from {url}: {err}",
                );
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).expect("test URL must parse")
    }

    #[test]
    fn empty_jar_produces_no_cookie_header() {
        let jar = CookieJar::new();
        let target = url("http://api.example.com/test");

        let cookie = jar.cookie(&target);

        assert!(cookie.is_none());
    }

    #[test]
    fn capture_then_apply_roundtrip_emits_captured_cookie() {
        let mut jar = CookieJar::new();
        let target = url("http://api.example.com/test");
        let response_headers = vec![("Set-Cookie".to_string(), "sid=abc123".to_string())];

        jar.store_cookies(&response_headers, &target);
        let cookie = jar.cookie(&target);

        assert_eq!(
            cookie,
            Some(("Cookie".to_string(), "sid=abc123".to_string()))
        );
    }

    #[test]
    fn multiple_set_cookie_headers_all_feed_the_jar() {
        let mut jar = CookieJar::new();
        let target = url("http://api.example.com/test");
        let response_headers = vec![
            ("Set-Cookie".to_string(), "session=abc".to_string()),
            ("Set-Cookie".to_string(), "csrf=xyz".to_string()),
        ];

        jar.store_cookies(&response_headers, &target);

        let (_, value) = jar.cookie(&target).unwrap();
        assert!(
            value.contains("session=abc"),
            "missing session, got {value:?}"
        );
        assert!(value.contains("csrf=xyz"), "missing csrf, got {value:?}");
    }

    #[test]
    fn capture_ignores_non_set_cookie_headers() {
        let mut jar = CookieJar::new();
        let target = url("http://api.example.com/test");
        let response_headers = vec![
            ("Content-Type".to_string(), "text/html".to_string()),
            ("X-Trace-Id".to_string(), "trace-42".to_string()),
            ("Set-Cookie".to_string(), "sid=only-real-cookie".to_string()),
        ];

        jar.store_cookies(&response_headers, &target);
        let cookie = jar.cookie(&target);

        assert_eq!(
            cookie,
            Some(("Cookie".to_string(), "sid=only-real-cookie".to_string()))
        );
    }

    #[test]
    fn expired_cookies_are_dropped_when_applied() {
        let mut jar = CookieJar::new();
        let target = url("http://api.example.com/test");

        // A long-lived cookie proves the apply path picks up cookies that
        // are still within their validity window.
        jar.store_cookies(
            &[(
                "Set-Cookie".to_string(),
                "long=keep; Max-Age=3600".to_string(),
            )],
            &target,
        );
        // The short-lived cookie is captured, then immediately expired by a
        // follow-up Set-Cookie with Max-Age=0 — the RFC6265 §5.3 step 11
        // mechanism a server uses to delete a cookie. This avoids the clock
        // seam needed to test natural time-based expiry while still
        // exercising the underlying cookie_store's expired-cookie removal.
        jar.store_cookies(
            &[("Set-Cookie".to_string(), "short=drop".to_string())],
            &target,
        );
        jar.store_cookies(
            &[("Set-Cookie".to_string(), "short=; Max-Age=0".to_string())],
            &target,
        );

        let (_, value) = jar.cookie(&target).unwrap();

        assert!(value.contains("long=keep"), "missing long, got {value:?}");
        assert!(
            !value.contains("short"),
            "expired short cookie must not appear, got {value:?}"
        );
    }

    #[test]
    fn cookies_from_one_domain_do_not_match_another_domain() {
        let mut jar = CookieJar::new();
        let api_url = url("http://api.example.com/login");
        let cdn_url = url("http://cdn.example.com/asset");

        jar.store_cookies(
            &[("Set-Cookie".to_string(), "sid=secret".to_string())],
            &api_url,
        );

        let cookies_for_cdn = jar.cookie(&cdn_url);
        assert!(
            cookies_for_cdn.is_none(),
            "cross-domain leak: cookie from api.example.com must not match cdn.example.com"
        );

        let (_, value) = jar.cookie(&api_url).unwrap();
        assert_eq!(value, "sid=secret");
    }

    #[test]
    fn http_only_cookie_is_captured_and_applied() {
        let mut jar = CookieJar::new();
        let target = url("http://api.example.com/test");
        jar.store_cookies(
            &[(
                "Set-Cookie".to_string(),
                "session=secret; HttpOnly".to_string(),
            )],
            &target,
        );
        let (_, value) = jar.cookie(&target).unwrap();
        assert!(
            value.contains("session=secret"),
            "HttpOnly cookie must still be captured, got {value:?}"
        );
    }

    #[test]
    fn http_only_cookie_combined_with_path_and_max_age() {
        let mut jar = CookieJar::new();
        let target = url("http://api.example.com/test");
        jar.store_cookies(
            &[(
                "Set-Cookie".to_string(),
                "auth=token123; Path=/; Max-Age=3600; HttpOnly".to_string(),
            )],
            &target,
        );
        let (_, value) = jar.cookie(&target).unwrap();
        assert!(
            value.contains("auth=token123"),
            "HttpOnly cookie with attributes must be captured, got {value:?}"
        );
    }

    #[test]
    fn secure_cookie_is_sent_over_https() {
        let mut jar = CookieJar::new();
        let target = url("https://api.example.com/test");
        jar.store_cookies(
            &[("Set-Cookie".to_string(), "token=xyz; Secure".to_string())],
            &target,
        );
        let (_, value) = jar.cookie(&target).unwrap();
        assert!(
            value.contains("token=xyz"),
            "Secure cookie must be sent over HTTPS, got {value:?}"
        );
    }

    #[test]
    fn secure_cookie_is_not_sent_over_http() {
        let mut jar = CookieJar::new();
        let https_target = url("https://api.example.com/test");
        jar.store_cookies(
            &[("Set-Cookie".to_string(), "token=xyz; Secure".to_string())],
            &https_target,
        );
        let http_target = url("http://api.example.com/test");
        let cookie = jar.cookie(&http_target);
        assert!(
            cookie.is_none(),
            "Secure cookie must not be sent over plain HTTP"
        );
    }

    #[test]
    fn percent_encoded_cookie_values_roundtrip() {
        let mut jar = CookieJar::new();
        let target = url("http://api.example.com/test");
        jar.store_cookies(
            &[("Set-Cookie".to_string(), "data=hello world".to_string())],
            &target,
        );
        let (_, value) = jar.cookie(&target).unwrap();
        assert!(
            value.contains("data=hello%20world"),
            "expected percent-encoded cookie value, got {value:?}"
        );
    }
}
