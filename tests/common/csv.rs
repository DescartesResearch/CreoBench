//! Utilities for reading output CSV files.

use std::path::Path;

/// One row of `interval.csv`.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct IntervalRow {
    pub target_time: f64,
    pub load_level: u32,
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub timeout_transactions: u64,
    pub dropped_transactions: u64,
    pub avg_service_time: u64,
    pub final_batch_time: Option<f64>,
    pub phase: String,
}

/// One row of `transactions.csv`.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct TransactionRow {
    pub target_time: f64,
    pub start_time: f64,
    pub load_generator_id: u8,
    pub virtual_user_id: u32,
    pub spec_id: Option<u32>,
    pub response_time_ms: Option<u64>,
    pub outcome: String,
    pub reason: String,
}

/// Reads all rows of an `interval.csv` file.
pub fn interval_rows(path: impl AsRef<Path>) -> Vec<IntervalRow> {
    let mut reader = csv::Reader::from_path(path.as_ref()).unwrap();
    reader.deserialize().map(|record| record.unwrap()).collect()
}

/// Reads all rows of a `transactions.csv` file.
pub fn transaction_rows(path: impl AsRef<Path>) -> Vec<TransactionRow> {
    let mut reader = csv::Reader::from_path(path.as_ref()).unwrap();
    reader.deserialize().map(|record| record.unwrap()).collect()
}
