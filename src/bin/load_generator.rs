use creo_bench::load_generator;

#[tokio::main]
async fn main() {
    if let Err(e) = load_generator::run().await {
        tracing::error!("Failed to run load generator: {e}");
        std::process::exit(1);
    }
}
