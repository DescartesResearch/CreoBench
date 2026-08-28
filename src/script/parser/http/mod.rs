use std::str::FromStr;

mod error;
mod method;

pub use error::Error;
pub use method::HttpMethod;

use mlua::{Function, Lua, LuaSerdeExt, Table};

use crate::script::HTTPStaticRequestSpec;

use super::{Error as ParseError, Result};

/// Parse a Lua table into an [`HTTPStaticRequestSpec`].
///
/// Required fields: `method`, `service`, `path`.
/// Optional fields: `headers`, `query`, `body`, `extract`.
///
/// # Errors
///
/// - [`Error::Lua`] if reading a Lua value fails.
/// - [`Error::RequiredFieldMissing`] if `method`, `service`, or `path` is absent.
/// - [`Error::FieldNotString`] if a required string field has the wrong Lua type.
/// - [`Error::FieldStringValueNotUTF8`] if a string field is not valid UTF-8.
/// - [`Error::InvalidHTTPSpec`] if `method` is not one of the seven standard HTTP methods.
/// - [`Error::FieldNotTable`] if `headers` or `query` is present but not a table.
/// - [`Error::StringMapValueNotCoercible`] if a value in `headers` or `query` is not string-coercible.
/// - [`Error::BodyNotJson`] if `body` cannot be converted to JSON.
/// - [`Error::FieldNotFunction`] if `extract` is present but not a function.
pub(super) fn parse_http_spec(lua: &Lua, table: &Table) -> Result<HTTPStaticRequestSpec> {
    let method = parse_method(table)?;
    let service = parse_service(table)?;
    let path = parse_path(table)?;
    let headers = parse_string_map(table, "headers")?;
    let query = parse_string_map(table, "query")?;
    let body = parse_body(lua, table)?;
    let extract = parse_extract(table)?;
    Ok(HTTPStaticRequestSpec {
        method,
        service,
        path,
        headers,
        query,
        body,
        extract,
    })
}

fn parse_method(table: &Table) -> Result<HttpMethod> {
    let method: mlua::Value = table.get("method")?;
    match method {
        mlua::Value::Nil => Err(ParseError::RequiredFieldMissing("method")),
        mlua::Value::String(s) => {
            let s = s
                .to_str()
                .map_err(|_| ParseError::FieldStringValueNotUTF8 {
                    field: "method",
                    value: s.clone(),
                })?;
            Ok(HttpMethod::from_str(&s)?)
        }
        other => Err(ParseError::FieldNotString {
            field: "method",
            type_name: other.type_name(),
        }),
    }
}

fn parse_service(table: &Table) -> Result<String> {
    let service: mlua::Value = table.get("service")?;
    match service {
        mlua::Value::Nil => Err(ParseError::RequiredFieldMissing("service")),
        mlua::Value::String(s) => {
            s.to_str()
                .map(|s| s.to_string())
                .map_err(|_| ParseError::FieldStringValueNotUTF8 {
                    field: "service",
                    value: s.clone(),
                })
        }
        other => Err(ParseError::FieldNotString {
            field: "service",
            type_name: other.type_name(),
        }),
    }
}

fn parse_path(table: &Table) -> Result<String> {
    let path: mlua::Value = table.get("path")?;
    match path {
        mlua::Value::Nil => Err(ParseError::RequiredFieldMissing("path")),
        mlua::Value::String(s) => {
            s.to_str()
                .map(|s| s.to_string())
                .map_err(|_| ParseError::FieldStringValueNotUTF8 {
                    field: "path",
                    value: s.clone(),
                })
        }
        other => Err(ParseError::FieldNotString {
            field: "path",
            type_name: other.type_name(),
        }),
    }
}

