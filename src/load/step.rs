use super::LoadStepDuration;

/// Defines a load interval with a specific number of requests to be completed in a given duration.
///
/// A [`LoadInterval`] represents a segment of a load test where a certain number of requests
/// must be sent and completed within a specified time frame. Each step implicitly begins
/// when the previous [`LoadInterval`] ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadInterval {
    /// The duration in which all requests in this load interval must be completed.
    duration: LoadStepDuration,
    /// The number of requests to send during this load interval.
    request_count: u32,
}

impl LoadInterval {
    /// Creates a new [`LoadInterval`] with the specified duration and request count.
    ///
    /// # Arguments
    /// * `duration` - The duration of the load interval
    /// * `request_count` - The number of requests to send during this interval
    ///
    /// # Returns
    /// A new [`LoadInterval`] instance
    ///
    /// # Examples
    /// ```
    /// use std::time::Duration;
    /// # use creo_bench::load::{LoadInterval, LoadStepDuration};
    ///
    /// let interval = LoadInterval::new(LoadStepDuration::new(Duration::from_secs(30)), 150);
    /// assert_eq!(interval.duration(), LoadStepDuration::new(Duration::from_secs(30)));
    /// assert_eq!(interval.request_count(), 150);
    /// ```
    pub const fn new(duration: LoadStepDuration, request_count: u32) -> Self {
        Self {
            duration,
            request_count,
        }
    }

    /// Returns the duration of this load interval.
    pub fn duration(&self) -> LoadStepDuration {
        self.duration
    }

    /// Returns the number of requests in this load interval.
    pub fn request_count(&self) -> u32 {
        self.request_count
    }

    /// Estimates the request rate for this load interval.
    ///
    /// Calculates the requests per second (RPS) based on the number of requests
    /// and the duration of this step.
    ///
    /// # Arguments
    /// * `previous` - The previous [`LoadInterval`] in the sequence
    ///
    /// # Returns
    ///
    /// The estimated requests per second
    ///
    /// # Examples
    /// ```
    /// use std::time::Duration;
    /// # use creo_bench::load::{LoadInterval, LoadStepDuration};
    ///
    /// let interval = LoadInterval::new(
    ///     LoadStepDuration::new(Duration::from_secs(5)),
    ///     100,
    /// );
    ///
    /// let rps = interval.estimated_rps();
    /// assert_eq!(rps, 20.0); // 100 requests in 5 seconds = 20 RPS
    /// ```
    pub fn estimated_rps(&self) -> f64 {
        let seconds = self.duration.as_secs_f64();
        (self.request_count as f64) / seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_estimated_rps() {
        let first = LoadInterval {
            duration: LoadStepDuration::new(Duration::from_millis(1000)),
            request_count: 10,
        };
        let second = LoadInterval {
            duration: LoadStepDuration::new(Duration::from_millis(2000)),
            request_count: 40,
        };

        let rps1 = first.estimated_rps();
        let rps2 = second.estimated_rps();

        assert!((rps1 - 10.0).abs() < f64::EPSILON);
        assert!((rps2 - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_large_duration() {
        let interval = LoadInterval::new(LoadStepDuration::new(Duration::from_secs(1000)), 1000);

        let rps = interval.estimated_rps();

        assert!((rps - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_zero_request_count() {
        let interval = LoadInterval::new(LoadStepDuration::new(Duration::from_secs(10)), 0); // Zero requests

        let rps = interval.estimated_rps();
        assert_eq!(rps, 0.0); // 0 requests in 5 seconds = 0 RPS
    }
}
