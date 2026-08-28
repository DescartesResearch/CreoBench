//! Parsing and execution of user-provided Lua scripts.
//!
//! A script is a Lua module returning two top-level functions, `setup` and
//! `requests`. Both functions return an array of request specs — each entry
//! is either static (parsed once at load time) or dynamic (computed at
//! request time). Dynamic request specs let a script conditionally return
//! a static request spec or jump to a different named request in the loop,
//! enabling the next request to depend on prior responses or shared state.
//!
//! - `parser` parses and validates Lua tables into typed request specs.
//! - `runner` drives the setup phase and the user-request loop.
//! - `store` is the per-runner key-value store for sharing state across requests.
//! - `response` is the HTTP response handle exposed to Lua extract callbacks.

mod parser;
mod response;
mod runner;
mod spec;
mod store;

const SETUP_FN_NAME: &str = "setup";
const USER_REQUEST_FN_NAME: &str = "requests";

pub use parser::http::HttpMethod;
pub use parser::parse_static_spec;
pub use response::Response;
pub use runner::{Error, ScriptRunner};
pub use spec::{HTTPStaticRequestSpec, LuaRequestSpec, RequestSpec};
pub use store::Store;
