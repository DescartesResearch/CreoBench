use std::time::Duration;

use crate::wire::LoadStepDeadline;

use super::{LoadStepElapsed, LoadStepRemaining, LoadStepStart, LoadTestTime};

/// Represents a time interval between two points in a load test.
///
/// A [`LoadStepDuration`] represents the length of time between a load step's start and end.
///
/// # Invariants
/// A [`LoadStepDuration`] can never represent a zero duration. Attempting to create
/// one with [`LoadStepDuration::new`] or [`LoadStepDuration::until_deadline`] will panic.
///
/// # Examples
/// ```
/// use std::time::Duration;
/// # use creo_bench::load::LoadStepDuration;
///
/// let duration = LoadStepDuration::new(Duration::from_secs(5));
/// assert_eq!(duration.as_secs_f64(), 5.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadStepDuration(Duration);

impl LoadStepDuration {
    /// Creates a new `LoadStepDuration` from a [`Duration`].
    ///
    /// Returns `None` if the provided `duration` is zero, as zero durations
    /// are not allowed for `LoadStepDuration`.
    ///
    /// # Arguments
    ///
    /// * `duration` - The time interval to represent.
    ///
    /// # Panics
    ///
    /// Panics if `duration` is [`Duration::ZERO`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// # use creo_bench::load::LoadStepDuration;
    ///
    /// let duration = LoadStepDuration::new(Duration::from_millis(100));
    ///
    /// // would panic!
    /// // let zero_duration = LoadStepDuration::new(Duration::ZERO);
    /// ```
    pub fn new(duration: Duration) -> Self {
        if duration == Duration::ZERO {
            panic!("duration must not be `Duration::ZERO`");
        }
        LoadStepDuration(duration)
    }

    /// Computes the [`LoadStepDuration`] between a [`LoadStepStart`] and a [`LoadStepDeadline`].
    ///
    /// Returns a [`LoadStepDuration`] representing the time interval between the
    /// step's start time and its deadline, where the deadline is expressed as
    /// an offset from `time_zero`.
    ///
    /// # Arguments
    ///
    /// * `step_start` - The start time of the load step.
    /// * `time_zero` - The reference time origin from which `deadline` is measured.
    /// * `deadline` - The deadline of the load step, as an offset from `time_zero`.
    ///
    /// # Returns
    ///
    /// * [`LoadStepDuration`] - The duration between `step_start` and the resolved deadline.
    ///
    /// # Panics
    ///
    /// Panics if the resolved duration (the deadline minus `step_start`) is zero,
    /// which violates the `LoadStepDuration`] non-zero invariant.
    pub fn until_deadline(
        step_start: LoadStepStart,
        time_zero: LoadTestTime,
        deadline: LoadStepDeadline,
    ) -> Self {
        let deadline = time_zero + deadline;
        let duration = deadline - step_start;
        Self::new(duration)
    }

    /// Returns the underlying [`Duration`] representing this time interval.
    ///
    /// # Returns
    ///
    /// The [`Duration`] of this interval.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// # use creo_bench::load::LoadStepDuration;
    ///
    /// let step_duration = LoadStepDuration::new(Duration::from_secs(30));
    /// assert_eq!(step_duration.as_duration(), Duration::from_secs(30));
    /// ```
    pub fn as_duration(&self) -> Duration {
        self.0
    }

    /// Returns the total number of seconds contained in this [`LoadStepDuration`] as [`f64`].
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
    /// # use creo_bench::load::LoadStepDuration;
    ///
    /// let step_duration = LoadStepDuration::new(Duration::from_millis(1500));
    /// assert_eq!(step_duration.as_secs_f64(), 1.5);
    /// ```
    pub fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }

    pub fn remaining_after(&self, rhs: LoadStepElapsed) -> LoadStepRemaining {
        LoadStepRemaining::new(self.0.saturating_sub(rhs.as_duration()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_step_duration_new() {
        let duration = LoadStepDuration::new(Duration::from_secs(5));
        assert_eq!(duration.as_duration(), Duration::from_secs(5));
    }

    #[test]
    #[should_panic = "duration must not be `Duration::ZERO`"]
    fn test_load_step_duration_new_zero() {
        let _ = LoadStepDuration::new(Duration::ZERO);
    }

    #[test]
    fn test_load_step_duration_as_secs_f64() {
        let duration = LoadStepDuration::new(Duration::from_millis(1500));
        assert_eq!(duration.as_secs_f64(), 1.5);
    }
    #[test]
    fn test_load_step_duration_extreme_values() {
        let small_duration = Duration::from_nanos(1);
        let duration = LoadStepDuration::new(small_duration);
        assert_eq!(duration.as_duration(), small_duration);

        let max_duration = Duration::new(u64::MAX, 999_999_999);
        let duration = LoadStepDuration::new(max_duration);
        assert_eq!(duration.as_duration(), max_duration);
    }
    #[test]
    fn test_load_step_duration_as_secs_f64_edge_cases() {
        let duration = LoadStepDuration::new(Duration::from_micros(1500));
        assert_eq!(duration.as_secs_f64(), 0.0015);

        let duration = LoadStepDuration::new(Duration::from_secs(86400)); // 1 day
        assert_eq!(duration.as_secs_f64(), 86400.0);
    }
}
