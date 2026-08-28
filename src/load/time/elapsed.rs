use std::time::{Duration, Instant};

/// Represents the starting point in time for a load test interval.
///
/// This wrapper around [`std::time::Instant`] provides type safety for marking
/// the beginning of a time interval, typically associated with a
/// [`LoadInterval`][`crate::load::LoadInterval`]'s execution.
/// It is used in conjunction with [`LoadStepElapsed`] to track time elapsed
/// since the interval began.
///
/// # Examples
/// ```
/// use creo_bench::load::LoadStepStart;
/// use std::time::Duration;
/// use std::thread;
///
/// let start = LoadStepStart::now();
///
/// thread::sleep(Duration::from_millis(10));
/// let elapsed = start.elapsed();
/// assert!(elapsed.as_duration() >= Duration::from_millis(10));
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadStepStart(pub(super) Instant);

/// Represents the duration of time that has elapsed since a [`LoadStepStart`].
///
/// This wrapper around [`std::time::Duration`] ensures type safety by preventing
/// accidental mixing with absolute load test times ([`super::LoadTestTime`]),
/// load step durations ([`super::LoadStepDuration`]), or time remaining
/// ([`super::LoadStepRemaining`]). It specifically represents the time
/// passed since a specific interval began.
///
/// It is typically obtained by calling [`LoadStepStart::elapsed`].
///
/// # Examples
/// ```
/// use creo_bench::load::{LoadStepStart, LoadStepElapsed};
/// use std::time::Duration;
/// use std::thread;
///
/// let start = LoadStepStart::now();
///
/// thread::sleep(Duration::from_millis(5));
/// let elapsed: LoadStepElapsed = start.elapsed();
/// assert!(elapsed.as_duration() >= Duration::from_millis(5));
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadStepElapsed(Duration);

impl LoadStepElapsed {
    /// Creates a new `LoadStepElapsed` from a [`Duration`].
    ///
    /// # Arguments
    /// * `duration` - The elapsed time.
    ///
    /// # Returns
    /// A new `LoadStepElapsed` instance.
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Returns the underlying [`Duration`] representing the elapsed time.
    ///
    /// # Returns
    /// The [`Duration`] since the associated interval started.
    pub fn as_duration(&self) -> Duration {
        self.0
    }

    /// Returns the total number of seconds contained in this [`LoadStepElapsed`] as [`f64`].
    ///
    /// This is a convenience method equivalent to calling `as_duration().as_secs_f64()`.
    ///
    /// # Returns
    /// The fractional number of seconds in the elapsed duration.
    pub fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }
}

impl LoadStepStart {
    /// Creates a new `LoadStepStart` representing the current instant.
    ///
    /// This captures the current time using [`std::time::Instant::now()`].
    ///
    /// # Returns
    /// A new `LoadStepStart` instance.
    pub fn now() -> Self {
        Self(Instant::now())
    }

    /// Creates a new `LoadStepStart` from a specific [`Instant`].
    ///
    /// # Arguments
    /// * `instant` - The point in time to mark as the start of the interval.
    ///
    /// # Returns
    /// A new `LoadStepStart` instance.
    pub const fn from_instant(instant: Instant) -> Self {
        Self(instant)
    }

    /// Returns the underlying [`Instant`] representing the start time.
    ///
    /// # Returns
    /// The [`Instant`] when the interval was started.
    pub fn as_instant(&self) -> Instant {
        self.0
    }

    /// Calculates the time elapsed since this `LoadStepStart`.
    ///
    /// This method calculates the duration between the stored start instant
    /// and the current instant ([`std::time::Instant::now()`]).
    ///
    /// # Returns
    /// A [`LoadStepElapsed`] representing the time passed.
    pub fn elapsed(&self) -> LoadStepElapsed {
        LoadStepElapsed(self.0.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_load_step_start_now() {
        let start1 = LoadStepStart::now();
        thread::sleep(Duration::from_nanos(100));
        let start2 = LoadStepStart::now();
        assert!(start2 > start1);
    }

    #[test]
    fn test_load_step_start_from_instant() {
        let instant = Instant::now();
        let start = LoadStepStart::from_instant(instant);
        assert_eq!(start.as_instant(), instant);
    }

    #[test]
    fn test_load_step_elapsed_new() {
        let duration = Duration::from_secs(10);
        let elapsed = LoadStepElapsed::new(duration);
        assert_eq!(elapsed.as_duration(), duration);
    }

    #[test]
    fn test_load_step_elapsed_as_secs_f64() {
        let duration = Duration::from_millis(1500);
        let elapsed = LoadStepElapsed::new(duration);
        assert_eq!(elapsed.as_secs_f64(), 1.5);
    }

    #[test]
    fn test_load_step_start_elapsed() {
        let start = LoadStepStart::now();
        thread::sleep(Duration::from_millis(2));
        let elapsed = start.elapsed();
        assert!(elapsed.as_duration() >= Duration::from_millis(2));
    }

    #[test]
    fn test_load_step_elapsed_ord() {
        let elapsed1 = LoadStepElapsed::new(Duration::from_secs(5));
        let elapsed2 = LoadStepElapsed::new(Duration::from_secs(10));
        assert!(elapsed1 < elapsed2);
        assert!(elapsed2 > elapsed1);
    }
}
