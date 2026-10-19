use std::time::{Duration, Instant};

use super::{LoadTestTime, RelativeLoadTestTime, ResponseTime, ServiceTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartTime {
    instant: LoadTestTime,
    relative_to_start: RelativeLoadTestTime,
}

impl StartTime {
    #[cfg(test)]
    pub(crate) fn now(time_zero: LoadTestTime) -> Self {
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

    /// Returns `instant` relative to the start of the load test.
    ///
    /// Instants before this start time are clamped to it.
    pub fn relative_to_start_of(&self, instant: Instant) -> RelativeLoadTestTime {
        let since_start = instant.saturating_duration_since(self.instant.into());
        RelativeLoadTestTime::new(self.relative_to_start.as_duration() + since_start)
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

    /// Ends the measurement, keeping when the request was sent.
    pub fn finish(self) -> RequestTiming {
        RequestTiming {
            sent_at: self.0,
            service_time: self.elapsed(),
        }
    }
}

/// When a request was sent and how long it took to receive its response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestTiming {
    pub sent_at: Instant,
    pub service_time: ServiceTime,
}

impl RequestTiming {
    /// A timing for a request sent `delay` after `start_time`.
    #[cfg(test)]
    pub(crate) fn sent_after(
        start_time: StartTime,
        delay: Duration,
        service_time: ServiceTime,
    ) -> Self {
        Self {
            sent_at: Instant::from(start_time.instant) + delay,
            service_time,
        }
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

    #[test]
    fn relative_to_start_of_adds_time_since_start() {
        let time_zero = LoadTestTime::now();
        let start_time = StartTime::scheduled(
            time_zero,
            RelativeLoadTestTime::new(Duration::from_millis(5)),
        );
        let instant = Instant::from(time_zero) + Duration::from_millis(8);

        assert_eq!(
            start_time.relative_to_start_of(instant),
            RelativeLoadTestTime::new(Duration::from_millis(8))
        );
    }

    #[test]
    fn relative_to_start_of_clamps_instants_before_start() {
        let time_zero = LoadTestTime::now();
        let offset = RelativeLoadTestTime::new(Duration::from_millis(5));
        let start_time = StartTime::scheduled(time_zero, offset);

        assert_eq!(start_time.relative_to_start_of(time_zero.into()), offset);
    }

    #[test]
    fn finish_keeps_send_instant() {
        let before = Instant::now();
        let measurement = ServiceTimeMeasurement::now();
        std::thread::sleep(Duration::from_millis(2));

        let timing = measurement.finish();

        assert!(timing.sent_at >= before);
        assert!(timing.service_time.as_duration() >= Duration::from_millis(2));
    }

    #[tokio::test(start_paused = true)]
    async fn service_time_measurement_tracks_elapsed_time() {
        let measurement = ServiceTimeMeasurement::now();
        tokio::time::sleep(Duration::from_millis(2)).await;
        let elapsed = measurement.elapsed();
        assert_ne!(elapsed, ServiceTime::new(Duration::from_millis(2)));
    }
}
