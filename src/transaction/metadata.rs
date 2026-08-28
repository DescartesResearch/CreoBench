use super::{LoadGeneratorId, SpecId};
use crate::load::{RelativeLoadTestTime, StartTime};
use crate::virtual_user::VirtualUserId;

#[derive(Debug, PartialEq, Eq)]
pub struct Metadata {
    pub loadgenerator_id: LoadGeneratorId,
    pub start_time: StartTime,
    pub target_time: RelativeLoadTestTime,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct DroppedMetadata {
    pub virtual_user_id: VirtualUserId,
    pub loadgenerator_id: LoadGeneratorId,
    pub start_time: RelativeLoadTestTime,
    pub target_time: RelativeLoadTestTime,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ResultMetadata {
    pub spec_id: SpecId,
    pub virtual_user_id: VirtualUserId,
    pub loadgenerator_id: LoadGeneratorId,
    pub start_time: RelativeLoadTestTime,
    pub target_time: RelativeLoadTestTime,
}
