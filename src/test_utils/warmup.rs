use crate::config::WarmupConfig;

/// A builder for warmup configs.
#[derive(Debug, Clone, Default)]
pub struct WarmupBuilder {
    warmup: WarmupConfig,
}

impl WarmupBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the request rate of the warmup phase.
    pub fn with_rate(mut self, rate: u32) -> Self {
        self.warmup.rate = rate;
        self
    }

    /// Gets the request rate of the warmup phase.
    pub fn rate(&self) -> u32 {
        self.warmup.rate
    }

    /// Sets the duration of the warmup phase in seconds.
    pub fn with_duration(mut self, duration: u32) -> Self {
        self.warmup.duration = duration;
        self
    }

    /// Gets the duration of the warmup phase in seconds.
    pub fn duration(&self) -> u32 {
        self.warmup.duration
    }

    /// Sets the duration of the pause between warmup and load phases.
    pub fn with_pause(mut self, pause: u32) -> Self {
        self.warmup.pause = pause;
        self
    }

    /// Gets the duration of the pause between warmup and load phases.
    pub fn pause(&self) -> u32 {
        self.warmup.pause
    }

    /// Serializes this config into the warmup YAML format the loader parses.
    #[cfg(feature = "test-utils")]
    pub fn to_yaml(&self) -> String {
        yaml_serde::to_string(&self.warmup).unwrap()
    }
}
