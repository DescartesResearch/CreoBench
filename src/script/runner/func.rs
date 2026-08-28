use mlua::{Function, Lua};

use crate::script::spec::LuaRequestSpec;
use crate::script::store::Store;
use crate::script::{parser, spec};

/// Call a top-level script function and parse its return table into specs.
///
/// The function receives the [`Store`] as its argument and must return a
/// table. Each element is parsed via [`parser::parse_spec`].
///
/// # Errors
///
/// Returns [`CallError::Call`] on Lua error, [`CallError::NonTableReturn`]
/// if the return value is not a table, or [`CallError::InvalidSpec`] for
/// a malformed spec entry.
pub fn call_top_level_func(
    lua: &Lua,
    f: &Function,
    store: Store,
) -> Result<Vec<LuaRequestSpec>, CallError> {
    let result: mlua::Value = f.call(store).map_err(CallError::Call)?;
    let result = match result {
        mlua::Value::Table(t) => t,
        other => return Err(CallError::NonTableReturn(other.type_name())),
    };
    let mut specs = Vec::with_capacity(result.raw_len());
    for (index, value) in result.sequence_values::<mlua::Value>().enumerate() {
        let spec = parser::parse_spec(lua, value?).map_err(|e| CallError::InvalidSpec {
            index: index + 1,
            source: e,
        })?;
        specs.push(spec);
    }
    Ok(specs)
}

fn coerce_to_string(value: mlua::Value) -> Result<String, CallError> {
    match value {
        mlua::Value::String(ref s) => Ok(s
            .to_str()
            .map_err(|_| CallError::NonUTF8String {
                value: value.clone(),
            })?
            .to_string()),
        other => {
            let type_name = other.type_name();
            Err(CallError::NotStringCoercible {
                value: other,
                type_name,
            })
        }
    }
}

/// The result of calling a dynamic request-spec function.
pub enum DynamicResolve {
    /// The function returned a valid request spec.
    Spec(spec::RequestSpec),
    /// The function returned a string — a jump to a named spec.
    Jump(String),
}

/// Call a dynamic spec function and interpret its return value.
///
/// The function receives the [`Store`] as its argument. If it returns a
/// table, it is parsed as a static spec. Otherwise the value is coerced to
/// a string (the target name for a jump).
///
/// # Errors
///
/// - [`CallError::Call`] on a Lua runtime error during the function call.
/// - [`CallError::NotStringCoercible`] if the return value is not a string, number, or boolean.
/// - [`CallError::NonUTF8String`] if a string return value is not valid UTF-8.
/// - [`CallError::InvalidSpec`] if the return value is a malformed spec table.
pub fn call_dynamic_func(
    index: usize,
    f: &Function,
    lua: &Lua,
    store: Store,
) -> Result<DynamicResolve, CallError> {
    let value: mlua::Value = f.call(store).map_err(CallError::Call)?;

    match value {
        mlua::Value::Table(ref t) => Ok(DynamicResolve::Spec(
            parser::parse_static_spec(lua, t)
                .map_err(|e| CallError::InvalidSpec { index, source: e })?,
        )),
        other => Ok(DynamicResolve::Jump(coerce_to_string(other)?)),
    }
}

/// Errors that can occur running a top-level function.
#[derive(Debug, thiserror::Error)]
pub enum CallError {
    /// An unexpected Lua runtime error occurred.
    #[error("unexpected Lua error: {0}")]
    Lua(#[from] mlua::Error),

    /// A Lua function call raised an error at runtime.
    #[error("an error occurred during the function call: {0}")]
    Call(#[source] mlua::Error),

    /// A top-level function did not return a table as required.
    #[error("invalid return type: expected `table`, but got type `{0}`")]
    NonTableReturn(&'static str),

    /// A dynamic function returned a value that is not
    /// coercible to a string (the target name for a jump).
    #[error(
        "invalid type for jump return value `{value:?}`: expected string coercible type, but got type `{type_name}`"
    )]
    NotStringCoercible {
        value: mlua::Value,
        type_name: &'static str,
    },

    /// A dynamic function returned a string that is not valid UTF-8.
    #[error("jump return value `{value:?}` is not a valid UTF-8 string")]
    NonUTF8String { value: mlua::Value },

    /// A spec entry in the returned table is malformed.
    #[error("invalid spec at index `{index}` (1-based): {source}")]
    InvalidSpec { index: usize, source: parser::Error },
}

#[cfg(test)]
mod tests {
    use mlua::FromLua;
    use std::assert_matches;

