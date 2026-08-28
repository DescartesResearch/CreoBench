use std::time::Duration;

use super::WaitTime;

/// Represents the duration of time remaining within a [`LoadInterval`][`crate::load::LoadInterval`]'s
/// allocated interval.
///
/// This wrapper around [`std::time::Duration`] ensures type safety by preventing
/// accidental mixing with times relative to the load test start ([`super::RelativeLoadTestTime`]),
/// total step durations ([`super::LoadStepDuration`]), or elapsed durations
/// ([`super::LoadStepElapsed`]).
///
/// It represents the time left from the current moment until the deadline
/// of the associated [`LoadInterval`][`crate::load::LoadInterval`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadStepRemaining(Duration);

impl LoadStepRemaining {
    /// Creates a new `LoadStepRemaining` from a [`Duration`].
    ///
    /// # Arguments
    /// * `duration` - The time remaining.
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Returns the underlying [`Duration`].
    pub fn as_duration(&self) -> Duration {
        self.0
    }

    /// Returns the total number of seconds contained in this [`LoadStepRemaining`] as [`f64`].
    ///
    /// This is a convenience method equivalent to calling `as_duration().as_secs_f64()`.
    ///
    /// # Returns
    ///
    /// The fractional number of seconds in the duration.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// # use creo_bench::load::LoadStepRemaining;
    ///
    /// let remaining = LoadStepRemaining::new(Duration::from_millis(1500));
    /// assert_eq!(remaining.as_secs_f64(), 1.5);
    /// ```
    pub fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }

    /// Calculates the maximum safe wait time, ensuring that at least the duration
    /// of `minimum_time_for_next_operation` remains after waiting.
    ///
    /// This is useful for determining the longest wait permissible before sending
    /// the next batch, guaranteeing time is reserved for subsequent actions.
    ///
    /// Uses saturating subtraction to prevent underflow, returning [`WaitTime::ZERO`]
    /// if `minimum_time_for_next_operation` is greater than or equal to `self`.
    ///
    /// # Arguments
    /// * `minimum_time_for_next_operation`: The time duration that must remain
    ///   available after the calculated wait time (e.g., time needed to send a batch).
    ///
    /// # Returns
    /// The maximum [`WaitTime`] that can be used, or [`WaitTime::ZERO`] if no time
    /// remains or the minimum required time exceeds the available time.
    pub fn max_safe_wait_time(&self, minimum_time_for_next_operation: WaitTime) -> WaitTime {
        WaitTime::new(
            self.0
                .saturating_sub(minimum_time_for_next_operation.as_duration()),
        )
    }
}

impl PartialEq<WaitTime> for LoadStepRemaining {
    fn eq(&self, other: &WaitTime) -> bool {
        self.as_duration() == other.as_duration()
    }
}

impl PartialOrd<WaitTime> for LoadStepRemaining {
    fn partial_cmp(&self, other: &WaitTime) -> Option<std::cmp::Ordering> {
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
        let remaining = LoadStepRemaining::new(duration);
        assert_eq!(remaining.as_duration(), duration);
    }

    #[test]
    fn test_as_duration() {
        let duration = Duration::from_millis(1500);
        let remaining = LoadStepRemaining::new(duration);
        assert_eq!(remaining.as_duration(), duration);
    }

    #[test]
    fn test_as_secs_f64() {
        let duration = Duration::from_secs_f64(1.5);
        let remaining = LoadStepRemaining::new(duration);
        assert_eq!(remaining.as_secs_f64(), 1.5);
    }

    #[test]
    fn test_max_safe_wait_time_sufficient_time() {
        let total_remaining = Duration::from_secs(10);
        let remaining = LoadStepRemaining::new(total_remaining);
        let min_time_needed = WaitTime::new(Duration::from_secs(2));
        let expected_max_wait = WaitTime::new(Duration::from_secs(8));

        let max_wait = remaining.max_safe_wait_time(min_time_needed);
        assert_eq!(max_wait, expected_max_wait);
    }

    #[test]
    fn test_max_safe_wait_time_insufficient_time() {
        let total_remaining = Duration::from_secs(2);
        let remaining = LoadStepRemaining::new(total_remaining);
        let min_time_needed = WaitTime::new(Duration::from_secs(5));

        let max_wait = remaining.max_safe_wait_time(min_time_needed);
        assert_eq!(max_wait, WaitTime::ZERO);
    }

