use std::time::Duration;

use crate::load::{LoadTestTime, RelativeLoadTestTime, ServiceTime, StartTime};
use crate::tracker::{IntervalStatistics, Tracker};
use crate::transaction::{
    DroppedCode, FailedCode, LoadGeneratorId, SpecId, TimeoutCode, Transaction, TransactionResult,
};
use crate::virtual_user::VirtualUserId;

pub fn fresh_transaction() -> Transaction {
    Transaction::new(
        LoadGeneratorId::new(1),
        StartTime::now(LoadTestTime::now()),
        RelativeLoadTestTime::new(Duration::from_millis(200)),
    )
}

pub fn stats_with_measured_transactions(
    success_ms: &[u64],
    failed_ms: &[u64],
) -> IntervalStatistics {
    let tracker = Tracker::new();
    for (vu_id, rt) in success_ms.iter().enumerate() {
        tracker.log_result(fresh_transaction().into_success(
            VirtualUserId::new(vu_id as u32),
            SpecId::new(0),
            ServiceTime::new(Duration::from_millis(*rt)),
        ));
    }
    for (vu_id, rt) in failed_ms.iter().enumerate() {
        tracker.log_result(fresh_transaction().into_failed(
            VirtualUserId::new((success_ms.len() + vu_id) as u32),
            SpecId::new(0),
            ServiceTime::new(Duration::from_millis(*rt)),
            FailedCode::Send("test".into()),
        ));
    }
    tracker.interval_statistics()
}

fn transaction_starting_at(start_ms: u64, target_ms: u64) -> Transaction {
    Transaction::new(
        LoadGeneratorId::new(1),
        StartTime::from_relative(RelativeLoadTestTime::new(Duration::from_millis(start_ms))),
        RelativeLoadTestTime::new(Duration::from_millis(target_ms)),
    )
}

pub fn success_result(
    vu_id: u32,
    spec_id: usize,
    start_ms: u64,
    target_ms: u64,
    response_ms: u64,
) -> TransactionResult {
    transaction_starting_at(start_ms, target_ms).into_success(
        VirtualUserId::new(vu_id),
        SpecId::new(spec_id),
        ServiceTime::new(Duration::from_millis(response_ms)),
    )
}

pub fn failed_result(
    vu_id: u32,
    spec_id: usize,
    start_ms: u64,
    target_ms: u64,
    response_ms: u64,
    code: FailedCode,
) -> TransactionResult {
    transaction_starting_at(start_ms, target_ms).into_failed(
        VirtualUserId::new(vu_id),
        SpecId::new(spec_id),
        ServiceTime::new(Duration::from_millis(response_ms)),
        code,
    )
}

pub fn timeout_result(
    vu_id: u32,
    spec_id: usize,
    start_ms: u64,
    target_ms: u64,
    response_ms: u64,
    code: TimeoutCode,
) -> TransactionResult {
    transaction_starting_at(start_ms, target_ms).into_timeout(
        VirtualUserId::new(vu_id),
        SpecId::new(spec_id),
        ServiceTime::new(Duration::from_millis(response_ms)),
        code,
    )
}

pub fn dropped_result(
    vu_id: u32,
    start_ms: u64,
    target_ms: u64,
    code: DroppedCode,
) -> TransactionResult {
    transaction_starting_at(start_ms, target_ms).into_dropped(VirtualUserId::new(vu_id), code)
}

pub fn stats_with(results: &[TransactionResult]) -> IntervalStatistics {
    let tracker = Tracker::new();
    for result in results {
        tracker.log_result(result.clone());
    }
    tracker.interval_statistics()
}