    use crate::script::parser::http::HttpMethod;
    use crate::script::runner::loader;
    use crate::script::spec::{RequestSpec, SpecType};

    use super::*;

    #[test]
    fn setup_succeeds_on_empty_table() {
        let source = r#"
            local function setup()
                return {}
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let specs =
            call_top_level_func(&script.lua, &script.setup, store).expect("empty setup should run");

        assert!(specs.is_empty());
    }

    #[test]
    fn requests_succeeds_on_empty_table() {
        let source = r#"
            local function setup()
                return {}
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let specs = call_top_level_func(&script.lua, &script.requests, store)
            .expect("empty setup should run");

        assert!(specs.is_empty());
    }

    #[test]
    fn setup_succeeds_on_static_setup_table() {
        let source = r#"
            local function setup()
                return {
                    {
                        protocol = "http",
                        method  = "POST",
                        service = "auth",
                        path    = "/login",
                        headers = {
                            ["Content-Type"] = "application/json",
                        },
                        body    = { username = "demo", password = "secret" },
                        extract = function(response) end,
                    },
                }
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let specs =
            call_top_level_func(&script.lua, &script.setup, store).expect("setup should run");

        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        let SpecType::Static(RequestSpec::Http(http_spec)) = &spec.spec else {
            panic!("expected static HTTP spec");
        };
        assert_eq!(http_spec.method, HttpMethod::Post);
        assert_eq!(http_spec.service, "auth");
        assert_eq!(http_spec.path, "/login");
        assert_eq!(
            http_spec.headers,
            vec![("Content-Type".to_string(), "application/json".to_string())]
        );
        assert!(http_spec.query.is_empty());
        assert_eq!(
            http_spec.body,
            Some(serde_json::json!({
                "username": "demo",
                "password": "secret",
            }))
        );
        assert!(http_spec.extract.is_some());
    }

    #[test]
    fn requests_succeeds_on_static_setup_table() {
        let source = r#"
            local function requests()
                return {
                    {
                        protocol = "http",
                        method  = "POST",
                        service = "auth",
                        path    = "/login",
                        headers = {
                            ["Content-Type"] = "application/json",
                        },
                        body    = { username = "demo", password = "secret" },
                        extract = function(response) end,
                    },
                }
            end
            local function setup()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let specs =
            call_top_level_func(&script.lua, &script.requests, store).expect("setup should run");

        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        let SpecType::Static(RequestSpec::Http(http_spec)) = &spec.spec else {
            panic!("expected static HTTP spec");
        };
        assert_eq!(http_spec.method, HttpMethod::Post);
        assert_eq!(http_spec.service, "auth");
        assert_eq!(http_spec.path, "/login");
        assert_eq!(
            http_spec.headers,
            vec![("Content-Type".to_string(), "application/json".to_string())]
        );
        assert!(http_spec.query.is_empty());
        assert_eq!(
            http_spec.body,
            Some(serde_json::json!({
                "username": "demo",
                "password": "secret",
            }))
        );
        assert!(http_spec.extract.is_some());
    }

    #[test]
    fn setup_succeeds_on_dynamic_setup_table() {
        let source = r#"
            local function setup()
                return {
                    function()
                        return {
                            protocol = "http",
                            method  = "POST",
                            service = "auth",
                            path    = "/login",
                            headers = {
                                ["Content-Type"] = "application/json",
                            },
                            body    = { username = "demo", password = "secret" },
                            extract = function(response) end,
                        }
                    end,
                }
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let specs =
            call_top_level_func(&script.lua, &script.setup, store).expect("setup should run");

        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        assert_matches!(spec.spec, SpecType::Dynamic(_));
    }

    #[test]
    fn requests_succeeds_on_dynamic_setup_table() {
        let source = r#"
            local function requests()
                return {
                    function()
                        return {
                            protocol = "http",
                            method  = "POST",
                            service = "auth",
                            path    = "/login",
                            headers = {
                                ["Content-Type"] = "application/json",
                            },
                            body    = { username = "demo", password = "secret" },
                            extract = function(response) end,
                        }
                    end,
                }
            end
            local function setup()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let specs =
            call_top_level_func(&script.lua, &script.requests, store).expect("setup should run");

        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        assert_matches!(spec.spec, SpecType::Dynamic(_));
    }

