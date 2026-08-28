use crate::config::WarmupConfig;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Warmup {
    pub rate: u32,
    pub duration: u32,
    pub pause: u32,
}

impl From<WarmupConfig> for Warmup {
    fn from(cfg: WarmupConfig) -> Self {
        Self {
            rate: cfg.rate,
            duration: cfg.duration,
            pause: cfg.pause,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_cfg_warmup_config_preserves_all_fields() {
        let cfg = WarmupConfig {
            rate: 10,
            duration: 30,
            pause: 5,
        };
        let wire: Warmup = cfg.into();
        assert_eq!(wire.rate, 10);
        assert_eq!(wire.duration, 30);
        assert_eq!(wire.pause, 5);
    }
}
