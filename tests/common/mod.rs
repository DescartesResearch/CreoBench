//! Utilities for common integration test infrastructure.
//!
//! The main types in this module are:
//!
//! * [`HttpTestServer`][`http_server::HttpTestServer`] — a local HTTP test server.
//! * [`Orchestrator`][`orchestrator::Orchestrator`] — a handle to an orchestrator.
//! * [`GeneratorInstance`][`generator::GeneratorInstance`] — a handle to a load generator instance.
//! * [`LoadTest`][`load_test::LoadTest`] — a campaign factory defining one end-to-end scenario.
#![allow(dead_code, unused)]

mod csv;
mod generator;
mod http_server;
mod load_test;
mod orchestrator;

pub mod prelude {
    pub use super::{
        load_test::{LoadTest, LoadTestBuilder, LoadTestConfig, Spec},
        orchestrator::OutputDir,
    };
    pub use creo_bench::test_utils::prelude::*;
}
