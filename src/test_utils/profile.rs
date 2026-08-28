use crate::config::{DeadlineConfig, LoadProfileConfig, LoadStepConfig};

/// A builder for load profiles made up of sequential steps.
#[derive(Debug, Clone)]
pub struct ProfileBuilder {
    profile: LoadProfileConfig,
}

impl Default for ProfileBuilder {
    fn default() -> Self {
        Self {
            profile: LoadProfileConfig {
                steps: Default::default(),
            },
        }
    }
}

impl ProfileBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a step to the end of the profile.
    ///
    /// Deadlines must be strictly increasing for the resulting CSV to load.
    pub fn add_step(mut self, deadline: f64, count: u32) -> Self {
        self.profile.steps.push(LoadStepConfig {
            deadline: DeadlineConfig::new(deadline),
            count,
        });
        self
    }

    /// The steps of this profile in order.
    pub fn steps(&self) -> &[LoadStepConfig] {
        &self.profile.steps
    }

    /// Serializes this profile into the profile CSV format the loader
    /// parses.
    ///
    /// # Panics
    ///
    /// Panics if the profile has no steps: the loader rejects an empty
    /// profile, so an empty profile is a test-authoring error.
    #[cfg(feature = "test-utils")]
    pub fn to_csv(&self) -> String {
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.write_record(["deadline", "count"]).unwrap();
        for step in self.steps() {
            writer.serialize((step.deadline, step.count)).unwrap();
        }
        writer.flush().unwrap();
        String::from_utf8(writer.into_inner().unwrap()).unwrap()
    }
}
