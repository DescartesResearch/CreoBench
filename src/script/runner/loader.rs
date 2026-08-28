use mlua::{Function, Lua, chunk::AsChunk};

use crate::script::{SETUP_FN_NAME, USER_REQUEST_FN_NAME};

/// A compiled and valid user script.
///
/// Holds a fresh Lua runtime and the two extracted top-level functions, ready
/// to be called with the per-runner [`Store`][crate::script::Store].
#[derive(Debug)]
pub struct UserScript {
    /// The Lua runtime instance for this script.
    pub lua: Lua,
    /// The `setup` function; returns a table of setup request specs.
    pub setup: Function,
    /// The `requests` function; returns a table of user request specs.
    pub requests: Function,
}

/// Load a Lua script module and extract its setup and user requests functions.
///
/// The module must return a table with two function-valued keys named
/// [`SETUP_FN_NAME`] and [`USER_REQUEST_FN_NAME`].
///
/// # Errors
///
/// Returns an error:
///   - if the source fails to compile,
///   - if it does not return a table,
///   - or if the expected functions are missing or have the wrong type.
pub fn load(source: impl AsChunk) -> Result<UserScript, LoadError> {
    let lua = Lua::new();
    let module: mlua::Value = lua.load(source).eval().map_err(LoadError::ModuleLoad)?;
    let module = match module {
        mlua::Value::Table(t) => t,
        other => return Err(LoadError::ModuleReturn(other.type_name())),
    };
    let setup: mlua::Value = module.get(SETUP_FN_NAME)?;
    let setup = match setup {
        mlua::Value::Nil => return Err(LoadError::MissingFn(SETUP_FN_NAME)),
        mlua::Value::Function(f) => f,
        other => {
            return Err(LoadError::TableValueNotFunction {
                key: SETUP_FN_NAME,
                type_name: other.type_name(),
            });
        }
    };
    let requests: mlua::Value = module.get(USER_REQUEST_FN_NAME)?;
    let requests = match requests {
        mlua::Value::Nil => return Err(LoadError::MissingFn(USER_REQUEST_FN_NAME)),
        mlua::Value::Function(f) => f,
        other => {
            return Err(LoadError::TableValueNotFunction {
                key: USER_REQUEST_FN_NAME,
                type_name: other.type_name(),
            });
        }
    };
    Ok(UserScript {
        lua,
        setup,
        requests,
    })
}

/// Errors that can occur loading a Lua script.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// An unexpected Lua runtime error occurred during loading.
    #[error("unexpected Lua error: {0}")]
    Lua(#[from] mlua::Error),

    /// The Lua source failed to compile.
    #[error("failed to compile Lua script: {0}")]
    ModuleLoad(#[source] mlua::Error),

    /// The module did not return a table.
    #[error("invalid Lua module return type: expected `table`, but got type `{0}`")]
    ModuleReturn(&'static str),

    /// A required function key was missing from the module's return table.
    #[error("missing `{0}` function in Lua module return table")]
    MissingFn(&'static str),

    /// A value in the module's return table was not a function.
    #[error("expected `{key}` to be a function, but got type `{type_name}`")]
    TableValueNotFunction {
        key: &'static str,
        type_name: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    #[test]
    fn load_valid_script() {
        let source = r#"
            local function setup()
                return {}
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let script = load(source).expect("valid script should load");

        // Smoke-check the functions: each is callable and returns a table.
        let _: mlua::Table = script.setup.call(()).expect("setup call");
        let _: mlua::Table = script.requests.call(()).expect("requests call");
    }

    #[test]
    fn load_fails_on_non_table_return() {
        let source = r#"return "not a table""#;

        let err = load(source).expect_err("non-table module should fail");
        assert_matches!(err, LoadError::ModuleReturn(_));
    }

    #[test]
    fn load_fails_on_missing_setup_fn() {
        let source = r#"
            local function requests()
                return {}
            end
            return { requests = requests }
        "#;

        let err = load(source).expect_err("missing setup function should fail");
        assert_matches!(err, LoadError::MissingFn(SETUP_FN_NAME));
    }

    #[test]
    fn load_fails_on_non_function_setup_fn() {
        let source = r#"
            local function requests()
                return {}
            end
            return { setup = 42, requests = requests }
        "#;

        let err = load(source).expect_err("non-function setup should fail");
        assert_matches!(
            err,
            LoadError::TableValueNotFunction {
                key: SETUP_FN_NAME,
                ..
            }
        );
    }

    #[test]
    fn load_fails_on_missing_requests_fn() {
        let source = r#"
            local function setup()
                return {}
            end
            return { setup = setup }
        "#;

        let err = load(source).expect_err("missing requests function should fail");
        assert_matches!(err, LoadError::MissingFn(USER_REQUEST_FN_NAME));
    }

    #[test]
    fn load_fails_on_non_function_requests_fn() {
        let source = r#"
            local function setup()
                return {}
            end
            return { setup = setup, requests = "invalid" }
        "#;

        let err = load(source).expect_err("non-function requests should fail");
        assert_matches!(
            err,
            LoadError::TableValueNotFunction {
                key: USER_REQUEST_FN_NAME,
                ..
            }
        );
    }
}
