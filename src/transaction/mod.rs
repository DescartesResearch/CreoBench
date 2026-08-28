mod batch;
mod id;
mod metadata;
mod result;

pub use batch::Batch;
pub use id::{LoadGeneratorId, SpecId};
pub use metadata::Metadata;
pub use result::{DroppedCode, FailedCode, TimeoutCode, TransactionResult};

use crate::load::{RelativeLoadTestTime, ServiceTime, StartTime};
use crate::virtual_user::VirtualUserId;

use self::metadata::{DroppedMetadata, ResultMetadata};

#[derive(Debug, PartialEq, Eq)]
pub struct Transaction {
    metadata: Metadata,
}

impl Transaction {
    pub fn new(
        loadgenerator_id: LoadGeneratorId,
        start_time: StartTime,
        target_time: RelativeLoadTestTime,
    ) -> Self {
        let metadata = Metadata {
            loadgenerator_id,
            start_time,
            target_time,
        };
        Self { metadata }
    }

    pub fn start_time(&self) -> StartTime {
        self.metadata.start_time
    }

    pub fn into_dropped(
        self,
        virtual_user_id: VirtualUserId,
        code: DroppedCode,
    ) -> TransactionResult {
        TransactionResult::Dropped {
            metadata: DroppedMetadata {
                virtual_user_id,
                loadgenerator_id: self.metadata.loadgenerator_id,
                start_time: self.metadata.start_time.relative_to_start(),
                target_time: self.metadata.target_time,
            },
            response_time: self.start_time().elapsed(),
            code,
        }
    }

    pub fn into_success(
        self,
        virtual_user_id: VirtualUserId,
        spec_id: SpecId,
        service_time: ServiceTime,
    ) -> TransactionResult {
        TransactionResult::Success {
            metadata: ResultMetadata {
                spec_id,
                virtual_user_id,
                loadgenerator_id: self.metadata.loadgenerator_id,
                start_time: self.metadata.start_time.relative_to_start(),
                target_time: self.metadata.target_time,
            },
            response_time: self.start_time().elapsed(),
            service_time,
        }
    }

    pub fn into_failed(
        self,
        virtual_user_id: VirtualUserId,
        spec_id: SpecId,
        service_time: ServiceTime,
        code: FailedCode,
    ) -> TransactionResult {
        TransactionResult::Failed {
            metadata: ResultMetadata {
                spec_id,
                virtual_user_id,
                loadgenerator_id: self.metadata.loadgenerator_id,
                start_time: self.metadata.start_time.relative_to_start(),
                target_time: self.metadata.target_time,
            },
            response_time: self.start_time().elapsed(),
            service_time,
            code,
        }
    }

