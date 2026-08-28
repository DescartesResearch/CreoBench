use super::{Error, FromBytes, Result};

/// A load profile made up of sequential load steps.
///
/// Each step defines a deadline and a target request count, and the
/// profile is used to drive the load generator's execution plan.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[cfg_attr(any(test, feature = "test-utils"), derive(serde::Serialize))]
pub struct LoadProfileConfig {
    /// The individual steps of the load profile.
    pub steps: Vec<LoadStepConfig>,
}

/// A single step within a [`LoadProfileConfig`].
///
/// Specifies how many requests must be sent by a given deadline.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[cfg_attr(any(test, feature = "test-utils"), derive(serde::Serialize))]
pub struct LoadStepConfig {
    /// The deadline in seconds relative to the load test start.
    pub deadline: DeadlineConfig,
    /// The number of requests to sent until the deadline.
    pub count: u32,
}

impl FromBytes for LoadProfileConfig {
    /// Parses a load profile from CSV bytes.
    ///
    /// The CSV must have columns `deadline` and `count`.
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut reader = csv::Reader::from_reader(bytes);
        let mut steps = Vec::new();
        let mut previous_step = None;
        for (idx, step) in reader.deserialize().enumerate() {
            let step: LoadStepConfig = step.map_err(|source| Error::Csv {
                step: idx + 1,
                source,
            })?;
            previous_step = validate_step(step, previous_step, idx)?;
            steps.push(step);
        }
        if steps.is_empty() {
            return Err(Error::EmptyLoadProfile);
        }
        Ok(Self { steps })
    }
}

fn validate_step(
    step: LoadStepConfig,
    prev: Option<LoadStepConfig>,
    idx: usize,
) -> Result<Option<LoadStepConfig>> {
    if let Some(prev) = prev
        && step.deadline.0 <= prev.deadline.0
    {
        return Err(Error::NonMonotonicDeadline {
            step: idx + 1,
            deadline: step.deadline.0,
            previous: prev.deadline.0,
        });
    }
    Ok(Some(step))
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(any(test, feature = "test-utils"), derive(serde::Serialize))]
pub struct DeadlineConfig(f64);

impl DeadlineConfig {
    #[cfg(any(test, feature = "test-utils"))]
    pub fn new(v: f64) -> Self {
        Self(v)
    }

    pub fn as_f64(&self) -> f64 {
        self.0
    }
}

impl<'de> serde::Deserialize<'de> for DeadlineConfig {
    fn deserialize<D>(deserializer: D) -> std::prelude::v1::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let this = f64::deserialize(deserializer)?;

        if !this.is_finite() {
            return Err(serde::de::Error::custom(format!(
                "load profile deadlines must be finite, but deadline `{this}` is non-finite"
            )));
        }

        if this.is_sign_negative() {
            return Err(serde::de::Error::custom(format!(
                "load profile deadlines must be positive, but deadline `{this}` is non-positive"
            )));
        }

        Ok(Self(this))
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use crate::config::Error;

    use super::*;

    #[test]
    fn load_profile_valid_csv_parses_correctly() {
        let profile =
            LoadProfileConfig::from_bytes(b"deadline,count\n30.0,1000\n60.5,2000\n").unwrap();
        assert_eq!(profile.steps.len(), 2);
        assert_eq!(profile.steps[0].deadline.0, 30.0);
        assert_eq!(profile.steps[0].count, 1000);
        assert_eq!(profile.steps[1].deadline.0, 60.5);
        assert_eq!(profile.steps[1].count, 2000);
    }

    #[test]
    fn load_profile_invalid_csv_data_returns_error() {
        let result = LoadProfileConfig::from_bytes(b"deadline,count\nnot-a-number,1000\n");
        assert_matches!(result, Err(Error::Csv{step, ..}) if step == 1);
    }

    #[test]
    fn load_profile_missing_csv_column_returns_error() {
        let result = LoadProfileConfig::from_bytes(b"deadline\n30.0\n");
        assert_matches!(result, Err(Error::Csv{step, ..}) if step == 1);
    }

    #[test]
    fn load_profile_empty_csv_returns_error() {
        let err = LoadProfileConfig::from_bytes(b"").unwrap_err();
        assert_matches!(err, Error::EmptyLoadProfile);
    }

    #[test]
    fn load_profile_non_monotonic_returns_error() {
        let err =
            LoadProfileConfig::from_bytes(b"deadline,count\n2.0,1000\n1.0,2000\n").unwrap_err();
        assert_matches!(
            err,
            Error::NonMonotonicDeadline {
                step,
                deadline,
                previous
            }
            if step == 2 && deadline == 1.0 && previous == 2.0
        );
    }

    // TODO: Non-finite and Non-positive tests
}
