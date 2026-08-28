use std::ops::{Add, Sub};
use std::time::{Duration, Instant};

use crate::wire::LoadStepDeadline;

use super::LoadStepStart;

/// Represents a specific point in time in a load test.
///
/// This wrapper around [`std::time::Instant`] ensures type safety by preventing
/// accidental mixing of absolute times and time intervals. A [`LoadTestTime`] represents
/// an absolute moment in the load test's timeline.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadTestTime(Instant);

impl LoadTestTime {
    /// Creates a new [`LoadTestTime`].
    ///
    /// # Returns
    ///
    /// A new [`LoadTestTime`] instance at the current time.
    pub fn now() -> Self {
        Self(Instant::now())
    }

    pub fn elapsed(&self) -> RelativeLoadTestTime {
        RelativeLoadTestTime(self.0.elapsed())
    }

    pub fn duration_since(&self, earlier: Self) -> RelativeLoadTestTime {
        RelativeLoadTestTime(self.0.duration_since(earlier.0))
    }
}

impl Add<LoadStepDeadline> for LoadTestTime {
    type Output = Self;

    fn add(self, rhs: LoadStepDeadline) -> Self::Output {
        let duration = Duration::from_secs_f64(rhs.as_f64());
        Self(self.0 + duration)
    }
}

impl Sub<LoadStepStart> for LoadTestTime {
    type Output = Duration;

    fn sub(self, rhs: LoadStepStart) -> Self::Output {
        self.0 - rhs.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelativeLoadTestTime(Duration);

impl RelativeLoadTestTime {
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }

    pub fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }

    pub fn as_duration(&self) -> Duration {
        self.0
    }
}

impl Sub<Duration> for RelativeLoadTestTime {
    type Output = RelativeLoadTestTime;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl<'de> serde::Deserialize<'de> for RelativeLoadTestTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let secs = f64::deserialize(deserializer)?;
        let duration = Duration::from_secs_f64(secs);
        Ok(Self(duration))
    }
}

impl serde::Serialize for RelativeLoadTestTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        f64::serialize(&self.0.as_secs_f64(), serializer)
    }
}

impl PartialEq<Duration> for RelativeLoadTestTime {
    fn eq(&self, other: &Duration) -> bool {
        self.0.eq(other)
    }
}

impl PartialOrd<Duration> for RelativeLoadTestTime {
    fn partial_cmp(&self, other: &Duration) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use proptest::prelude::*;

    use super::*;
    use crate::test_utils::{impl_proptest_arbitrary, proptest_strategy, round_trip_proptest};

    proptest_strategy! {
        relative_load_test_time_strategy: RelativeLoadTestTime => {
            (0u64..1_000_000_000)
                .prop_map(|ms| RelativeLoadTestTime::new(Duration::from_millis(ms)))
        }
    }

    impl_proptest_arbitrary!(RelativeLoadTestTime, relative_load_test_time_strategy);

    round_trip_proptest! {
        RelativeLoadTestTime,
        relative_load_test_time_round_trip_single,
        relative_load_test_time_round_trip_multi,
        relative_load_test_time_round_trip_stream,
    }
}
