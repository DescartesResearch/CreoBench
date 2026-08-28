use std::collections::HashMap;
use std::sync::Arc;

use mlua::{Function, Lua, chunk::AsChunk};

mod error;
mod func;
mod loader;

use crate::transaction::SpecId;

use self::func::DynamicResolve;

use super::response::Response;
use super::spec::{HTTPStaticRequestSpec, SpecType};
use super::{LuaRequestSpec, RequestSpec, SETUP_FN_NAME, Store, USER_REQUEST_FN_NAME};
pub use error::{Error, Result};

/// Per ScriptRunner lifecycle object for running a Lua script.
///
/// [`ScriptRunner::setup`] loads the script, calls the setup and user requests
/// functions, and caches the parsed specs. The runner then allows to drive the two
/// request phases through [`Self::next_setup_spec`] / [`Self::next_user_spec`] and
/// [`Self::run_http_extract`].
///
/// Setup specs iterate in declaration order and return `None` once exhausted.
/// The user loop requests cycles and wraps around. Named specs enable
/// dynamic dispatch via the jump protocol.
#[derive(Debug)]
pub struct ScriptRunner {
    /// The Lua runtime.
    lua: Lua,
    /// Per-ScriptRunner key-value store shared with Lua scripts.
    store: Store,
    /// Parsed specs from the `setup` function.
    setup_specs: Vec<LuaRequestSpec>,
    /// Index of named setup specs for dynamic dispatch.
    setup_specs_by_name: HashMap<Arc<str>, usize>,
    /// Current position in the setup spec iteration (one-pass).
    setup_cursor: usize,
    /// Parsed specs from the `requests` function.
    user_specs: Vec<LuaRequestSpec>,
    /// Index of named user specs for dynamic dispatch.
    user_specs_by_name: HashMap<Arc<str>, usize>,
    /// Current position in the user spec cycle (wraps around).
    user_cursor: usize,
}

impl ScriptRunner {
    /// Load a Lua script and initialise the runner for both request phases.
    ///
    /// Calls the script's `setup` and `requests` functions, parses their
    /// return tables into cached spec lists. The returned runner is ready
    /// for the setup phase via [`Self::next_setup_spec`] followed by the
    /// user request cycle via [`Self::next_user_spec`].
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the Lua source fails to compile or does not return the expected shape,
    /// - either top-level function call fails at runtime,
    /// - the `requests` function returns an empty table,
    /// - or any spec entry is malformed.
    pub fn setup(source: impl AsChunk) -> Result<Self> {
        let script = loader::load(source)?;

        let store = Store::new();

        let setup_specs = func::call_top_level_func(&script.lua, &script.setup, store.clone())
            .map_err(|e| Error::TopLevelFunctionCall {
                fn_name: SETUP_FN_NAME,
                source: e,
            })?;

        let setup_specs_by_name: HashMap<Arc<str>, usize> = setup_specs
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((s.name.clone()?, i)))
            .collect();

