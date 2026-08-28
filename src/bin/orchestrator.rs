use creo_bench::orchestrator;

#[tokio::main]
async fn main() {
    if let Err(e) = orchestrator::run().await {
        tracing::error!("Failed to run load test: {e}");
        std::process::exit(1);
    }
}
