use bytes::Bytes;
use mlua::{LuaSerdeExt, UserData, UserDataMethods};

/// Lua-facing HTTP response handle.
///
/// Passed to a script's `extract` function. Exposes three methods to Lua:
///
/// | Method                  | Returns             | Notes                                                                                             |
/// | :---                    | :---:               | :---                                                                                              |
/// | `response:status()`     | `u16`               | The HTTP status code                                                                              |
/// | `response:header(name)` | `string?`           | Case-insensitive; `nil` if absent; duplicates joined with `", "` per RFC 7230                     |
/// | `response:json()`       | `(table?, string?)` | `(data?, nil)` on success (Note: data may still be nil if body is empty), `(nil, err)` on failure |
#[derive(Debug, Clone)]
pub struct Response {
    /// The HTTP status code.
    status: u16,
    /// Response headers in declaration order (name, value).
    headers: Vec<(String, String)>,
    /// Raw response body bytes, or `None` for an empty body.
    body: Option<Bytes>,
}

impl Response {
    /// Builds a [`Response`] from raw HTTP fields.
    pub fn new(status: u16, headers: Vec<(String, String)>, body: Option<Bytes>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }
}

impl UserData for Response {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("status", |_lua, this, ()| Ok(this.status));

        methods.add_method("header", |_lua, this, name: String| {
            let mut matches = this
                .headers
                .iter()
                .filter(|(k, _)| k.eq_ignore_ascii_case(&name))
                .map(|(_, v)| v.as_str())
                .peekable();

            if matches.peek().is_none() {
                Ok(None::<String>)
            } else {
                let acc = matches.next().unwrap_or_default();
                Ok(Some(matches.fold(acc.to_owned(), |mut acc, b| {
                    acc.reserve(b.len() + 1);
                    acc.push(',');
                    acc.push_str(b);
                    acc
                })))
            }
        });

        methods.add_method(
            "json",
            |lua, this, ()| -> mlua::Result<(Option<mlua::Value>, Option<String>)> {
                let Some(body) = this.body.as_ref() else {
                    return Ok((None, None));
                };
                let value: serde_json::Value = match serde_json::from_slice(body) {
                    Ok(v) => v,
                    Err(e) => return Ok((None, Some(format!("failed to parse JSON body: {e}")))),
                };
                let value = lua.to_value(&value)?;
                Ok((Some(value), None))
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;

    fn create_lua_runtime(response: Response) -> Lua {
        let lua = Lua::new();
        lua.globals().set("response", response).unwrap();
        lua
    }

    #[test]
    fn status_returns_the_status_code() {
        let lua = create_lua_runtime(Response::new(404, Vec::new(), None));

        let value: u16 = lua.load("return response:status()").eval().unwrap();

        assert_eq!(value, 404);
    }

    #[test]
    fn header_returns_value_for_existing_header() {
        let lua = create_lua_runtime(Response::new(
            200,
            vec![("Content-Type".to_string(), "application/json".to_string())],
            None,
        ));

        let value: String = lua
            .load(r#"return response:header("Content-Type")"#)
            .eval()
            .unwrap();

        assert_eq!(value, "application/json");
    }

    #[test]
    fn header_is_case_insensitive() {
        let lua = create_lua_runtime(Response::new(
            200,
            vec![("Content-Type".to_string(), "application/json".to_string())],
            None,
        ));

        let lower: String = lua
            .load(r#"return response:header("content-type")"#)
            .eval()
            .unwrap();
        let upper: String = lua
            .load(r#"return response:header("CONTENT-TYPE")"#)
            .eval()
            .unwrap();
        let mixed: String = lua
            .load(r#"return response:header("cOnTeNt-TyPe")"#)
            .eval()
            .unwrap();

        assert_eq!(lower, "application/json");
        assert_eq!(upper, "application/json");
        assert_eq!(mixed, "application/json");
    }

    #[test]
    fn header_returns_nil_for_missing_header() {
        let lua = create_lua_runtime(Response::new(
            200,
            vec![("Content-Type".to_string(), "application/json".to_string())],
            None,
        ));

        let value: mlua::Value = lua
            .load(r#"return response:header("X-Missing")"#)
            .eval()
            .unwrap();

        assert_eq!(value, mlua::Value::Nil);
    }

    #[test]
    fn header_joins_duplicate_values_with_comma_space() {
        let lua = create_lua_runtime(Response::new(
            200,
            vec![
                ("Set-Cookie".to_string(), "a=1".to_string()),
                ("Set-Cookie".to_string(), "b=2".to_string()),
                ("Set-Cookie".to_string(), "c=3".to_string()),
            ],
            None,
        ));

        let value: String = lua
            .load(r#"return response:header("Set-Cookie")"#)
            .eval()
            .expect("eval should succeed");

        assert_eq!(value, "a=1,b=2,c=3");
    }

    #[test]
    fn json_succeeds_for_valid_object_body() {
        let lua = create_lua_runtime(Response::new(
            200,
            Vec::new(),
            Some(br#"{"name": "demo", "n": 3}"#.as_slice().into()),
        ));

        let (value, err): (mlua::Value, mlua::Value) =
            lua.load("return response:json()").eval().unwrap();

        assert_eq!(err, mlua::Nil);

        let table = match value {
            mlua::Value::Table(t) => t,
            other => panic!("expected table, got {}", other.type_name()),
        };
        let name: String = table.get("name").unwrap();
        let n: i64 = table.get("n").unwrap();
        assert_eq!(name, "demo");
        assert_eq!(n, 3);
    }

    #[test]
    fn json_succeeds_for_empty_body() {
        let lua = create_lua_runtime(Response::new(200, Vec::new(), None));

        let (value, err): (mlua::Value, mlua::Value) =
            lua.load("return response:json()").eval().unwrap();

        assert_eq!(value, mlua::Nil);
        assert_eq!(err, mlua::Nil);
    }

    #[test]
    fn json_returns_nil_and_error_for_malformed_body() {
        let lua = create_lua_runtime(Response::new(
            200,
            Vec::new(),
            Some(br#"{"name": "demo", "n": 3"#.as_slice().into()),
        ));

        let (data, err): (mlua::Value, String) = lua.load("return response:json()").eval().unwrap();

        assert_eq!(data, mlua::Value::Nil);
        assert!(!err.is_empty());
    }
}