    #[test]
    fn test_max_safe_wait_time_exact_time() {
        let total_remaining = Duration::from_secs(5);
        let remaining = LoadStepRemaining::new(total_remaining);
        let min_time_needed = WaitTime::new(Duration::from_secs(5));

        let max_wait = remaining.max_safe_wait_time(min_time_needed);
        assert_eq!(max_wait, WaitTime::ZERO);
    }

    #[test]
    fn test_max_safe_wait_time_zero_remaining() {
        let total_remaining = Duration::ZERO;
        let remaining = LoadStepRemaining::new(total_remaining);
        let min_time_needed = WaitTime::new(Duration::from_secs(1));

        let max_wait = remaining.max_safe_wait_time(min_time_needed);
        assert_eq!(max_wait, WaitTime::ZERO);
    }

    #[test]
    fn test_max_safe_wait_time_zero_min_time_needed() {
        let total_remaining = Duration::from_secs(5);
        let remaining = LoadStepRemaining::new(total_remaining);
        let min_time_needed = WaitTime::ZERO;

        let max_wait = remaining.max_safe_wait_time(min_time_needed);
        let expected_max_wait = WaitTime::new(Duration::from_secs(5));
        assert_eq!(max_wait, expected_max_wait);
    }

    #[test]
    fn test_partial_eq_wait_time_equal() {
        let duration = Duration::from_secs(5);
        let remaining = LoadStepRemaining::new(duration);
        let wait_time = WaitTime::new(duration);

        assert_eq!(remaining, wait_time);
        assert_eq!(wait_time, remaining);
    }

    #[test]
    fn test_partial_eq_wait_time_not_equal() {
        let remaining_duration = Duration::from_secs(5);
        let wait_time_duration = Duration::from_secs(3);
        let remaining = LoadStepRemaining::new(remaining_duration);
        let wait_time = WaitTime::new(wait_time_duration);

        assert_ne!(remaining, wait_time);
        assert_ne!(wait_time, remaining);
    }

    #[test]
    fn test_partial_ord_wait_time_less() {
        let remaining_duration = Duration::from_secs(3);
        let wait_time_duration = Duration::from_secs(5);
        let remaining = LoadStepRemaining::new(remaining_duration);
        let wait_time = WaitTime::new(wait_time_duration);

        assert!(remaining < wait_time);
        assert!(wait_time > remaining);
    }

    #[test]
    fn test_partial_ord_wait_time_greater() {
        let remaining_duration = Duration::from_secs(7);
        let wait_time_duration = Duration::from_secs(5);
        let remaining = LoadStepRemaining::new(remaining_duration);
        let wait_time = WaitTime::new(wait_time_duration);

        assert!(remaining > wait_time);
        assert!(wait_time < remaining);
    }

    #[test]
    fn test_partial_ord_wait_time_equal() {
        let duration = Duration::from_secs(4);
        let remaining = LoadStepRemaining::new(duration);
        let wait_time = WaitTime::new(duration);

        assert!((remaining >= wait_time));
        assert!((remaining <= wait_time));
        assert!(remaining >= wait_time);
        assert!(remaining <= wait_time);

        assert!((wait_time >= remaining));
        assert!((wait_time <= remaining));
        assert!(wait_time >= remaining);
        assert!(wait_time <= remaining);
    }

    #[test]
    fn test_load_step_remaining_zero() {
        let zero_remaining = LoadStepRemaining::new(Duration::ZERO);
        assert_eq!(zero_remaining.as_duration(), Duration::ZERO);
        assert_eq!(zero_remaining.as_secs_f64(), 0.0);
    }

    #[test]
    fn test_load_step_remaining_small() {
        let small_duration = Duration::from_nanos(1);
        let small_remaining = LoadStepRemaining::new(small_duration);
        assert_eq!(small_remaining.as_duration(), small_duration);
    }

    #[test]
    fn test_load_step_remaining_large() {
        let large_duration = Duration::new(u64::MAX, 999_999_999);
        let large_remaining = LoadStepRemaining::new(large_duration);
        assert_eq!(large_remaining.as_duration(), large_duration);
    }

    #[test]
    fn test_max_safe_wait_with_large_duration() {
        let large_duration = Duration::new(u64::MAX, 999_999_999);
        let large_remaining = LoadStepRemaining::new(large_duration);
        let min_needed = WaitTime::new(Duration::from_secs(1));
        let max_wait = large_remaining.max_safe_wait_time(min_needed);
        assert_eq!(
            max_wait.as_duration(),
            (large_duration - min_needed.as_duration())
        );
    }
}