        let user_specs = func::call_top_level_func(&script.lua, &script.requests, store.clone())
            .map_err(|e| Error::TopLevelFunctionCall {
                fn_name: USER_REQUEST_FN_NAME,
                source: e,
            })?;
        if user_specs.is_empty() {
            return Err(Error::EmptyUserSpecs);
        }
        let user_specs_by_name: HashMap<Arc<str>, usize> = user_specs
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((s.name.clone()?, i)))
            .collect();

        Ok(ScriptRunner {
            lua: script.lua,
            store,
            setup_specs,
            setup_specs_by_name,
            setup_cursor: 0,
            user_specs,
            user_specs_by_name,
            user_cursor: 0,
        })
    }

    /// Advance to the next spec in the setup phase.
    ///
    /// Setup specs are returned in declaration order. Returns `None` once
    /// all setup specs have been yielded. Drive the setup phase to
    /// completion before entering the user request cycle.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SetupSpec`] if a dynamic spec resolution fails.
    pub fn next_setup_spec(&mut self) -> Result<Option<(SpecId, RequestSpec)>> {
        if self.setup_specs.is_empty() || self.setup_cursor >= self.setup_specs.len() {
            return Ok(None);
        }
        let result = next_spec(
            &self.setup_specs,
            &self.setup_specs_by_name,
            &mut self.setup_cursor,
            &self.lua,
            self.store.clone(),
        )
        .map_err(Error::SetupSpec)?;
        Ok(Some(result))
    }

    /// Advance to the next spec in the user request cycle.
    ///
    /// User specs form a circular buffer: the cursor wraps around
    /// after the last spec, producing an infinite sequence. Dynamic
    /// dispatch (jumps) may redirect to a named spec elsewhere in
    /// the cycle.
    ///
    /// On success, returns a `RevertHandle` capturing the cursor's
    /// pre-call position alongside the produced spec. Drop the handle
    /// to commit the cursor advance, or pass it to
    /// `RevertHandle::revert` to roll the cursor back to its
    /// pre-call position. On [`Err`], no handle is produced and the
    /// cursor is left unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UserSpec`] if a dynamic spec resolution fails.
    pub fn next_user_spec(&mut self) -> Result<(RevertHandle, SpecId, RequestSpec)> {
        let previous_cursor = self.user_cursor;
        let result = next_spec(
            &self.user_specs,
            &self.user_specs_by_name,
            &mut self.user_cursor,
            &self.lua,
            self.store.clone(),
        )
        .map_err(Error::UserSpec)?;
        self.user_cursor %= self.user_specs.len();
        Ok((RevertHandle { previous_cursor }, result.0, result.1))
    }

    /// Run the extract function on a completed HTTP response.
    ///
    /// If the spec has an `extract` function, this passes the [`Store`] and
    /// [`Response`] to it so the script can persist data from the response.
    /// No-op when `spec.extract` is `None`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::HttpExtract`] if the Lua extract function raises an error.
    pub fn run_http_extract(&self, spec: &HTTPStaticRequestSpec, response: Response) -> Result<()> {
        let extract = match &spec.extract {
            Some(f) => f,
            None => return Ok(()),
        };
        Ok(extract.call((self.store.clone(), response))?)
    }

    /// Access the store.
    pub fn store(&self) -> &Store {
        &self.store
    }
}

/// A handle returned from [`ScriptRunner::next_user_spec`] that lets the
/// caller roll the runner's user-cursor back to its pre-call position.
///
/// Drop the handle to commit the cursor advance. Pass the handle to
/// [`Self::revert`] to roll the cursor back. Dropping without calling
/// `revert` is the success path.
#[derive(Debug)]
pub struct RevertHandle {
    previous_cursor: usize,
}

impl RevertHandle {
    /// Roll the runner's user-cursor back to the position it had before
    /// the matching [`ScriptRunner::next_user_spec`] call.
    ///
    /// Consumes the handle — it can only be reverted once. After this
    /// call, the next [`ScriptRunner::next_user_spec`] will produce the
    /// same spec as before.
    pub fn revert(self, runner: &mut ScriptRunner) {
        runner.user_cursor = self.previous_cursor;
    }
}

/// Errors during dynamic-dispatch resolution.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// A dynamic spec function call failed.
    #[error("failed to resolve dynamic request spec at index `{index}` (1-based): {source}")]
    Dynamic {
        index: usize,
        source: func::CallError,
    },

    /// A dynamic function returned a jump to a name that does not exist in the spec list.
    #[error(
        "unknown jump in dynamic request spec at index `{index}` (1-based): `{jump}` is not a valid jump"
    )]
    UnknownJump { index: usize, jump: String },
}

fn next_spec(
    specs: &[LuaRequestSpec],
    specs_by_name: &HashMap<Arc<str>, usize>,
    cursor: &mut usize,
    lua: &Lua,
    store: Store,
) -> std::result::Result<(SpecId, RequestSpec), Box<ResolveError>> {
    let request = &specs[*cursor];
    let spec = match &request.spec {
        SpecType::Static(spec) => spec.clone(),
        SpecType::Dynamic(f) => {
            let (index, spec) = resolve(*cursor, f, specs, specs_by_name, lua, store)?;
            *cursor = index;
            spec
        }
    };
    let index = *cursor;
    *cursor += 1;
    Ok((SpecId::new(index), spec))
}

