use std::time::Duration;

use super::LoadStepRemaining;

/// Represents a duration to wait between sending batches of requests.
///
/// This wrapper around [`std::time::Duration`] provides type safety for wait times,
/// distinguishing them from times relative to the load test start ([`super::RelativeLoadTestTime`])
/// or load step durations ([`super::LoadStepDuration`]).
///
/// # Examples
/// ```
/// use std::time::Duration;
/// # use creo_bench::load::WaitTime;
///
/// let wait = WaitTime::new(Duration::from_millis(100));
/// assert_eq!(wait.as_duration(), Duration::from_millis(100));
/// assert_eq!(WaitTime::ZERO, WaitTime::new(Duration::ZERO));
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WaitTime(Duration);

impl WaitTime {
    /// A constant representing zero wait time.
    ///
    /// This is equivalent to `WaitTime::new(Duration::ZERO)`.
    pub const ZERO: WaitTime = WaitTime::new(Duration::ZERO);

    /// Creates a new `WaitTime` from a [`Duration`].
    ///
    /// # Arguments
    ///
    /// * `duration` - The time to wait.
    ///
    /// # Returns
    ///
    /// A new `WaitTime` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// # use creo_bench::load::WaitTime;
    ///
    /// let wait = WaitTime::new(Duration::from_secs(5));
    /// assert_eq!(wait.as_duration(), Duration::from_secs(5));
    /// ```
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Returns the total number of seconds contained in this [`WaitTime`] as [`f64`].
    ///
    /// This is a convenience method equivalent to calling `as_duration().as_secs_f64()`.
    ///
    /// # Returns
    ///
    /// The fractional number of seconds in the wait time.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// # use creo_bench::load::WaitTime;
    ///
    /// let wait = WaitTime::new(Duration::from_millis(1500));
    /// assert_eq!(wait.as_secs_f64(), 1.5);
    /// ```
    pub fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }

    /// Creates a new `WaitTime` from a floating-point number of seconds.
    ///
    /// # Arguments
    ///
    /// * `secs` - The number of seconds to wait.
    ///
    /// # Returns
    ///
    /// A new `WaitTime` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// # use creo_bench::load::WaitTime;
    ///
    /// let wait = WaitTime::from_secs_f64(2.5);
    /// assert_eq!(wait.as_secs_f64(), 2.5);
    /// ```
    pub fn from_secs_f64(secs: f64) -> WaitTime {
        WaitTime(Duration::from_secs_f64(secs))
    }

    /// Creates a new `WaitTime` from a number of nanoseconds.
    ///
    /// # Arguments
    ///
    /// * `nanos` - The number of nanoseconds to wait.
    ///
    /// # Returns
    ///
    /// A new `WaitTime` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// # use creo_bench::load::WaitTime;
    ///
    /// let wait = WaitTime::from_nanos(1_500_000_000);
    /// assert_eq!(wait.as_secs_f64(), 1.5);
    /// ```
    pub const fn from_nanos(nanos: u64) -> WaitTime {
        WaitTime(Duration::from_nanos(nanos))
    }

    /// Returns the underlying [`Duration`] representing this wait time.
    ///
    /// # Returns
    ///
    /// The [`Duration`] to wait.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// # use creo_bench::load::WaitTime;
    ///
    /// let wait = WaitTime::new(Duration::from_millis(500));
    /// assert_eq!(wait.as_duration(), Duration::from_millis(500));
    /// ```
    pub fn as_duration(&self) -> Duration {
        self.0
    }
}

impl PartialEq<LoadStepRemaining> for WaitTime {
    fn eq(&self, other: &LoadStepRemaining) -> bool {
        self.as_duration() == other.as_duration()
    }
}

