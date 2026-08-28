use std::sync::Arc;

use mlua::{Lua, Table};

use super::spec::{LuaRequestSpec, RequestSpec, SpecType};

mod error;
pub use error::{Error, Result};
pub mod http;

/// Parse a Lua table into a protocol-specific, static [`RequestSpec`].
///
/// The table must have a `protocol` field (e.g. `"http"`). The remaining
/// fields are validated per the protocol's schema.
///
/// # Errors
///
/// - `Error::Lua` if reading a Lua value fails.
/// - `Error::RequiredFieldMissing` if `protocol` is absent.
/// - `Error::FieldNotString` if `protocol` is not a string.
/// - `Error::FieldStringValueNotUTF8` if `protocol` is not valid UTF-8.
/// - `Error::InvalidProtocol` if `protocol` is a string but not a supported transport.
/// - `Error::InvalidHTTPSpec` if the protocol-specific fields are malformed.
pub fn parse_static_spec(lua: &Lua, table: &Table) -> Result<RequestSpec> {
    let protocol: mlua::Value = table.get("protocol")?;
    let protocol = match protocol {
        mlua::Value::Nil => return Err(Error::RequiredFieldMissing("protocol")),
        mlua::Value::String(p) => p,
        other => {
            return Err(Error::FieldNotString {
                field: "protocol",
                type_name: other.type_name(),
            });
        }
    };
    let protocol = protocol
        .to_str()
        .map_err(|_| Error::FieldStringValueNotUTF8 {
            field: "protocol",
            value: protocol.clone(),
        })?
        .to_lowercase();
    match protocol.as_str() {
        "http" => Ok(RequestSpec::Http(http::parse_http_spec(lua, table)?)),
        _ => Err(Error::InvalidProtocol(protocol)),
    }
}

/// Parse a Lua value as a [`LuaRequestSpec`].
///
/// Accepts:
/// - A `table` — either an anonymous spec (no `name`) or a named spec with
///   a `name` field (string) and a `spec` field (table for a static spec,
///   function for a dynamic spec).
/// - A bare `function` — treated as an anonymous dynamic spec.
///
/// # Errors
///
/// - [`Error::Lua`] if reading a Lua value fails.
/// - [`Error::FieldNotString`] if `name` is present but not a string.
/// - [`Error::FieldStringValueNotUTF8`] if `name` is not valid UTF-8.
/// - [`Error::InvalidSpecType`] if a named spec's `spec` field is neither a table nor a function.
/// - [`Error::InvalidRequestSpec`] if the top-level value is neither a table nor a function.
/// - Any error from [`parse_static_spec`] when validating a static spec.
pub fn parse_spec(lua: &Lua, value: mlua::Value) -> Result<LuaRequestSpec> {
    match value {
        mlua::Value::Table(t) => {
            let name: Option<Arc<str>> = match t.get::<mlua::Value>("name")? {
                mlua::Value::Nil => None,
                mlua::Value::String(s) => Some(
                    s.to_str()
                        .map_err(|_| Error::FieldStringValueNotUTF8 {
                            field: "name",
                            value: s.clone(),
                        })?
                        .to_owned()
                        .into(),
                ),
                other => {
                    return Err(Error::FieldNotString {
                        field: "name",
                        type_name: other.type_name(),
                    });
                }
            };

            let spec = match name {
                None => SpecType::Static(parse_static_spec(lua, &t)?),
                Some(_) => match t.get("spec")? {
                    mlua::Value::Table(t) => SpecType::Static(parse_static_spec(lua, &t)?),
                    mlua::Value::Function(f) => SpecType::Dynamic(f),
                    other => return Err(Error::InvalidSpecType(other.type_name())),
                },
            };
            Ok(LuaRequestSpec { name, spec })
        }
        mlua::Value::Function(f) => Ok(LuaRequestSpec {
            name: None,
            spec: SpecType::Dynamic(f),
        }),
        other => Err(Error::InvalidRequestSpec(other.type_name())),
    }
}
