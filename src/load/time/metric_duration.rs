use std::marker::PhantomData;
use std::time::Duration;

/// Marker type for [`ServiceTime`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceTimeMarker;

/// Marker type for [`ResponseTime`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResponseTimeMarker;

/// A measurement of a duration.
///
/// Measurement of logical separate durations can be distinguished
/// using `Kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DurationMeasurement<Kind> {
    inner: Duration,
    _kind: PhantomData<Kind>,
}

/// Time taken for a service to process a request (server-side).
pub type ServiceTime = DurationMeasurement<ServiceTimeMarker>;

/// Total time for a request round-trip (client-side).
pub type ResponseTime = DurationMeasurement<ResponseTimeMarker>;

impl<Kind> DurationMeasurement<Kind> {
    /// Create a new `DuratonMeasurement` from the given [`Duration`].
    pub fn new(duration: Duration) -> Self {
        Self {
            inner: duration,
            _kind: PhantomData,
        }
    }

    /// Returns the underlying [`Duration`].
    pub fn as_duration(&self) -> Duration {
        self.inner
    }

    /// Returns the duration as a whole number of milliseconds.
    pub fn as_millis(&self) -> u64 {
        self.inner.as_millis() as u64
    }
}

impl<Kind> serde::Serialize for DurationMeasurement<Kind> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        u64::serialize(
            &self.inner.as_millis().try_into().map_err(|_| {
                serde::ser::Error::custom("duration milliseconds overflows u64::MAX")
            })?,
            serializer,
        )
    }
}

impl<'de, Kind> serde::Deserialize<'de> for DurationMeasurement<Kind> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let ms = u64::deserialize(deserializer)?;

        Ok(Self {
            inner: Duration::from_millis(ms),
            _kind: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use proptest::prelude::*;

    use super::*;
    use crate::test_utils::{impl_proptest_arbitrary, proptest_strategy, round_trip_proptest};

    proptest_strategy! {
        response_time_strategy: ResponseTime => {
            (0u64..1_000_000_000)
                .prop_map(|ms| ResponseTime::new(Duration::from_millis(ms)))
        }
    }

    proptest_strategy! {
        service_time_strategy: ServiceTime => {
            (0u64..1_000_000_000)
                .prop_map(|ms| ServiceTime::new(Duration::from_millis(ms)))
        }
    }

    impl_proptest_arbitrary!(ResponseTime, response_time_strategy);
    impl_proptest_arbitrary!(ServiceTime, service_time_strategy);

    round_trip_proptest! {
        ResponseTime,
        response_time_round_trip_single,
        response_time_round_trip_multi,
        response_time_round_trip_stream,
    }
}
