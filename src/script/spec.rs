use std::sync::Arc;

use mlua::Function;

use crate::script::parser::http::HttpMethod;

/// A single request spec returned by a script.
///
/// A spec is either _static_ (parsed at load time) or _dynamic_ (a Lua
/// function that produces the spec at runtime). Named specs support dynamic
/// dispatch via the name/jump protocol.
#[derive(Debug, Clone)]
pub struct LuaRequestSpec {
    /// Name for dynamic-dispatch jumps. `None` for anonymous specs.
    pub name: Option<Arc<str>>,
    /// Either a static `SpecType::Static` parsed at load time, or a
    /// `SpecType::Dynamic` Lua function called per request.
    pub spec: SpecType,
}

/// Whether a request spec is static or dynamic.
#[derive(Debug, Clone)]
pub enum SpecType {
    /// A spec table parsed at script load time.
    Static(RequestSpec),
    /// A Lua function called once per request, receiving the [`Store`].
    Dynamic(Function),
}

/// A protocol-specific request specification.
///
/// This is the validated form produced by the parser.
#[derive(Debug, Clone)]
pub enum RequestSpec {
    /// An HTTP request, carrying the fully-parsed [`HTTPStaticRequestSpec`].
    Http(HTTPStaticRequestSpec),
}

/// A fully-parsed, static HTTP request spec from a Lua script.
#[derive(Debug, Clone)]
pub struct HTTPStaticRequestSpec {
    /// The HTTP method for the request.
    pub method: HttpMethod,
    /// The logical service name (resolved by the transport layer).
    pub service: String,
    /// The URL path.
    pub path: String,
    /// Static header key-value pairs.
    pub headers: Vec<(String, String)>,
    /// Static query parameter key-value pairs.
    pub query: Vec<(String, String)>,
    /// A static JSON body, or `None` for no body.
    pub body: Option<serde_json::Value>,
    /// An optional Lua function to extract data from the response.
    pub(super) extract: Option<Function>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_request_spec_is_send() {
        // Compile-time check: `LuaRequestSpec: Send` is required for the
        // runtime to ship the value across thread boundaries. The function
        // body is what matters — if the assertion is wrong, this file
        // won't compile.
        fn assert_send<T: Send>() {}
        assert_send::<LuaRequestSpec>();
    }
}
