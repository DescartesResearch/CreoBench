//! Utilities for defining a [`LoadTest`]
use std::io::Write;

use tempfile::NamedTempFile;

use super::{
    generator::GeneratorInstance, http_server::HttpTestServer, orchestrator::Orchestrator,
};
use creo_bench::config::LoadStepConfig;
use creo_bench::test_utils::prelude::*;

#[derive(Debug)]
pub struct LoadTestConfig {
    /// The total number of requests the load test dispatches.
    pub total_requests: u32,
    /// The user specs in spec-id order.
    pub user_specs: Vec<Spec>,
    /// The warmup duration of the load test.
    pub warmup_duration: u32,
    /// The warmup rate of the load test.
    pub warmup_rate: u32,
    /// The warmup pause of the load test.
    pub warmup_pause: u32,
    /// The number of intervals in the load test.
    pub interval_count: usize,

    _server: HttpTestServer,
}

#[derive(Debug)]
pub struct Spec {
    /// The 0-based spec id the runner assigns to this spec.
    pub id: u32,
    /// The HTTP method of this spec.
    pub method: &'static str,
    /// The path this spec targets.
    pub path: &'static str,
    /// The service this spec targets.
    pub service: &'static str,
    /// The body of this spec
    pub body: Option<serde_json::Value>,
    /// The headers of this spec
    pub headers: Vec<(&'static str, &'static str)>,
    /// The query of this spec
    pub query: Vec<(&'static str, &'static str)>,
}

impl LoadTestConfig {
    fn new(
        user_specs: Vec<Spec>,
        steps: &[LoadStepConfig],
        server: HttpTestServer,
        warmup: &WarmupBuilder,
    ) -> Self {
        let total_requests = steps.iter().map(|step| step.count).sum();
        Self {
            total_requests,
            user_specs,
            _server: server,
            warmup_duration: warmup.duration(),
            warmup_rate: warmup.rate(),
            warmup_pause: warmup.pause(),
            interval_count: steps.len(),
        }
    }
}

/// A [`LoadTest`] defines one end-to-end benchmark.
///
/// It creates required input files (i.e., the script, profile, warmup,
/// and registry configuration files), spawns the requested number of
/// generator instances, and registers the server routes the script expects.
pub struct LoadTest {
    script: NamedTempFile,
    profile: NamedTempFile,
    warmup: NamedTempFile,
    registry: NamedTempFile,
    generators: Vec<GeneratorInstance>,
    virtual_user_count: u32,
    seed: u64,
    config: LoadTestConfig,
}

impl LoadTest {
    pub fn start(self) -> (Orchestrator, Vec<GeneratorInstance>, LoadTestConfig) {
        let Self {
            script,
            profile,
            warmup,
            registry,
            generators,
            virtual_user_count,
            seed,
            config,
        } = self;

        let orchestrator = Orchestrator::new(
            script,
            profile,
            warmup,
            registry,
            generators
                .iter()
                .map(|generator| generator.listen_addr().clone()),
        )
        .with_virtual_user_count(virtual_user_count)
        .with_seed(seed);

        (orchestrator, generators, config)
    }
}

pub struct LoadTestBuilder {
    server: HttpTestServer,
}

impl LoadTestBuilder {
    pub async fn new() -> Self {
        Self {
            server: HttpTestServer::new().await,
        }
    }

    pub fn url(&self) -> String {
        self.server.url()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn build(
        self,
        script: ScriptBuilder,
        profile: ProfileBuilder,
        warmup: WarmupBuilder,
        registry: RegistryBuilder,
        count: u8,
        virtual_user_count: u32,
        seed: u64,
        user_specs: Vec<Spec>,
    ) -> LoadTest {
        let mut server = self.server;
        for spec in &user_specs {
            let mut route = server.route(spec.method, spec.path).with_status(200);
            if let Some(ref body) = spec.body {
                route = route.with_header("Content-Type", "application/json");
                route = route.with_body(body);
            }
            for (name, value) in &spec.headers {
                route = route.matching_header(name, value);
            }
            route = route.matching_query(&spec.query);

            route.create();
        }

        let mut generators = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            generators.push(GeneratorInstance::spawn().await);
        }
        let config = LoadTestConfig::new(user_specs, profile.steps(), server, &warmup);

        LoadTest {
            script: write_to_file(&script.build()),
            profile: write_to_file(&profile.to_csv()),
            warmup: write_to_file(&warmup.to_yaml()),
            registry: write_to_file(&registry.to_yaml()),
            generators,
            virtual_user_count,
            seed,
            config,
        }
    }
}

fn write_to_file(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    file
}