    #[test]
    fn setup_can_use_store() {
        let source = r#"
            local function setup(store)
                store:set("string", "hello")
                store:set("number", 123)
                store:set("bool", true)
                store:set("float", 123.456)
                local f = function() end
                store:set("func", f)
                local t = {}
                store:set("table", t)

                assert(store:get("string") == "hello")
                assert(store:get("number") == 123)
                assert(store:get("bool") == true)
                assert(store:get("float") == 123.456)
                assert(store:get("func") == f)
                assert(store:get("table") == t)
                return { }
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let _ = call_top_level_func(&script.lua, &script.setup, store.clone())
            .expect("setup should run");

        assert_eq!(
            String::from_lua(store.get("string").unwrap(), &script.lua).unwrap(),
            "hello".to_string()
        );
        assert_eq!(
            u32::from_lua(store.get("number").unwrap(), &script.lua).unwrap(),
            123
        );
        assert!(bool::from_lua(store.get("bool").unwrap(), &script.lua).unwrap(),);
        assert_eq!(
            f32::from_lua(store.get("float").unwrap(), &script.lua).unwrap(),
            123.456
        );
        assert_matches!(store.get("func").unwrap(), mlua::Value::Function(_));
        assert_matches!(store.get("table").unwrap(), mlua::Value::Table(_));
    }

    #[test]
    fn requests_can_use_store() {
        let source = r#"
            local function requests(store)
                store:set("string", "hello")
                store:set("number", 123)
                store:set("bool", true)
                store:set("float", 123.456)
                local f = function() end
                store:set("func", f)
                local t = {}
                store:set("table", t)

                assert(store:get("string") == "hello")
                assert(store:get("number") == 123)
                assert(store:get("bool") == true)
                assert(store:get("float") == 123.456)
                assert(store:get("func") == f)
                assert(store:get("table") == t)
                return { }
            end
            local function setup()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let _ = call_top_level_func(&script.lua, &script.requests, store.clone())
            .expect("setup should run");

        assert_eq!(
            String::from_lua(store.get("string").unwrap(), &script.lua).unwrap(),
            "hello".to_string()
        );
        assert_eq!(
            u32::from_lua(store.get("number").unwrap(), &script.lua).unwrap(),
            123
        );
        assert!(bool::from_lua(store.get("bool").unwrap(), &script.lua).unwrap(),);
        assert_eq!(
            f32::from_lua(store.get("float").unwrap(), &script.lua).unwrap(),
            123.456
        );
        assert_matches!(store.get("func").unwrap(), mlua::Value::Function(_));
        assert_matches!(store.get("table").unwrap(), mlua::Value::Table(_));
    }

    #[test]
    fn setup_succeeds_for_array_style_table() {
        // Array-style syntax preserves declaration order; each entry's
        // name is its numeric key.
        let source = r#"
            local function setup()
                return {
                    {
                        protocol = "http",
                        method  = "GET",
                        service = "auth",
                        path    = "/login",
                    },
                    {
                        protocol = "http",
                        method  = "POST",
                        service = "service-1",
                        path    = "/create",
                        body    = { title = "Hello", content = "World" },
                    },
                    {
                        protocol = "http",
                        method  = "DELETE",
                        service = "service-1",
                        path    = "/items/42",
                    },
                }
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let specs =
            call_top_level_func(&script.lua, &script.setup, store).expect("setup should run");

        let SpecType::Static(RequestSpec::Http(ref http_spec)) = specs[0].spec else {
            panic!("expected static HTTP spec");
        };
        assert_eq!(http_spec.method, HttpMethod::Get);
        let SpecType::Static(RequestSpec::Http(ref http_spec)) = specs[1].spec else {
            panic!("expected static HTTP spec");
        };
        assert_eq!(http_spec.method, HttpMethod::Post);
        assert_eq!(
            http_spec.body,
            Some(serde_json::json!({"title": "Hello", "content": "World"}))
        );
        let SpecType::Static(RequestSpec::Http(ref http_spec)) = specs[2].spec else {
            panic!("expected static HTTP spec");
        };
        assert_eq!(http_spec.method, HttpMethod::Delete);
    }

