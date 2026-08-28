//! Virtual user and virtual user pool management.
//!
//! This module provides abstractions for managing virtual users and their lifecycle
//! within a load generator. Virtual users are the abstract actors that issue requests
//! during load test execution.

mod http;
mod pool;
mod user;

pub use pool::{Error, Pool};
pub use user::VirtualUser;

/// A unique identifier for a virtual user within a load generator process.
///
/// `VirtualUserId` is a thin newtype around `u32`. A virtual user is the
/// abstract actor that issues requests; each one is assigned a fresh
/// identifier when it is spawned.
///
/// Identifiers are created internally by the pool and are comparable
/// with `u32` values via the `PartialEq<u32>` implementation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct VirtualUserId(u32);

impl VirtualUserId {
    /// Creates a new `VirtualUserId` from a raw `u32` value.
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for VirtualUserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialEq<u32> for VirtualUserId {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}
