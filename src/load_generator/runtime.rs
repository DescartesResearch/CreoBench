use std::num::NonZeroUsize;

use crate::wire::configure::GeneratorInfo;

/// Builds the async runtime of the load generator.
///
/// One of the available cores is left to the pacer, which busy-waits on a
/// blocking thread to dispatch arrivals precisely (see
/// [`Dispatcher::run_schedule`](crate::dispatch::Dispatcher::run_schedule)).
/// The remaining cores run the async workers that execute requests.
pub fn build() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads(available_cores()))
        .enable_all()
        .build()
}

/// Details about this load generator to report to the orchestrator.
///
/// Must be called from within the load generator's runtime.
pub fn generator_info() -> GeneratorInfo {
    GeneratorInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cores: u32::try_from(available_cores().get()).unwrap_or(u32::MAX),
        worker_threads: u32::try_from(tokio::runtime::Handle::current().metrics().num_workers())
            .unwrap_or(u32::MAX),
    }
}

/// Number of cores this process may run on.
///
/// Respects CPU affinity masks (e.g., set by `taskset`) and, on Linux,
/// cgroup CPU quotas.
pub fn available_cores() -> NonZeroUsize {
    std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN)
}

/// Number of async worker threads for `cores` available cores.
///
/// Reserves one core for the pacer, but always keeps at least one worker.
pub fn worker_threads(cores: NonZeroUsize) -> usize {
    cores.get().saturating_sub(1).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cores(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }

    #[test]
    fn reserves_one_core_for_the_pacer() {
        assert_eq!(worker_threads(cores(2)), 1);
        assert_eq!(worker_threads(cores(8)), 7);
        assert_eq!(worker_threads(cores(64)), 63);
    }

    #[test]
    fn keeps_one_worker_on_a_single_core() {
        assert_eq!(worker_threads(cores(1)), 1);
    }
}