    #[test]
    fn requests_succeeds_for_array_style_table() {
        // Array-style syntax preserves declaration order; each entry's
        // name is its numeric key.
        let source = r#"
            local function requests()
                return {
                    {
                        protocol = "http",
                        method  = "GET",
                        service = "auth",
                        path    = "/login",
                    },
                    {
                        protocol = "http",
                        method  = "POST",
                        service = "service-1",
                        path    = "/create",
                        body    = { title = "Hello", content = "World" },
                    },
                    {
                        protocol = "http",
                        method  = "DELETE",
                        service = "service-1",
                        path    = "/items/42",
                    },
                }
            end
            local function setup()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let specs =
            call_top_level_func(&script.lua, &script.requests, store).expect("setup should run");

        let SpecType::Static(RequestSpec::Http(ref http_spec)) = specs[0].spec else {
            panic!("expected static HTTP spec");
        };
        assert_eq!(http_spec.method, HttpMethod::Get);
        let SpecType::Static(RequestSpec::Http(ref http_spec)) = specs[1].spec else {
            panic!("expected static HTTP spec");
        };
        assert_eq!(http_spec.method, HttpMethod::Post);
        assert_eq!(
            http_spec.body,
            Some(serde_json::json!({"title": "Hello", "content": "World"}))
        );
        let SpecType::Static(RequestSpec::Http(ref http_spec)) = specs[2].spec else {
            panic!("expected static HTTP spec");
        };
        assert_eq!(http_spec.method, HttpMethod::Delete);
    }

    #[test]
    fn setup_fails_on_non_table_return() {
        let source = r#"
            local function setup()
                return "not a table"
            end

            local function requests()
                return {}
            end

            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let err =
            call_top_level_func(&script.lua, &script.setup, store).expect_err("script should fail");

        assert_matches!(err, CallError::NonTableReturn(_))
    }

    #[test]
    fn requests_fails_on_non_table_return() {
        let source = r#"
            local function requests()
                return "not a table"
            end

            local function setup()
                return {}
            end

            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let err = call_top_level_func(&script.lua, &script.requests, store)
            .expect_err("script should fail");

        assert_matches!(err, CallError::NonTableReturn(_))
    }

    #[test]
    fn setup_fails_on_function_error() {
        let source = r#"
            local function setup()
                error("boom")
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let err = call_top_level_func(&script.lua, &script.setup, store)
            .expect_err("onSetup error should propagate");
        assert_matches!(err, CallError::Call(_));
    }

    #[test]
    fn requests_fails_on_function_error() {
        let source = r#"
            local function requests()
                error("boom")
            end
            local function setup()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let err = call_top_level_func(&script.lua, &script.requests, store)
            .expect_err("onSetup error should propagate");
        assert_matches!(err, CallError::Call(_));
    }

    #[test]
    fn setup_fails_on_string_request_spec() {
        let source = r#"
            local function setup()
                return {
                    "not a table",
                }
            end
            local function requests()
                return {}
            end
            return { setup = setup , requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let err = call_top_level_func(&script.lua, &script.setup, store)
            .expect_err("non-table entry should fail");
        assert_matches!(err, CallError::InvalidSpec{index, ..} if index == 1);
    }

    #[test]
    fn requests_fails_on_string_request_spec() {
        let source = r#"
            local function requests()
                return {
                    "not a table",
                }
            end
            local function setup()
                return {}
            end
            return { setup = setup , requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let err = call_top_level_func(&script.lua, &script.requests, store)
            .expect_err("non-table entry should fail");
        assert_matches!(err, CallError::InvalidSpec{index, ..} if index == 1);
    }

    #[test]
    fn setup_fails_on_invalid_request_spec() {
        let source = r#"
            local function setup()
                return {
                    {
                        {
                            method = "BREW",
                            service = "auth",
                            path = "/login",
                        }
                    },
                }
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let err = call_top_level_func(&script.lua, &script.setup, store)
            .expect_err("malformed spec should fail");
        assert_matches!(err, CallError::InvalidSpec{index, ..} if index == 1);
    }

    #[test]
    fn requests_fails_on_invalid_request_spec() {
        let source = r#"
            local function requests()
                return {
                    {
                        {
                            method = "BREW",
                            service = "auth",
                            path = "/login",
                        }
                    },
                }
            end
            local function setup()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = loader::load(source).expect("script should load");
        let store = Store::new();

        let err = call_top_level_func(&script.lua, &script.requests, store)
            .expect_err("malformed spec should fail");
        assert_matches!(err, CallError::InvalidSpec{index, ..} if index == 1);
    }
}
