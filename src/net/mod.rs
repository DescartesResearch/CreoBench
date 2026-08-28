//! Network communication module for the load generator.
//!
//! This module provides the foundation for communication between the orchestrator
//! and load agents using a custom binary TCP protocol. It includes (de-)serialization
//! traits and the base protocol implementation.

mod framer;
mod protocol;

mod ser;
use bytes::TryGetError;
use ser::BytesSerializer;
mod de;
use de::BytesDeserializer;

pub mod arc_str {
    pub fn serialize<S>(value: &std::sync::Arc<str>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        s.serialize_str(value)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<std::sync::Arc<str>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(std::sync::Arc::from(
            <String as serde::Deserialize>::deserialize(d)?,
        ))
    }
}

use std::fmt;
use std::str::Utf8Error;

pub use framer::{DecoderError, EncoderError, MessageFramer};
pub use protocol::{HeaderDeError, HeaderSeError};

#[derive(Debug)]
pub struct SerdeError {
    msg: String,
}

impl fmt::Display for SerdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for SerdeError {}

impl serde::ser::Error for SerdeError {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Self {
            msg: format!("failed to serialize data: {msg}"),
        }
    }
}

impl serde::de::Error for SerdeError {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Self {
            msg: format!("failed to serialize data: {msg}"),
        }
    }
}

impl From<TryGetError> for SerdeError {
    fn from(value: TryGetError) -> Self {
        Self {
            msg: format!(
                "unexpected end of buffer: expected `{}` bytes but only `{}` bytes remain",
                value.requested, value.available
            ),
        }
    }
}

impl From<Utf8Error> for SerdeError {
    fn from(value: Utf8Error) -> Self {
        Self {
            msg: format!("string bytes are not valid UTF-8: {value}"),
        }
    }
}
