/// Errors during Lua script spec parsing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An unexpected Lua runtime error occurred during parsing.
    #[error("unexpected Lua error: {0}")]
    Lua(#[from] mlua::Error),

    /// The `protocol` field contained an unsupported protocol string.
    #[error("invalid `protocol` field: {0}")]
    InvalidProtocol(String),

    /// A protocol-specific parsing error occurred.
    #[error("invalid http spec: {0}")]
    InvalidHTTPSpec(#[from] crate::script::parser::http::Error),

    /// A required field was missing from the Lua spec table.
    #[error("field `{0}` is required, but is missing")]
    RequiredFieldMissing(&'static str),

    /// A field expected to be a string was a different Lua type.
    #[error("field `{field}` must be a string, but got type `{type_name}`")]
    FieldNotString {
        field: &'static str,
        type_name: &'static str,
    },

    /// A string field contained invalid UTF-8.
    #[error("field `{field}` is not a valid UTF-8 string (got `{value:?}`)")]
    FieldStringValueNotUTF8 {
        field: &'static str,
        value: mlua::LuaString,
    },

    /// A field expected to be a table was a different Lua type.
    #[error("field `{field}` must be a table, but got type `{type_name}`")]
    FieldNotTable {
        field: &'static str,
        type_name: &'static str,
    },

    /// A map value (`headers`, `query`) is not string-coercible.
    /// Only strings, numbers, and booleans are accepted.
    #[error("value for key `{key}` in field `{field}` is not string-coercible (got `{value:?}`)")]
    StringMapValueNotCoercible {
        field: &'static str,
        key: String,
        value: mlua::Value,
    },

    /// The `body` table could not be converted to JSON via the mlua serde bridge.
    #[error("table in field `body` is not a valid JSON value: {0}")]
    BodyNotJson(#[source] mlua::Error),

    /// A field expected to be a function was a different Lua type.
    #[error("field `{field}` must be a function, but got type `{type_name}`")]
    FieldNotFunction {
        field: &'static str,
        type_name: &'static str,
    },

    /// A named spec entry had an invalid `spec` field type.
    #[error("invalid request spec type: expected `table` or `function`, but got type `{0}`")]
    InvalidSpecType(&'static str),

    /// A top-level spec value was neither a table nor a function.
    #[error("invalid request spec: expected `table`, but got type `{0}`")]
    InvalidRequestSpec(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;
