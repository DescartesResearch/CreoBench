use std::sync::Arc;

use crate::transaction::LoadGeneratorId;

use super::{LoadProfile, ServiceRegistry, Warmup};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LoadGeneratorConfig {
    pub profile: LoadProfile,
    #[serde(with = "crate::net::arc_str")]
    pub script: Arc<str>,
    pub registry: ServiceRegistry,
    pub warmup: Warmup,
    pub virtual_user_count: u32,
    pub seed: u64,
    pub timeout_ms: u64,
    pub load_generator_id: LoadGeneratorId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConfigMessage {
    Config(LoadGeneratorConfig),
    Abort,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConfigResponse {
    Ready,
    SetupFailed { reason: String },
    Aborted,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use proptest::prelude::*;

    use super::*;
    use crate::test_utils::{impl_proptest_arbitrary, proptest_strategy, round_trip_proptest};
    use crate::wire::profile::LoadStepDeadline;
    use crate::wire::{LoadProfile, LoadStep, ServiceRegistry, Warmup};

    proptest_strategy! {
        load_profile_strategy: LoadProfile => {
            proptest::collection::vec(
                (proptest::num::f64::NORMAL, any::<u32>())
                    .prop_map(|(deadline, count)| LoadStep { deadline: LoadStepDeadline::new(deadline) , count }),
                1..10,
            )
            .prop_map(|steps| LoadProfile { steps })
        }
    }

    proptest_strategy! {
        registry_strategy: ServiceRegistry => {
            proptest::collection::btree_map(any::<String>(), any::<String>(), 1..10)
                .prop_map(|services| ServiceRegistry { services: services.into() })
        }
    }

    proptest_strategy! {
        warmup_config_strategy: Warmup => {
            (any::<u32>(), any::<u32>(), any::<u32>())
                .prop_map(|(rate, duration, pause)| Warmup { rate, duration, pause })
        }
    }

    proptest_strategy! {
        load_generator_config_strategy: LoadGeneratorConfig => {
            (
                load_profile_strategy(),
                any::<String>().prop_map(Arc::from),
                registry_strategy(),
                warmup_config_strategy(),
                any::<u32>(),
                any::<u64>(),
                any::<u64>(),
                any::<u8>().prop_map(LoadGeneratorId::new),
            )
                .prop_map(
                    |(profile, script, registry, warmup, virtual_user_count, seed, timeout_ms, load_generator_id)|
                        LoadGeneratorConfig {
                            profile,
                            script,
                            registry,
                            warmup,
                            virtual_user_count,
                            seed,
                            timeout_ms,
                            load_generator_id,
                        },
                )
        }
    }

    proptest_strategy! {
        config_message_strategy: ConfigMessage => {
            prop_oneof![
                load_generator_config_strategy().prop_map(ConfigMessage::Config),
                Just(ConfigMessage::Abort),
            ]
        }
    }

    proptest_strategy! {
        config_response_strategy: ConfigResponse => {
            prop_oneof![
                Just(ConfigResponse::Ready),
                any::<String>().prop_map(|reason| {
                    ConfigResponse::SetupFailed { reason }
                }),
                Just(ConfigResponse::Aborted),
            ]
        }
    }

    impl_proptest_arbitrary!(LoadGeneratorConfig, load_generator_config_strategy);
    impl_proptest_arbitrary!(ConfigMessage, config_message_strategy);
    impl_proptest_arbitrary!(ConfigResponse, config_response_strategy);

    round_trip_proptest! {
        LoadGeneratorConfig,
        load_generator_config_round_trip_single,
        load_generator_config_round_trip_multi,
        load_generator_config_round_trip_stream,
    }

    round_trip_proptest! {
        ConfigMessage,
        config_message_round_trip_single,
        config_message_round_trip_multi,
        config_message_round_trip_stream,
    }

    round_trip_proptest! {
        ConfigResponse,
        config_response_round_trip_single,
        config_response_round_trip_multi,
        config_response_round_trip_stream,
    }
}
