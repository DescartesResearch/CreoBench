use crate::load::{ResponseTime, ServiceTime};

use super::metadata::{DroppedMetadata, ResultMetadata};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransactionResult {
    Success {
        metadata: ResultMetadata,
        response_time: ResponseTime,
        service_time: ServiceTime,
    },
    Failed {
        metadata: ResultMetadata,
        response_time: ResponseTime,
        service_time: ServiceTime,
        code: FailedCode,
    },
    Timeout {
        metadata: ResultMetadata,
        response_time: ResponseTime,
        service_time: ServiceTime,
        code: TimeoutCode,
    },
    Dropped {
        metadata: DroppedMetadata,
        response_time: ResponseTime,
        code: DroppedCode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DroppedCode {
    WaitTimeTooLong(ResponseTime),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FailedCode {
    Send(String),
    Extract(String),
    Status(u16),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TimeoutCode {
    Error(String),
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use proptest::prelude::*;

    use super::*;
    use crate::load::{RelativeLoadTestTime, ResponseTime};
    use crate::test_utils::{impl_proptest_arbitrary, proptest_strategy, round_trip_proptest};
    use crate::transaction::{LoadGeneratorId, SpecId, VirtualUserId};

    proptest_strategy! {
        relative_load_test_time_strategy: RelativeLoadTestTime => {
            (0u64..1_000_000_000)
                .prop_map(|ms| RelativeLoadTestTime::new(Duration::from_millis(ms)))
        }
    }

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

    proptest_strategy! {
        result_metadata_strategy: ResultMetadata => {
            (
                any::<usize>().prop_map(SpecId::new),
                any::<u32>().prop_map(VirtualUserId::new),
                any::<u8>().prop_map(LoadGeneratorId::new),
                relative_load_test_time_strategy(),
                relative_load_test_time_strategy(),
            )
                .prop_map(
                    |(spec_id, virtual_user_id, loadgenerator_id, start_time, target_time)| {
                        ResultMetadata {
                            spec_id,
                            virtual_user_id,
                            loadgenerator_id,
                            start_time,
                            target_time,
                        }
                    },
                )
        }
    }

    proptest_strategy! {
        dropped_metadata_strategy: DroppedMetadata => {
            (
                any::<u32>().prop_map(VirtualUserId::new),
                any::<u8>().prop_map(LoadGeneratorId::new),
                relative_load_test_time_strategy(),
                relative_load_test_time_strategy(),
            )
                .prop_map(
                    |(virtual_user_id, loadgenerator_id, start_time, target_time)| {
                        DroppedMetadata {
                            virtual_user_id,
                            loadgenerator_id,
                            start_time,
                            target_time,
                        }
                    },
                )
        }
    }

    proptest_strategy! {
        dropped_code_strategy: DroppedCode => {
            prop_oneof![
                response_time_strategy().prop_map(DroppedCode::WaitTimeTooLong),
                any::<String>().prop_map(DroppedCode::Error),
            ]
        }
    }

    proptest_strategy! {
        failed_code_strategy: FailedCode => {
            prop_oneof![
                any::<String>().prop_map(FailedCode::Send),
                any::<String>().prop_map(FailedCode::Extract),
                any::<u16>().prop_map(FailedCode::Status),
            ]
        }
    }

    proptest_strategy! {
        timeout_code_strategy: TimeoutCode => {
            any::<String>().prop_map(TimeoutCode::Error)
        }
    }

    proptest_strategy! {
        transaction_result_strategy: TransactionResult => {
            prop_oneof![
                (result_metadata_strategy(), response_time_strategy(), service_time_strategy())
                    .prop_map(|(metadata, response_time, service_time)| TransactionResult::Success { metadata, response_time, service_time }),
                (result_metadata_strategy(), response_time_strategy(), service_time_strategy(), failed_code_strategy())
                    .prop_map(|(metadata, response_time, service_time, code)| TransactionResult::Failed { metadata, response_time, service_time, code }),
                (result_metadata_strategy(), response_time_strategy(), service_time_strategy(), timeout_code_strategy())
                    .prop_map(|(metadata, response_time, service_time, code)| TransactionResult::Timeout { metadata, response_time, service_time, code }),
                (dropped_metadata_strategy(), response_time_strategy(), dropped_code_strategy())
                    .prop_map(|(metadata, response_time, code)| TransactionResult::Dropped { metadata, response_time, code }),
            ]
        }
    }

    impl_proptest_arbitrary!(DroppedCode, dropped_code_strategy);
    impl_proptest_arbitrary!(FailedCode, failed_code_strategy);
    impl_proptest_arbitrary!(TimeoutCode, timeout_code_strategy);
    impl_proptest_arbitrary!(ResultMetadata, result_metadata_strategy);
    impl_proptest_arbitrary!(DroppedMetadata, dropped_metadata_strategy);
    impl_proptest_arbitrary!(TransactionResult, transaction_result_strategy);

    round_trip_proptest! {
        TransactionResult,
        transaction_result_round_trip_single,
        transaction_result_round_trip_multi,
        transaction_result_round_trip_stream,
    }
}
