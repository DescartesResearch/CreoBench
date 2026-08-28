use super::{func, loader};
use crate::script::USER_REQUEST_FN_NAME;

/// Errors that can occur loading a script into or driving a script with a
/// [`ScriptRunner`][`super::ScriptRunner`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The Lua module failed to load (compile or shape check).
    #[error("failed to load Lua module: {0}")]
    ScriptLoad(#[from] loader::LoadError),

    /// A top-level script function (`setup` or `requests`) call failed.
    #[error("failed to run function `{fn_name}`: {source}")]
    TopLevelFunctionCall {
        fn_name: &'static str,
        source: func::CallError,
    },

    /// The `requests` function returned an empty table.
    #[error("top-level requests function `{USER_REQUEST_FN_NAME}` returned an empty request table")]
    EmptyUserSpecs,

    /// A setup spec could not be resolved (dynamic dispatch error).
    #[error("failed to retrieve next setup request: {0}")]
    SetupSpec(#[source] Box<super::ResolveError>),

    /// A user spec could not be resolved (dynamic dispatch error).
    #[error("failed to retrieve next user request: {0}")]
    UserSpec(#[source] Box<super::ResolveError>),

    /// The HTTP extract Lua function raised an error.
    #[error("{0}")]
    HttpExtract(#[from] mlua::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
