use std::time::{Duration, Instant};

/// Waits shorter than this are spent busy-waiting instead of sleeping.
///
/// Sleeping may overshoot its deadline by up to about a millisecond,
/// depending on the operating system's timer slack and scheduler.
/// Busy-waiting the final stretch makes sub-millisecond scheduling precise
/// at the cost of keeping the waiting thread busy.
const SPIN_THRESHOLD: Duration = Duration::from_millis(1);

/// Blocks the current thread until `deadline`.
///
/// Sleeps until shortly before the deadline and busy-waits for the rest.
/// Returns immediately if the deadline has already passed, so the wait is
/// never negative. Never returns before the deadline.
///
/// This blocks the calling thread, so it must not be called from an async
/// task; use [`tokio::task::spawn_blocking`] instead.
pub fn wait_until(deadline: Instant) {
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return;
        };
        if remaining <= SPIN_THRESHOLD {
            while Instant::now() < deadline {
                std::hint::spin_loop();
            }
            return;
        }
        std::thread::sleep(remaining - SPIN_THRESHOLD);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_immediately_for_past_deadline() {
        let deadline = Instant::now();
        std::thread::sleep(Duration::from_millis(5));

        let before = Instant::now();
        wait_until(deadline);

        assert!(before.elapsed() < SPIN_THRESHOLD);
    }

    #[test]
    fn never_returns_before_deadline() {
        let offsets = [
            Duration::ZERO,
            Duration::from_micros(1),
            Duration::from_micros(50),
            Duration::from_micros(500),
            Duration::from_millis(1),
            Duration::from_millis(3),
            Duration::from_millis(15),
        ];
        for offset in offsets {
            let deadline = Instant::now() + offset;

            wait_until(deadline);

            assert!(
                Instant::now() >= deadline,
                "returned before deadline for offset {offset:?}"
            );
        }
    }

    /// Checks the scheduling precision of exponentially distributed waits at
    /// 50,000 requests per second. Depends on the machine, so it is ignored by
    /// default; run it on the benchmarking machine with `--ignored`.
    #[test]
    #[ignore]
    fn lag_is_below_50_microseconds_at_high_rates() {
        use rand::SeedableRng;

        use crate::math::exponential::sample_exponential;

        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let mut deadline = Instant::now();
        let mut lags: Vec<_> = (0..100_000)
            .map(|_| {
                deadline += Duration::from_secs_f64(sample_exponential(1.0 / 50_000.0, &mut rng));
                wait_until(deadline);
                Instant::now().saturating_duration_since(deadline)
            })
            .collect();
        lags.sort();

        let p99 = lags[lags.len() * 99 / 100];
        assert!(p99 < Duration::from_micros(50), "p99 lag was {p99:?}");
    }
}
