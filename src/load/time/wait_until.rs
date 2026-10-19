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

    /// Checks that short waits are busy-waited rather than slept.
    ///
    /// Busy-waiting returns within tens of nanoseconds of the deadline, while
    /// sleeping wakes up microseconds late even with precise timers (macOS) and
    /// tens of microseconds late on Linux. On a busy machine such as a shared CI
    /// runner, the waiting thread is sometimes preempted and misses deadlines by
    /// much more, so the test only requires a quarter of all waits to end within
    /// 2 µs. Busy-waiting stays above that even with twice as many CPU-bound
    /// threads as cores, while sleeping stays well below it.
    ///
    /// Windows timers are too coarse and unreliable for this, so the test only
    /// runs on Unix-like systems.
    #[test]
    #[cfg_attr(
        not(unix),
        ignore = "high-resolution timing is unreliable on this platform"
    )]
    fn busy_waiting_ends_short_waits_within_2_microseconds() {
        use rand::SeedableRng;

        use crate::math::exponential::sample_exponential;

        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let mut deadline = Instant::now();
        let samples = 50_000;
        let precise = (0..samples)
            .filter(|_| {
                deadline += Duration::from_secs_f64(sample_exponential(1.0 / 50_000.0, &mut rng));
                wait_until(deadline);
                Instant::now().saturating_duration_since(deadline) < Duration::from_micros(2)
            })
            .count();

        let share = precise as f64 / f64::from(samples);
        assert!(
            share >= 0.25,
            "only {:.1}% of waits ended within 2 µs",
            share * 100.0
        );
    }
}
