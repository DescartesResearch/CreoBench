use std::ops::Add;

use crate::config::{DeadlineConfig, LoadProfileConfig, LoadStepConfig};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LoadProfile {
    pub steps: Vec<LoadStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LoadStep {
    pub deadline: LoadStepDeadline,
    pub count: u32,
}

impl From<LoadProfileConfig> for LoadProfile {
    fn from(cfg: LoadProfileConfig) -> Self {
        Self {
            steps: cfg.steps.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<LoadStepConfig> for LoadStep {
    fn from(cfg: LoadStepConfig) -> Self {
        Self {
            deadline: cfg.deadline.into(),
            count: cfg.count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize)]
pub struct LoadStepDeadline(f64);

impl LoadStepDeadline {
    #[cfg(test)]
    pub fn new(v: f64) -> Self {
        Self(v)
    }

    pub fn as_f64(&self) -> f64 {
        self.0
    }

    pub fn from_secs(secs: u32) -> Self {
        Self(secs.into())
    }
}

impl Add<f64> for LoadStepDeadline {
    type Output = LoadStepDeadline;

    fn add(self, rhs: f64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl From<DeadlineConfig> for LoadStepDeadline {
    fn from(value: DeadlineConfig) -> Self {
        Self(value.as_f64())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{LoadProfileConfig, LoadStepConfig};

    use super::*;

    #[test]
    fn from_cfg_load_profile_preserves_all_fields() {
        let cfg = LoadProfileConfig {
            steps: vec![
                LoadStepConfig {
                    deadline: DeadlineConfig::new(30.0),
                    count: 1000,
                },
                LoadStepConfig {
                    deadline: DeadlineConfig::new(60.5),
                    count: 2000,
                },
            ],
        };
        let wire: LoadProfile = cfg.into();
        assert_eq!(wire.steps.len(), 2);
        assert_eq!(wire.steps[0].deadline.0, 30.0);
        assert_eq!(wire.steps[0].count, 1000);
        assert_eq!(wire.steps[1].deadline.0, 60.5);
        assert_eq!(wire.steps[1].count, 2000);
    }

    #[test]
    fn from_cfg_load_step_config_preserves_all_fields() {
        let cfg = LoadStepConfig {
            deadline: DeadlineConfig::new(30.0),
            count: 1000,
        };
        let wire: LoadStep = cfg.into();
        assert_eq!(wire.deadline.0, 30.0);
        assert_eq!(wire.count, 1000);
    }
}
