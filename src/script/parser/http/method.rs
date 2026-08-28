use super::Error;
use std::str::FromStr;

/// The seven standard HTTP methods supported by the load generator.
///
/// The variants mirror the IANA HTTP method registry's "common methods" — the
/// set the Lua side is allowed to write in a http spec's `method` field. Any
/// other string is a parser error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl FromStr for HttpMethod {
    type Err = Error;

    /// Parses a string into an [`HttpMethod`].
    ///
    /// The match is case-insensitive against the seven standard HTTP methods.
    ///
    /// # Errors
    ///
    /// Returns `Error::UnknownMethod` if `s` does not match any standard method.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "get" => Ok(Self::Get),
            "post" => Ok(Self::Post),
            "put" => Ok(Self::Put),
            "delete" => Ok(Self::Delete),
            "patch" => Ok(Self::Patch),
            "head" => Ok(Self::Head),
            "options" => Ok(Self::Options),
            _ => Err(Error::UnknownMethod(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_lowercase_get() {
        assert_eq!(
            HttpMethod::from_str("get").expect("lowercase get"),
            HttpMethod::Get
        );
    }

    #[test]
    fn parse_accepts_uppercase_get() {
        assert_eq!(
            HttpMethod::from_str("GET").expect("uppercase GET"),
            HttpMethod::Get
        );
    }

    #[test]
    fn parse_accepts_mixed_case_get() {
        assert_eq!(
            HttpMethod::from_str("GeT").expect("mixed-case GeT"),
            HttpMethod::Get
        );
    }

    #[test]
    fn parse_accepts_lowercase_post() {
        assert_eq!(
            HttpMethod::from_str("post").expect("lowercase post"),
            HttpMethod::Post
        );
    }

    #[test]
    fn parse_accepts_uppercase_post() {
        assert_eq!(
            HttpMethod::from_str("POST").expect("uppercase POST"),
            HttpMethod::Post
        );
    }

    #[test]
    fn parse_accepts_lowercase_put() {
        assert_eq!(
            HttpMethod::from_str("put").expect("lowercase put"),
            HttpMethod::Put
        );
    }

    #[test]
    fn parse_accepts_uppercase_put() {
        assert_eq!(
            HttpMethod::from_str("PUT").expect("uppercase PUT"),
            HttpMethod::Put
        );
    }

    #[test]
    fn parse_accepts_lowercase_delete() {
        assert_eq!(
            HttpMethod::from_str("delete").expect("lowercase delete"),
            HttpMethod::Delete
        );
    }

    #[test]
    fn parse_accepts_uppercase_delete() {
        assert_eq!(
            HttpMethod::from_str("DELETE").expect("uppercase DELETE"),
            HttpMethod::Delete
        );
    }

    #[test]
    fn parse_accepts_lowercase_patch() {
        assert_eq!(
            HttpMethod::from_str("patch").expect("lowercase patch"),
            HttpMethod::Patch
        );
    }

    #[test]
    fn parse_accepts_uppercase_patch() {
        assert_eq!(
            HttpMethod::from_str("PATCH").expect("uppercase PATCH"),
            HttpMethod::Patch
        );
    }

    #[test]
    fn parse_accepts_lowercase_head() {
        assert_eq!(
            HttpMethod::from_str("head").expect("lowercase head"),
            HttpMethod::Head
        );
    }

    #[test]
    fn parse_accepts_uppercase_head() {
        assert_eq!(
            HttpMethod::from_str("HEAD").expect("uppercase HEAD"),
            HttpMethod::Head
        );
    }

    #[test]
    fn parse_accepts_lowercase_options() {
        assert_eq!(
            HttpMethod::from_str("options").expect("lowercase options"),
            HttpMethod::Options
        );
    }

    #[test]
    fn parse_accepts_uppercase_options() {
        assert_eq!(
            HttpMethod::from_str("OPTIONS").expect("uppercase OPTIONS"),
            HttpMethod::Options
        );
    }

    #[test]
    fn parse_rejects_empty_string() {
        let err = HttpMethod::from_str("").expect_err("empty string should fail");
        assert_eq!(err, Error::UnknownMethod(String::new()));
    }

    #[test]
    fn parse_rejects_unknown_method() {
        let err = HttpMethod::from_str("BREW").expect_err("BREW should fail");
        assert_eq!(err, Error::UnknownMethod("BREW".to_string()));
    }

    #[test]
    fn parse_rejects_close_but_wrong_method() {
        // "GETS" is not a standard HTTP method, even if it looks like GET.
        let err = HttpMethod::from_str("GETS").expect_err("GETS should fail");
        assert_eq!(err, Error::UnknownMethod("GETS".to_string()));
    }

    #[test]
    fn parse_error_preserves_original_casing_in_message() {
        let err = HttpMethod::from_str("brew").expect_err("brew should fail");
        // We lowercase internally for the match, but the user-facing error
        // should carry the original (lowercase) input — the script is the
        // source of truth.
        assert_eq!(err, Error::UnknownMethod("brew".to_string()));
    }
}