impl PartialOrd<LoadStepRemaining> for WaitTime {
    fn partial_cmp(&self, other: &LoadStepRemaining) -> Option<std::cmp::Ordering> {
        self.as_duration().partial_cmp(&other.as_duration())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_new() {
        let duration = Duration::from_secs(10);
        let wait_time = WaitTime::new(duration);
        assert_eq!(wait_time.as_duration(), duration);
    }

    #[test]
    fn test_zero() {
        assert_eq!(WaitTime::ZERO, WaitTime::new(Duration::ZERO));
        assert_eq!(WaitTime::ZERO.as_duration(), Duration::ZERO);
    }

    #[test]
    fn test_from_secs_f64() {
        let secs = 3.95141;
        let wait_time = WaitTime::from_secs_f64(secs);
        assert_eq!(wait_time.as_secs_f64(), secs);
    }

    #[test]
    fn test_from_nanos() {
        let nanos = 2_500_000_000u64;
        let wait_time = WaitTime::from_nanos(nanos);
        assert_eq!(wait_time.as_duration(), Duration::from_nanos(nanos));
        assert_eq!(wait_time.as_secs_f64(), 2.5);
    }

    #[test]
    fn test_as_duration() {
        let duration = Duration::from_millis(1234);
        let wait_time = WaitTime::new(duration);
        assert_eq!(wait_time.as_duration(), duration);
    }

    #[test]
    fn test_as_secs_f64() {
        let duration = Duration::from_micros(5_678_901);
        let wait_time = WaitTime::new(duration);
        assert_eq!(wait_time.as_secs_f64(), 5.678901);
    }

    #[test]
    fn test_partial_eq_remaining_step_duration_equal() {
        let duration = Duration::from_secs(5);
        let wait_time = WaitTime::new(duration);
        let remaining = LoadStepRemaining::new(duration);

        assert_eq!(wait_time, remaining);
        assert_eq!(remaining, wait_time);
    }

    #[test]
    fn test_partial_eq_remaining_step_duration_not_equal() {
        let wait_duration = Duration::from_secs(3);
        let remaining_duration = Duration::from_secs(7);
        let wait_time = WaitTime::new(wait_duration);
        let remaining = LoadStepRemaining::new(remaining_duration);

        assert_ne!(wait_time, remaining);
        assert_ne!(remaining, wait_time);
    }

    #[test]
    fn test_partial_ord_remaining_step_duration_less() {
        let wait_duration = Duration::from_secs(2);
        let remaining_duration = Duration::from_secs(5);
        let wait_time = WaitTime::new(wait_duration);
        let remaining = LoadStepRemaining::new(remaining_duration);

        assert!(wait_time < remaining);
        assert!(remaining > wait_time);
    }

    #[test]
    fn test_partial_ord_remaining_step_duration_greater() {
        let wait_duration = Duration::from_secs(8);
        let remaining_duration = Duration::from_secs(3);
        let wait_time = WaitTime::new(wait_duration);
        let remaining = LoadStepRemaining::new(remaining_duration);

        assert!(wait_time > remaining);
        assert!(remaining < wait_time);
    }

    #[test]
    fn test_partial_ord_remaining_step_duration_equal() {
        let duration = Duration::from_secs(4);
        let wait_time = WaitTime::new(duration);
        let remaining = LoadStepRemaining::new(duration);

        assert!((wait_time >= remaining));
        assert!((wait_time <= remaining));
        assert!(wait_time >= remaining);
        assert!(wait_time <= remaining);

        assert!((remaining >= wait_time));
        assert!((remaining <= wait_time));
        assert!(remaining >= wait_time);
        assert!(remaining <= wait_time);
    }

    #[test]
    fn test_wait_time_zero_duration() {
        let zero_wait = WaitTime::ZERO;
        assert_eq!(zero_wait.as_duration(), Duration::ZERO);
        assert_eq!(zero_wait.as_secs_f64(), 0.0);
    }

    #[test]
    fn test_wait_time_small_duration() {
        let small_duration = Duration::from_nanos(1);
        let small_wait = WaitTime::new(small_duration);
        assert_eq!(small_wait.as_duration(), small_duration);
    }

    #[test]
    fn test_wait_time_large_duration() {
        let large_duration = Duration::new(u64::MAX, 999_999_999);
        let large_wait = WaitTime::new(large_duration);
        assert_eq!(large_wait.as_duration(), large_duration);
    }

    #[test]
    fn test_wait_time_large_f64_secs() {
        let large_secs = 1_000_000_000.0;
        let large_wait_from_secs = WaitTime::from_secs_f64(large_secs);
        assert_eq!(large_wait_from_secs.as_secs_f64(), large_secs);
    }
}