fn parse_string_map(table: &Table, field: &'static str) -> Result<Vec<(String, String)>> {
    let raw: mlua::Value = table.get(field)?;
    match raw {
        mlua::Value::Nil => Ok(Vec::new()),
        mlua::Value::Table(t) => {
            let mut map = Vec::new();
            for pair in t.pairs::<String, mlua::Value>() {
                let (key, v) = pair?;
                let value =
                    coerce_to_string(&v).ok_or_else(|| ParseError::StringMapValueNotCoercible {
                        field,
                        key: key.clone(),
                        value: v.clone(),
                    })?;
                map.push((key, value));
            }
            Ok(map)
        }
        other => Err(ParseError::FieldNotTable {
            field,
            type_name: other.type_name(),
        }),
    }
}

fn coerce_to_string(v: &mlua::Value) -> Option<String> {
    match v {
        mlua::Value::String(s) => s.to_str().ok().map(|s| s.to_string()),
        mlua::Value::Integer(i) => Some(i.to_string()),
        mlua::Value::Number(n) => Some(n.to_string()),
        mlua::Value::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn parse_body(lua: &Lua, table: &Table) -> Result<Option<serde_json::Value>> {
    let body: mlua::Value = table.get("body")?;
    if body == mlua::Value::Nil {
        return Ok(None);
    }
    let value: serde_json::Value = lua.from_value(body).map_err(ParseError::BodyNotJson)?;

    Ok(Some(value))
}

fn parse_extract(table: &Table) -> Result<Option<Function>> {
    let func: mlua::Value = table.get("extract")?;
    match func {
        mlua::Value::Nil => Ok(None),
        mlua::Value::Function(f) => Ok(Some(f)),
        other => Err(ParseError::FieldNotFunction {
            field: "extract",
            type_name: other.type_name(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    fn make_spec(source: &str) -> (Lua, mlua::Table) {
        let lua = Lua::new();
        let table: mlua::Table = lua.load(source).eval().expect("eval to succeed");
        (lua, table)
    }

    #[test]
    fn parses_valid_http_spec_with_required_fields() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
            }
            "#,
        );

        let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

        assert_eq!(spec.method, HttpMethod::Get);
        assert_eq!(spec.service, "auth");
        assert_eq!(spec.path, "/login");
        assert!(spec.headers.is_empty());
        assert!(spec.query.is_empty());
        assert!(spec.body.is_none());
        assert!(spec.extract.is_none());
    }

    #[test]
    fn parses_valid_http_method_case_insensitively() {
        let specs = [
            r#"
            {
                method = "post",
                service = "auth",
                path = "/login",
            }
            "#,
            r#"
            {
                method = "POST",
                service = "auth",
                path = "/login",
            }
            "#,
            r#"
            {
                method = "PosT",
                service = "auth",
                path = "/login",
            }
            "#,
        ];
        for spec in specs {
            let (lua, table) = make_spec(spec);

            let spec = parse_http_spec(&lua, &table).expect("method should parse");

            assert_eq!(spec.method, HttpMethod::Post);
        }
    }

    #[test]
    fn method_field_is_required() {
        let (lua, table) = make_spec(
            r#"
            {
                service = "auth",
                path = "/login",
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("missing method should fail");
        assert_matches!(err, ParseError::RequiredFieldMissing("method"));
    }

    #[test]
    fn fails_on_unknown_method() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "BREW",
                service = "auth",
                path = "/login",
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("unknown method should fail");
        assert_matches!(err, ParseError::InvalidHTTPSpec(_));
    }

    #[test]
    fn fails_on_non_string_method() {
        let (lua, table) = make_spec(
            r#"
            {
                method = 42,
                service = "auth",
                path = "/login",
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("integer method should fail");
        assert_matches!(
            err,
            ParseError::FieldNotString {
                field: "method",
                ..
            }
        );
    }

    #[test]
    fn service_field_is_required() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                path = "/login",
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("missing service should fail");
        assert_matches!(err, ParseError::RequiredFieldMissing("service"));
    }
    #[test]
    fn fails_on_non_string_service() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                service = 7,
                path = "/login",
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("integer service should fail");
        assert_matches!(
            err,
            ParseError::FieldNotString {
                field: "service",
                ..
            }
        );
    }

    #[test]
    fn path_field_is_required() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                service = "auth",
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("missing path should fail");
        assert_matches!(err, ParseError::RequiredFieldMissing("path"));
    }

    #[test]
    fn fails_on_non_string_path() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                service = "auth",
                path = true,
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("boolean path should fail");
        assert_matches!(err, ParseError::FieldNotString { field: "path", .. });
    }

    #[test]
    fn parses_valid_headers_table() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "get",
                service = "auth",
                path = "/login",
                headers = {
                    Accept = "application/json",
                    ["X-Trace-Id"] = "abc-123",
                    ["Request-Id"] = 123456,
                    Refresh = true,
                },
            }
            "#,
        );

        let mut spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

        assert_eq!(spec.headers.len(), 4);
        spec.headers.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            spec.headers,
            vec![
                ("Accept".to_string(), "application/json".to_string()),
                ("Refresh".to_string(), "true".to_string()),
                ("Request-Id".to_string(), "123456".to_string()),
                ("X-Trace-Id".to_string(), "abc-123".to_string()),
            ]
        );
    }

    #[test]
    fn parses_absent_and_nil_header_table() {
        let specs = [
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
            }
            "#,
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
                headers = nil,
            }
            "#,
        ];
        for spec in specs {
            let (lua, table) = make_spec(spec);

            let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

            assert!(spec.headers.is_empty());
        }
    }

    #[test]
    fn fails_on_invalid_headers() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
                headers = {
                    Accept = { value = "application/json" },
                },
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("string headers should fail");
        assert_matches!(
            err,
            ParseError::StringMapValueNotCoercible {
                field: "headers",
                key,
                ..
            } if key == *"Accept".to_string()
        );
    }

    #[test]
    fn fails_on_non_table_headers() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
                headers = "Accept: text/plain",
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("string headers should fail");
        assert_matches!(
            err,
            ParseError::FieldNotTable {
                field: "headers",
                ..
            }
        );
    }

    #[test]
    fn parses_valid_query_table() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
                query = {
                    n = 3,
                    verbose = true,
                    name = "user-123",
                },
            }
            "#,
        );

        let mut spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

        spec.query.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            spec.query,
            vec![
                ("n".to_string(), "3".to_string()),
                ("name".to_string(), "user-123".to_string()),
                ("verbose".to_string(), "true".to_string()),
            ]
        );
    }

    #[test]
    fn parses_absent_and_nil_query_table() {
        let specs = [
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
            }
            "#,
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
                query = nil,
            }
            "#,
        ];
        for spec in specs {
            let (lua, table) = make_spec(spec);

            let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

            assert!(spec.query.is_empty());
        }
    }

    #[test]
    fn fails_on_invalid_query() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
                query = {
                    filter = { a = 1 },
                },
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("table value should fail");
        assert_matches!(
            err,
            ParseError::StringMapValueNotCoercible {
                field: "query",
                key,
                ..
            } if key == *"filter".to_string()
        );
    }

    #[test]
    fn fails_on_non_table_query() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
                query = "n=3",
            }
            "#,
        );

        let err = parse_http_spec(&lua, &table).expect_err("string query should fail");
        assert_matches!(err, ParseError::FieldNotTable { field: "query", .. });
    }

    #[test]
    fn parses_valid_string_body() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "POST",
                service = "auth",
                path = "/login",
                body = "raw text",
            }
            "#,
        );

        let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");
        assert_eq!(
            spec.body,
            Some(serde_json::Value::String("raw text".to_string()))
        );
    }

    #[test]
    fn parses_valid_number_body() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "POST",
                service = "auth",
                path = "/login",
                body = 42,
            }
            "#,
        );

        let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");
        assert_eq!(spec.body, Some(serde_json::Value::Number(42.into())));
    }

    #[test]
    fn parses_valid_boolean_body() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "POST",
                service = "auth",
                path = "/login",
                body = true,
            }
            "#,
        );

        let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");
        assert_eq!(spec.body, Some(serde_json::Value::Bool(true)));
    }

    #[test]
    fn parses_valid_object_body() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "POST",
                service = "auth",
                path = "/login",
                body = { username = "demo", password = "secret" },
            }
            "#,
        );

        let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

        assert_eq!(
            spec.body,
            Some(serde_json::json!({
                "username": "demo",
                "password": "secret",
            }))
        );
    }

    #[test]
    fn parses_nested_body() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "POST",
                service = "auth",
                path = "/login",
                body = {
                    user = {
                        name = "demo",
                        roles = { "admin", "user" },
                    },
                },
            }
            "#,
        );

        let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

        assert_eq!(
            spec.body,
            Some(serde_json::json!({
                "user": {
                    "name": "demo",
                    "roles": ["admin", "user"],
                },
            }))
        );
    }

    #[test]
    fn body_field_is_not_required() {
        let specs = [
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
            }
            "#,
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
                body = nil,
            }
            "#,
        ];
        for spec in specs {
            let (lua, table) = make_spec(spec);

            let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

            assert!(spec.body.is_none());
        }
    }

    #[test]
    fn parses_valid_extract() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "POST",
                service = "auth",
                path = "/login",
                extract = function(response) end,
            }
            "#,
        );

        let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

        assert!(spec.extract.is_some());
    }

    #[test]
    fn extract_field_is_not_required() {
        let specs = [
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
            }
            "#,
            r#"
            {
                method = "GET",
                service = "auth",
                path = "/login",
                extract = nil,
            }
            "#,
        ];
        for spec in specs {
            let (lua, table) = make_spec(spec);

            let spec = parse_http_spec(&lua, &table).expect("valid spec should parse");

            assert!(spec.extract.is_none());
        }
    }

    #[test]
    fn fails_on_non_function_extract() {
        let specs = [
            r#"
            {
                method = "POST",
                service = "auth",
                path = "/login",
                extract = "not a function",
            }
            "#,
            r#"
            {
                method = "POST",
                service = "auth",
                path = "/login",
                extract = { handler = function() end },
            }
            "#,
        ];
        for spec in specs {
            let (lua, table) = make_spec(spec);

            let err = parse_http_spec(&lua, &table).expect_err("string extract should fail");
            assert_matches!(
                err,
                ParseError::FieldNotFunction {
                    field: "extract",
                    ..
                }
            );
        }
    }

    #[test]
    fn parses_full_http_spec() {
        let (lua, table) = make_spec(
            r#"
            {
                method = "DELETE",
                service = "user",
                path = "",
                headers = {
                    Accept = "application/json",
                    ["X-Trace-Id"] = "abc-123",
                    ["Request-Id"] = 123456,
                    Refresh = true,
                },
                query = {
                    n = 3,
                    verbose = true,
                    name = "user-123",
                },
                body = {
                    user = {
                        name = "demo",
                        roles = { "admin", "user" },
                    },
                },
                extract = function(response) end,
            }
            "#,
        );

        let mut spec = parse_http_spec(&lua, &table).expect("valid spec should parse");
        assert_eq!(spec.method, HttpMethod::Delete);
        assert_eq!(spec.service, "user");
        assert_eq!(spec.path, "");
        spec.headers.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            spec.headers,
            vec![
                ("Accept".to_string(), "application/json".to_string()),
                ("Refresh".to_string(), "true".to_string()),
                ("Request-Id".to_string(), "123456".to_string()),
                ("X-Trace-Id".to_string(), "abc-123".to_string()),
            ]
        );
        spec.query.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            spec.query,
            vec![
                ("n".to_string(), "3".to_string()),
                ("name".to_string(), "user-123".to_string()),
                ("verbose".to_string(), "true".to_string()),
            ]
        );
        assert_eq!(
            spec.body,
            Some(serde_json::json!({
                "user": {
                    "name": "demo",
                    "roles": ["admin", "user"],
                },
            }))
        );
        assert!(spec.extract.is_some());
    }
}
