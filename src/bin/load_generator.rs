use creo_bench::load_generator;

fn main() {
    let runtime = match load_generator::runtime::build() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("Failed to start async runtime: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = runtime.block_on(load_generator::run()) {
        tracing::error!("Failed to run load generator: {e}");
        std::process::exit(1);
    }
}