    pub fn into_timeout(
        self,
        virtual_user_id: VirtualUserId,
        spec_id: SpecId,
        service_time: ServiceTime,
        code: TimeoutCode,
    ) -> TransactionResult {
        TransactionResult::Timeout {
            metadata: ResultMetadata {
                spec_id,
                virtual_user_id,
                loadgenerator_id: self.metadata.loadgenerator_id,
                start_time: self.metadata.start_time.relative_to_start(),
                target_time: self.metadata.target_time,
            },
            response_time: self.start_time().elapsed(),
            service_time,
            code,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::load::{LoadTestTime, RelativeLoadTestTime, StartTime};
    use crate::virtual_user::VirtualUserId;

    fn sample_transaction() -> (
        Transaction,
        LoadGeneratorId,
        StartTime,
        RelativeLoadTestTime,
    ) {
        let loadgenerator_id = LoadGeneratorId::new(7);
        let start_time = StartTime::now(LoadTestTime::now());
        let target_time = RelativeLoadTestTime::new(Duration::from_millis(500));
        let transaction = Transaction::new(loadgenerator_id, start_time, target_time);
        (transaction, loadgenerator_id, start_time, target_time)
    }

    fn assert_result_metadata(
        metadata: ResultMetadata,
        loadgenerator_id: LoadGeneratorId,
        spec_id: SpecId,
        virtual_user_id: VirtualUserId,
        start_time: StartTime,
        target_time: RelativeLoadTestTime,
    ) {
        let ResultMetadata {
            spec_id: actual_spec_id,
            virtual_user_id: actual_virtual_user_id,
            loadgenerator_id: actual_loadgenerator_id,
            start_time: actual_start_time,
            target_time: actual_target_time,
        } = metadata;
        assert_eq!(actual_spec_id, spec_id);
        assert_eq!(actual_virtual_user_id, virtual_user_id);
        assert_eq!(actual_loadgenerator_id, loadgenerator_id);
        assert_eq!(actual_start_time, start_time.relative_to_start());
        assert_eq!(actual_target_time, target_time);
    }

    fn assert_dropped_metadata(
        metadata: DroppedMetadata,
        loadgenerator_id: LoadGeneratorId,
        virtual_user_id: VirtualUserId,
        start_time: StartTime,
        target_time: RelativeLoadTestTime,
    ) {
        let DroppedMetadata {
            virtual_user_id: actual_virtual_user_id,
            loadgenerator_id: actual_loadgenerator_id,
            start_time: actual_start_time,
            target_time: actual_target_time,
        } = metadata;
        assert_eq!(actual_virtual_user_id, virtual_user_id);
        assert_eq!(actual_loadgenerator_id, loadgenerator_id);
        assert_eq!(actual_start_time, start_time.relative_to_start());
        assert_eq!(actual_target_time, target_time);
    }

    #[test]
    fn into_success_produces_success_variant_with_metadata() {
        let (transaction, loadgenerator_id, start_time, target_time) = sample_transaction();
        let virtual_user_id = VirtualUserId::new(42);
        let spec_id = SpecId::new(3);
        let service_time = ServiceTime::new(Duration::from_millis(120));

        let result = transaction.into_success(virtual_user_id, spec_id, service_time);

        match result {
            TransactionResult::Success {
                metadata,
                service_time: actual_service_time,
                ..
            } => {
                assert_eq!(actual_service_time, service_time);
                assert_result_metadata(
                    metadata,
                    loadgenerator_id,
                    spec_id,
                    virtual_user_id,
                    start_time,
                    target_time,
                );
            }
            result => panic!("unexpected result variant: {:?}", result),
        };
    }

    #[test]
    fn into_failed_produces_failed_variant_with_metadata_and_code() {
        let (transaction, loadgenerator_id, start_time, target_time) = sample_transaction();
        let virtual_user_id = VirtualUserId::new(11);
        let spec_id = SpecId::new(4);
        let service_time = ServiceTime::new(Duration::from_millis(75));
        let code = FailedCode::Status(503);

        let result = transaction.into_failed(virtual_user_id, spec_id, service_time, code.clone());

        match result {
            TransactionResult::Failed {
                metadata,
                service_time: actual_service_time,
                code: actual_code,
                ..
            } => {
                assert_eq!(actual_service_time, service_time);
                assert_eq!(actual_code, code);
                assert_result_metadata(
                    metadata,
                    loadgenerator_id,
                    spec_id,
                    virtual_user_id,
                    start_time,
                    target_time,
                );
            }
            result => panic!("unexpected result variant: {:?}", result),
        };
    }

    #[test]
    fn into_timeout_produces_timeout_variant_with_metadata_and_code() {
        let (transaction, loadgenerator_id, start_time, target_time) = sample_transaction();
        let virtual_user_id = VirtualUserId::new(13);
        let spec_id = SpecId::new(5);
        let service_time = ServiceTime::new(Duration::from_millis(2_000));
        let code = TimeoutCode::Error("deadline exceeded".to_string());

        let result = transaction.into_timeout(virtual_user_id, spec_id, service_time, code.clone());

        match result {
            TransactionResult::Timeout {
                metadata,
                service_time: actual_service_time,
                code: actual_code,
                ..
            } => {
                assert_eq!(actual_service_time, service_time);
                assert_eq!(actual_code, code);
                assert_result_metadata(
                    metadata,
                    loadgenerator_id,
                    spec_id,
                    virtual_user_id,
                    start_time,
                    target_time,
                );
            }
            result => panic!("unexpected result variant: {:?}", result),
        };
    }

    #[test]
    fn into_dropped_produces_dropped_variant_with_dropped_metadata() {
        let (transaction, loadgenerator_id, start_time, target_time) = sample_transaction();
        let virtual_user_id = VirtualUserId::new(99);
        let code = DroppedCode::Error("runner error".to_string());

        let result = transaction.into_dropped(virtual_user_id, code.clone());

        match result {
            TransactionResult::Dropped {
                metadata,
                code: actual_code,
                ..
            } => {
                assert_eq!(actual_code, code);
                assert_dropped_metadata(
                    metadata,
                    loadgenerator_id,
                    virtual_user_id,
                    start_time,
                    target_time,
                );
            }
            result => panic!("unexpected result variant: {:?}", result),
        };
    }
}