fn resolve(
    cursor: usize,
    f: &Function,
    specs: &[LuaRequestSpec],
    specs_by_name: &HashMap<Arc<str>, usize>,
    lua: &Lua,
    store: Store,
) -> std::result::Result<(usize, RequestSpec), Box<ResolveError>> {
    let result = func::call_dynamic_func(cursor, f, lua, store.clone()).map_err(|e| {
        ResolveError::Dynamic {
            index: cursor + 1,
            source: e,
        }
    })?;
    match result {
        DynamicResolve::Spec(spec) => Ok((cursor, spec)),
        DynamicResolve::Jump(jump) => {
            let index = specs_by_name
                .get(jump.as_str())
                .ok_or(ResolveError::UnknownJump {
                    index: cursor + 1,
                    jump,
                })?;
            let jump_spec = &specs[*index];
            match &jump_spec.spec {
                SpecType::Static(spec) => Ok((*index, spec.clone())),
                SpecType::Dynamic(jump_func) => {
                    resolve(*index, jump_func, specs, specs_by_name, lua, store)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use mlua::IntoLua;

    use super::*;
    use crate::script::parser::http::HttpMethod;
    use std::assert_matches;

    #[test]
    fn setup_loads_valid_script() {
        let source = r#"
            local function setup()
                return {}
            end
            local function requests()
                return {
                    {
                        protocol = "http",
                        method  = "POST",
                        service = "service-1",
                        path    = "/create",
                        body    = { title = "Hello" },
                    },
                }
            end
            return { setup = setup, requests = requests }
        "#;

        assert!(ScriptRunner::setup(source).is_ok());
    }

    #[test]
    fn setup_fails_on_top_level_setup_error() {
        let source = r#"
            local function setup()
                return error("fail")
            end
            local function requests()
                return {
                    {
                        protocol = "http",
                        method  = "POST",
                        service = "service-1",
                        path    = "/create",
                        body    = { title = "Hello" },
                    },
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let err = ScriptRunner::setup(source).unwrap_err();
        assert_matches!(
            err,
            Error::TopLevelFunctionCall {
                fn_name: SETUP_FN_NAME,
                ..
            }
        );
    }

    #[test]
    fn setup_fails_on_top_level_requests_error() {
        let source = r#"
            local function setup()
                return {}
            end
            local function requests()
                return error("fail")
            end
            return { setup = setup, requests = requests }
        "#;

        let err = ScriptRunner::setup(source).unwrap_err();
        assert_matches!(
            err,
            Error::TopLevelFunctionCall {
                fn_name: USER_REQUEST_FN_NAME,
                ..
            }
        );
    }

    #[test]
    fn setup_fails_on_empty_user_requests_table() {
        let source = r#"
            local function setup()
                return {}
            end
            local function requests()
                return {}
            end
            return { setup = setup, requests = requests }
        "#;

        let err = ScriptRunner::setup(source).unwrap_err();
        assert_matches!(err, Error::EmptyUserSpecs);
    }

    #[test]
    fn next_setup_spec_returns_none_on_empty_table() {
        let source = r#"
            local function setup()
                return {}
            end
            local function requests()
                return {
                    {
                        protocol = "http",
                        method  = "POST",
                        service = "service-1",
                        path    = "/create",
                        body    = { title = "Hello" },
                    },
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        assert!(runner.setup_specs.is_empty());
        assert!(runner.next_setup_spec().unwrap().is_none());
    }

    #[test]
    fn script_runner_can_run_minimal_script() {
        let source = r#"
            local function setup()
                return {}
            end
            local function requests()
                return {
                    {
                        protocol = "http",
                        method  = "POST",
                        service = "service-1",
                        path    = "/create",
                        body    = { title = "Hello" },
                    },
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        assert!(runner.setup_specs.is_empty());
        assert!(runner.next_setup_spec().unwrap().is_none());

        assert_eq!(runner.user_specs.len(), 1);
        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.method, HttpMethod::Post);
        assert_eq!(http_spec.service, "service-1");
        assert_eq!(http_spec.path, "/create");
        assert_eq!(http_spec.body, Some(serde_json::json!({"title": "Hello"})));
    }

    #[test]
    fn script_runner_can_run_script_with_static_setup_requests() {
        let source = r#"
            local function setup()
                return {
                    {
                        protocol = "http",
                        method = "POST",
                        service = "auth",
                        path = "/login",
                        body = { user = "user-1", password = "password123" },
                    }
                }
            end
            local function requests()
                return {
                    {
                        protocol = "http",
                        method  = "DELETE",
                        service = "post",
                        path    = "/posts",
                        body    = { id = 123456 },
                    },
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        assert_eq!(runner.setup_specs.len(), 1);
        let (_, setup_spec) = runner.next_setup_spec().unwrap().unwrap();
        let RequestSpec::Http(setup_http_spec) = setup_spec;
        assert_eq!(setup_http_spec.method, HttpMethod::Post);
        assert_eq!(setup_http_spec.service, "auth");
        assert_eq!(setup_http_spec.path, "/login");
        assert_eq!(
            setup_http_spec.body,
            Some(serde_json::json!({"user": "user-1", "password": "password123"}))
        );

        assert_eq!(runner.user_specs.len(), 1);
        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.method, HttpMethod::Delete);
        assert_eq!(http_spec.service, "post");
        assert_eq!(http_spec.path, "/posts");
        assert_eq!(http_spec.body, Some(serde_json::json!({"id": 123456})));
    }

    #[test]
    fn script_runner_can_run_script_with_dynamic_requests() {
        let source = r#"
            local function setup()
                return {
                    function()
                        return {
                            protocol = "http",
                            method = "POST",
                            service = "user",
                            path = "/users",
                            body = { user = "user-1", password = "password123" },
                        }
                    end,
                    {
                        protocol = "http",
                        method = "POST",
                        service = "auth",
                        path = "/login",
                        body = { user = "user-1", password = "password123" },
                    }
                }
            end
            local function requests()
                return {
                    {
                        protocol = "http",
                        method  = "DELETE",
                        service = "post",
                        path    = "/posts",
                        body    = { id = 123456 },
                    },
                    function()
                        return {
                            protocol = "http",
                            method = "DELETE",
                            service = "user",
                            path = "/users",
                            body = { user = "user-1" },
                        }
                    end,
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        for expected_spec in [
            (
                HttpMethod::Post,
                "user",
                "/users",
                serde_json::json!({"user": "user-1", "password": "password123"}),
            ),
            (
                HttpMethod::Post,
                "auth",
                "/login",
                serde_json::json!({"user": "user-1", "password": "password123"}),
            ),
        ] {
            let (_, spec) = runner.next_setup_spec().unwrap().unwrap();
            let RequestSpec::Http(http_spec) = spec;
            assert_eq!(http_spec.method, expected_spec.0);
            assert_eq!(http_spec.service, expected_spec.1);
            assert_eq!(http_spec.path, expected_spec.2);
            assert_eq!(http_spec.body, Some(expected_spec.3));
        }

        assert!(runner.next_setup_spec().unwrap().is_none());

        for expected_spec in [
            (
                HttpMethod::Delete,
                "post",
                "/posts",
                serde_json::json!({"id": 123456}),
            ),
            (
                HttpMethod::Delete,
                "user",
                "/users",
                serde_json::json!({"user": "user-1" }),
            ),
        ] {
            let (_, _, spec) = runner.next_user_spec().unwrap();
            let RequestSpec::Http(http_spec) = spec;
            assert_eq!(http_spec.method, expected_spec.0);
            assert_eq!(http_spec.service, expected_spec.1);
            assert_eq!(http_spec.path, expected_spec.2);
            assert_eq!(http_spec.body, Some(expected_spec.3));
        }
    }

    #[test]
    fn next_user_request_wraps() {
        let source = r#"
            local function setup()
                return { }
            end
            local function requests()
                return {
                    {
                        protocol = "http",
                        method = "POST",
                        service = "auth",
                        path = "/login",
                        body = { user = "user-1", password = "password123" },
                    },
                    {
                        protocol = "http",
                        method  = "DELETE",
                        service = "post",
                        path    = "/posts",
                        body    = { id = 123456 },
                    },
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        assert_eq!(runner.setup_specs.len(), 0);
        assert!(runner.next_setup_spec().unwrap().is_none());

        let expected_specs = [
            (
                HttpMethod::Post,
                "auth",
                "/login",
                Some(serde_json::json!({"user": "user-1", "password": "password123"})),
            ),
            (
                HttpMethod::Delete,
                "post",
                "/posts",
                Some(serde_json::json!({"id": 123456})),
            ),
        ];

        for i in 0..4 {
            let expected_spec = &expected_specs[i % expected_specs.len()];
            let (_, _, spec) = runner.next_user_spec().unwrap();
            let RequestSpec::Http(http_spec) = spec;
            assert_eq!(http_spec.method, expected_spec.0);
            assert_eq!(http_spec.service, expected_spec.1);
            assert_eq!(http_spec.path, expected_spec.2);
            assert_eq!(http_spec.body, expected_spec.3);
        }
    }

    #[test]
    fn script_runner_can_jump_to_static_requests() {
        let source = r#"
            setupCalled = false
            local function setup()
                return {
                    {
                        name = "createUser",
                        spec = {
                            protocol = "http",
                            method = "POST",
                            service = "user",
                            path = "/users",
                            body = { user = "user-1", password = "password123" },
                        }
                    },
                    function()
                        if setupCalled then
                            return {
                                protocol = "http",
                                method = "POST",
                                service = "auth",
                                path = "/login",
                                body = { user = "user-1", password = "password123" },
                            }
                        end
                        setupCalled = true
                        return "createUser"
                    end,
                }
            end

            requestsCalled = false
            local function requests()
                return {
                    {
                        name = "deletePost",
                        spec = {
                            protocol = "http",
                            method  = "DELETE",
                            service = "post",
                            path    = "/posts",
                            body    = { id = 123456 },
                        }
                    },
                    function()
                        if requestsCalled then
                            return {
                                protocol = "http",
                                method = "DELETE",
                                service = "user",
                                path = "/users",
                                body = { user = "user-1" },
                            }
                        end
                        requestsCalled = true
                        return "deletePost"
                    end,
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        for expected_spec in [
            (
                HttpMethod::Post,
                "user",
                "/users",
                serde_json::json!({"user": "user-1", "password": "password123"}),
            ),
            (
                HttpMethod::Post,
                "user",
                "/users",
                serde_json::json!({"user": "user-1", "password": "password123"}),
            ),
            (
                HttpMethod::Post,
                "auth",
                "/login",
                serde_json::json!({"user": "user-1", "password": "password123"}),
            ),
        ] {
            let (_, spec) = runner.next_setup_spec().unwrap().unwrap();
            let RequestSpec::Http(http_spec) = spec;
            assert_eq!(http_spec.method, expected_spec.0);
            assert_eq!(http_spec.service, expected_spec.1);
            assert_eq!(http_spec.path, expected_spec.2);
            assert_eq!(http_spec.body, Some(expected_spec.3));
        }

        assert!(runner.next_setup_spec().unwrap().is_none());

        for expected_spec in [
            (
                HttpMethod::Delete,
                "post",
                "/posts",
                serde_json::json!({"id": 123456}),
            ),
            (
                HttpMethod::Delete,
                "post",
                "/posts",
                serde_json::json!({"id": 123456}),
            ),
            (
                HttpMethod::Delete,
                "user",
                "/users",
                serde_json::json!({"user": "user-1" }),
            ),
        ] {
            let (_, _, spec) = runner.next_user_spec().unwrap();
            let RequestSpec::Http(http_spec) = spec;
            assert_eq!(http_spec.method, expected_spec.0);
            assert_eq!(http_spec.service, expected_spec.1);
            assert_eq!(http_spec.path, expected_spec.2);
            assert_eq!(http_spec.body, Some(expected_spec.3));
        }
    }

    #[test]
    fn script_runner_can_jump_to_static_requests_multi() {
        let source = r#"
            isUserCreated = true
            isLoggedIn = true
            local function setup()
                return {
                    {
                        name = "createUser",
                        spec = function()
                            if isUserCreated then
                                return "login"
                            end
                            return {
                                protocol = "http",
                                method = "POST",
                                service = "user",
                                path = "/users",
                                body = { user = "user-1", password = "password123" },
                            }
                        end
                    },
                    {
                        name = "login",
                        spec = function()
                            if isLoggedIn then
                                return "createPost"
                            end
                            return {
                                protocol = "http",
                                method = "POST",
                                service = "auth",
                                path = "/login",
                                body = { user = "user-1", password = "password123" },
                            }
                        end,
                    },
                    {
                        name = "createPost",
                        spec = {
                            protocol = "http",
                            method = "POST",
                            service = "post",
                            path = "/posts",
                            body = { title = "My first Post", content = "Hello, World!" },
                        }
                    }
                }
            end

            isPostDeleted = true
            isUserDeleted = true
            local function requests()
                return {
                    {
                        name = "deletePost",
                        spec = function()
                            if isPostDeleted then
                                return "deleteUser"
                            end
                            return {
                                protocol = "http",
                                method  = "DELETE",
                                service = "post",
                                path    = "/posts",
                                body    = { id = 123456 },
                            }
                        end
                    },
                    {
                        name = "deleteUser",
                        spec = function()
                            if isUserDeleted then
                                return "createPost"
                            end
                            return {
                                protocol = "http",
                                method = "DELETE",
                                service = "user",
                                path = "/users",
                                body = { user = "user-1" },
                            }
                        end,
                    },
                    {
                        name = "createPost",
                        spec = {
                            protocol = "http",
                            method = "POST",
                            service = "post",
                            path = "/posts",
                            body = { title = "New Post", content = "Goodbye, World!" },
                        }
                    }
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        let (_, spec) = runner.next_setup_spec().unwrap().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.method, HttpMethod::Post);
        assert_eq!(http_spec.service, "post");
        assert_eq!(http_spec.path, "/posts");
        assert_eq!(
            http_spec.body,
            Some(serde_json::json!({"title": "My first Post", "content": "Hello, World!"}))
        );

        assert!(runner.next_setup_spec().unwrap().is_none());

        for _ in 0..2 {
            let (_, _, spec) = runner.next_user_spec().unwrap();
            let RequestSpec::Http(http_spec) = spec;
            assert_eq!(http_spec.method, HttpMethod::Post);
            assert_eq!(http_spec.service, "post");
            assert_eq!(http_spec.path, "/posts");
            assert_eq!(
                http_spec.body,
                Some(serde_json::json!({"title": "New Post", "content": "Goodbye, World!"}))
            );
        }
    }

    #[test]
    fn resolve_errors_on_invalid_jump() {
        let source = r#"
            isUserCreated = true
            local function setup()
                return {
                    {
                        name = "createUser",
                        spec = function()
                            if isUserCreated then
                                return "login"
                            end
                            return {
                                protocol = "http",
                                method = "POST",
                                service = "user",
                                path = "/users",
                                body = { user = "user-1", password = "password123" },
                            }
                        end
                    },
                    {
                        protocol = "http",
                        method = "POST",
                        service = "auth",
                        path = "/login",
                        body = { user = "user-1", password = "password123" },
                    },
                }
            end

            isPostDeleted = true
            local function requests()
                return {
                    {
                        name = "deletePost",
                        spec = function()
                            if isPostDeleted then
                                return "deleteUser"
                            end
                            return {
                                protocol = "http",
                                method  = "DELETE",
                                service = "post",
                                path    = "/posts",
                                body    = { id = 123456 },
                            }
                        end
                    },
                    {
                        protocol = "http",
                        method = "DELETE",
                        service = "user",
                        path = "/users",
                        body = { user = "user-1" },
                    }
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        let err = runner.next_setup_spec().unwrap_err();

        assert_matches!(err, Error::SetupSpec(inner) if matches!(inner.as_ref(), ResolveError::UnknownJump { index, jump } if *index == 1 && jump == "login"));

        let err = runner.next_user_spec().unwrap_err();
        assert_matches!(err, Error::UserSpec(inner) if matches!(inner.as_ref(), ResolveError::UnknownJump { index, jump } if *index == 1 && jump == "deleteUser"));
    }

    #[test]
    fn can_use_store_in_top_level_functions() {
        let source = r#"
            local function setup(store)
                store:set("myInt", 3)
                assert(store:get("myInt") == 3)
                assert(store:get("unset") == nil)
                return { }
            end

            local function requests(store)
                assert(store:get("myInt") == 3)
                store:set("myBool", true)
                assert(store:get("myBool"))
                return {
                    {
                        protocol = "http",
                        method = "DELETE",
                        service = "user",
                        path = "/users",
                        body = { user = "user-1" },
                    }
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let runner = ScriptRunner::setup(source).unwrap();

        assert_eq!(runner.store.get("myInt").unwrap().as_usize().unwrap(), 3);
        assert!(runner.store.get("unset").is_none());
        assert!(runner.store.get("myBool").unwrap().as_boolean().unwrap());
    }

    #[test]
    fn can_use_store_in_dynamic_functions() {
        let source = r#"
            local function setup(store)
                store:set("myInt", 3)
                assert(store:get("myInt") == 3)
                assert(store:get("unset") == nil)
                return { 
                    function(store)
                        assert(store:get("myInt") == 3)
                        store:set("myInt", 5)
                        assert(store:get("myInt") == 5)
                        return {
                                protocol = "http",
                                method = "DELETE",
                                service = "user",
                                path = "/users",
                                body = { user = "user-1" },
                        }
                    end,
                }
            end

            local function requests(store)
                assert(store:get("myInt") == 3)
                store:set("myBool", true)
                assert(store:get("myBool"))
                return {
                    function(store)
                        assert(store:get("myInt") == 5)
                        store:set("myBool", false)
                        assert(store:get("myBool") == false)
                        return {
                                protocol = "http",
                                method = "DELETE",
                                service = "user",
                                path = "/users",
                                body = { user = "user-1" },
                        }
                    end,
                }
            end
            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        assert_eq!(runner.store.get("myInt").unwrap().as_usize().unwrap(), 3);
        assert!(runner.store.get("unset").is_none());
        assert!(runner.store.get("myBool").unwrap().as_boolean().unwrap());
        let _ = runner.next_setup_spec().unwrap().unwrap();
        assert_eq!(runner.store.get("myInt").unwrap().as_usize().unwrap(), 5);

        runner.next_user_spec().unwrap();
        assert_eq!(runner.store.get("myInt").unwrap().as_usize().unwrap(), 5);
        assert!(!runner.store.get("myBool").unwrap().as_boolean().unwrap());
    }

    #[test]
    fn run_http_extract_succeeds_and_can_use_store() {
        let source = r#"
            local function setup()
                return {}
            end

            local function requests()
                return {
                    {
                        protocol = "http",
                        method = "DELETE",
                        service = "user",
                        path = "/users",
                        body = { user = "user-1" },
                        extract = function(store, response)
                            local data, err = response:json()
                            if err then
                                error("expected valid json: " .. tostring(err))
                            end
                            store:set("token", data.token)
                            store:set("status", response:status())
                        end,
                    }
                }
            end
            return { setup = setup, requests = requests }
        "#;
        let mut runner = ScriptRunner::setup(source).unwrap();
        let (_, _, spec) = runner.next_user_spec().unwrap();

        let RequestSpec::Http(http_spec) = spec;

        let response = Response::new(
            200,
            Vec::new(),
            Some(br#"{"token": "abc-123"}"#.as_slice().into()),
        );

        runner.run_http_extract(&http_spec, response).unwrap();

        assert_eq!(
            runner.store.get("token").unwrap().as_string().unwrap(),
            "abc-123"
        );
        assert_eq!(runner.store.get("status").unwrap().as_usize().unwrap(), 200);
    }

    #[test]
    fn run_http_extract_succeeds_on_missing_extract_function() {
        let source = r#"
            local function setup()
                return {}
            end

            local function requests()
                return {
                    {
                        protocol = "http",
                        method = "DELETE",
                        service = "user",
                        path = "/users",
                        body = { user = "user-1" },
                    }
                }
            end
            return { setup = setup, requests = requests }
        "#;
        let mut runner = ScriptRunner::setup(source).unwrap();
        let (_, _, spec) = runner.next_user_spec().unwrap();

        let RequestSpec::Http(http_spec) = spec;

        let response = Response::new(200, Vec::new(), None);

        runner.run_http_extract(&http_spec, response).unwrap();
    }

    #[test]
    fn run_http_extract_fails_on_extract_error() {
        let source = r#"
            local function setup()
                return {}
            end

            local function requests()
                return {
                    {
                        protocol = "http",
                        method = "DELETE",
                        service = "user",
                        path = "/users",
                        body = { user = "user-1" },
                        extract = function(store, response)
                            local data, err = response:json()
                            if err then
                                error("expected valid json: " .. tostring(err))
                            end
                            store:set("token", data.token)
                            store:set("status", response:status())
                        end,
                    }
                }
            end
            return { setup = setup, requests = requests }
        "#;
        let mut runner = ScriptRunner::setup(source).unwrap();
        let (_, _, spec) = runner.next_user_spec().unwrap();

        let RequestSpec::Http(http_spec) = spec;

        let response = Response::new(
            200,
            Vec::new(),
            Some(br#"{"token": "abc-123""#.as_slice().into()),
        );

        let err = runner.run_http_extract(&http_spec, response).unwrap_err();

        assert_matches!(err, Error::HttpExtract(_))
    }

    #[test]
    fn script_runner_succeeds_end_to_end() {
        let source = r#"
            local function setup()
                return {
                    {
                        protocol = "http",
                        method  = "POST",
                        service = "auth",
                        path    = "/login",
                        body    = { username = "demo", password = "secret" },
                        extract = function(store, response)
                            local data, err = response:json()
                            if not data then
                                error("login: expected JSON: " .. tostring(err))
                            end
                            store:set("access_token", data.accessToken)
                            store:set("refresh_token", data.refreshToken)
                        end,
                    },
                }
            end

            local function requests()
                return {
                    { 
                        name = "createPost",
                        spec = {
                            protocol = "http",
                            method = "POST",
                            service = "service-1",
                            path = "/create",
                            body = { title = "Hello", content = "World" } 
                        },
                    },
                    function(store)
                        if store:get("shouldJump") then
                            return "createPost"
                        else
                            return {
                                protocol = "http",
                                method  = "GET",
                                service = "service-2",
                                path    = "/protected",
                                query   = { n = 3 },
                            }
                        end
                    end,
                }
            end

            return { setup = setup, requests = requests }
        "#;

        let mut runner = ScriptRunner::setup(source).unwrap();

        assert_eq!(runner.setup_specs.len(), 1);
        let (_, setup_spec) = runner.next_setup_spec().unwrap().unwrap();
        let RequestSpec::Http(setup_http_spec) = setup_spec;
        assert_eq!(setup_http_spec.method, HttpMethod::Post);
        assert_eq!(setup_http_spec.service, "auth");
        assert_eq!(setup_http_spec.path, "/login");
        assert!(setup_http_spec.body.is_some());
        assert!(setup_http_spec.extract.is_some());

        let login_response = Response::new(
            200,
            vec![("Content-Type".to_string(), "application/json".to_string())],
            Some(
                br#"{"accessToken": "abc-123", "refreshToken": "def-456"}"#
                    .as_slice()
                    .into(),
            ),
        );
        runner
            .run_http_extract(&setup_http_spec, login_response)
            .unwrap();

        assert_eq!(
            runner
                .store
                .get("access_token")
                .unwrap()
                .as_string()
                .unwrap(),
            "abc-123"
        );
        assert_eq!(
            runner
                .store
                .get("refresh_token")
                .unwrap()
                .as_string()
                .unwrap(),
            "def-456"
        );

        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.method, HttpMethod::Post);
        assert_eq!(http_spec.service, "service-1");
        assert_eq!(http_spec.path, "/create");
        assert_eq!(
            http_spec.body,
            Some(serde_json::json!({"title": "Hello", "content": "World"}))
        );

        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.method, HttpMethod::Get);
        assert_eq!(http_spec.service, "service-2");
        assert_eq!(http_spec.path, "/protected");
        assert_eq!(http_spec.query, vec![("n".to_string(), "3".to_string())]);

        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.method, HttpMethod::Post);
        assert_eq!(http_spec.service, "service-1");
        assert_eq!(http_spec.path, "/create");
        assert_eq!(
            http_spec.body,
            Some(serde_json::json!({"title": "Hello", "content": "World"}))
        );

        runner
            .store
            .set("shouldJump", true.into_lua(&runner.lua).unwrap());
        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.method, HttpMethod::Post);
        assert_eq!(http_spec.path, "/create");
        assert_eq!(
            http_spec.body,
            Some(serde_json::json!({"title": "Hello", "content": "World"}))
        );

        assert!(
            runner
                .store
                .get("shouldJump")
                .unwrap()
                .as_boolean()
                .unwrap()
        );
    }

    #[test]
    fn revert_handle_drop_commits_cursor_advance() {
        let mut runner = ScriptRunner::setup(TWO_GET_SPECS_SCRIPT).unwrap();

        let (_handle, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.path, "/first");

        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.path, "/second");
    }

    #[test]
    fn revert_handle_revert_rolls_back_to_pre_call_position() {
        let mut runner = ScriptRunner::setup(TWO_GET_SPECS_SCRIPT).unwrap();

        let (handle, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.path, "/first");

        handle.revert(&mut runner);

        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.path, "/first");
    }

    #[test]
    fn revert_handle_preserves_last_index_at_wraparound() {
        let mut runner = ScriptRunner::setup(TWO_GET_SPECS_SCRIPT).unwrap();

        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.path, "/first");

        let (handle, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(http_spec.path, "/second");

        handle.revert(&mut runner);

        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(
            http_spec.path, "/second",
            "revert at the wraparound boundary must restore the last index, not panic on overflow"
        );
    }

    #[test]
    fn revert_handle_restores_pre_jump_cursor_on_dynamic_dispatch() {
        let mut runner = ScriptRunner::setup(JUMP_TO_NAMED_TARGET_SCRIPT).unwrap();

        let (handle, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(
            http_spec.path, "/target",
            "dynamic spec at index 0 should jump to 'target' at index 2"
        );

        handle.revert(&mut runner);

        let (_, _, spec) = runner.next_user_spec().unwrap();
        let RequestSpec::Http(http_spec) = spec;
        assert_eq!(
            http_spec.path, "/target",
            "revert must restore the pre-jump cursor (index 0), not the index before the jump target"
        );
    }

    const TWO_GET_SPECS_SCRIPT: &str = r#"
        local function setup()
            return {}
        end
        local function requests()
            return {
                { protocol = "http", method = "GET", service = "api", path = "/first" },
                { protocol = "http", method = "GET", service = "api", path = "/second" },
            }
        end
        return { setup = setup, requests = requests }
    "#;

    const JUMP_TO_NAMED_TARGET_SCRIPT: &str = r#"
        local function setup()
            return {}
        end
        local function requests()
            return {
                function()
                    return "target"
                end,
                { protocol = "http", method = "GET", service = "api", path = "/before-target" },
                {
                    name = "target",
                    spec = {
                        protocol = "http",
                        method = "GET",
                        service = "api",
                        path = "/target",
                    }
                },
            }
        end
        return { setup = setup, requests = requests }
    "#;
}
