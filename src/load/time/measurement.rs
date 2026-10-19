use std::time::{Duration, Instant};

use super::{LoadTestTime, RelativeLoadTestTime, ResponseTime, ServiceTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartTime {
    instant: LoadTestTime,
    relative_to_start: RelativeLoadTestTime,
}

impl StartTime {
    pub fn now(time_zero: LoadTestTime) -> Self {
        let now = LoadTestTime::now();
        let relative_to_start = now.duration_since(time_zero);
        Self {
            instant: now,
            relative_to_start,
        }
    }

    /// Creates a start time at a scheduled arrival rather than at the current time.
    ///
    /// Response times and wait times are then measured from the scheduled
    /// arrival, so any delay in dispatching a transaction is part of them.
    pub fn scheduled(time_zero: LoadTestTime, offset: RelativeLoadTestTime) -> Self {
        Self {
            instant: time_zero + offset,
            relative_to_start: offset,
        }
    }

    pub fn elapsed(&self) -> ResponseTime {
        ResponseTime::new(self.instant.elapsed().as_duration())
    }

    pub fn relative_to_start(&self) -> RelativeLoadTestTime {
        self.relative_to_start
    }

    #[cfg(test)]
    pub(crate) fn from_relative(relative_to_start: RelativeLoadTestTime) -> Self {
        Self {
            instant: LoadTestTime::now(),
            relative_to_start,
        }
    }
}

#[derive(Debug)]
pub struct ServiceTimeMeasurement(Instant);

impl ServiceTimeMeasurement {
    pub fn now() -> Self {
        Self(Instant::now())
    }

    pub fn elapsed(&self) -> ServiceTime {
        ServiceTime::new(self.0.elapsed())
    }
}

impl PartialEq<Duration> for ResponseTime {
    fn eq(&self, other: &Duration) -> bool {
        self.as_duration() == *other
    }
}

impl PartialOrd<Duration> for ResponseTime {
    fn partial_cmp(&self, other: &Duration) -> Option<std::cmp::Ordering> {
        self.as_duration().partial_cmp(other)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn relative_to_start_is_stable_across_calls() {
        let time_zero = LoadTestTime::now();
        let start_time = StartTime::now(time_zero);

        let first = start_time.relative_to_start();
        tokio::time::sleep(Duration::from_millis(2)).await;
        let second = start_time.relative_to_start();

        assert_eq!(first, second);
    }

    #[tokio::test(start_paused = true)]
    async fn scheduled_start_time_is_relative_to_time_zero() {
        let time_zero = LoadTestTime::now();
        let offset = RelativeLoadTestTime::new(Duration::from_millis(10));

        let start_time = StartTime::scheduled(time_zero, offset);

        assert_eq!(start_time.relative_to_start(), offset);
    }

    #[test]
    fn scheduled_start_time_in_the_past_includes_lag_in_elapsed() {
        let time_zero = LoadTestTime::now();
        std::thread::sleep(Duration::from_millis(20));

        let start_time = StartTime::scheduled(
            time_zero,
            RelativeLoadTestTime::new(Duration::from_millis(5)),
        );

        assert!(start_time.elapsed() >= Duration::from_millis(15));
    }

    #[tokio::test(start_paused = true)]
    async fn service_time_measurement_tracks_elapsed_time() {
        let measurement = ServiceTimeMeasurement::now();
        tokio::time::sleep(Duration::from_millis(2)).await;
        let elapsed = measurement.elapsed();
        assert_ne!(elapsed, ServiceTime::new(Duration::from_millis(2)));
    }
}
