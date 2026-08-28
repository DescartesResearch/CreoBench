use std::io::{BufWriter, Write};

use crate::tracker::IntervalReport;
use crate::transaction::TransactionResult;

use super::csv::escape_field;
use super::{PersistError, Persister, Phase};

pub const FILE_NAME: &str = "transactions.csv";
const HEADER: &str = "target_time,start_time,load_generator_id,virtual_user_id,spec_id,response_time_ms,service_time_ms,outcome,reason";

pub struct TransactionCsvPersister<W: Write> {
    writer: BufWriter<W>,
}

impl<W: Write> TransactionCsvPersister<W> {
    pub fn new(writer: W) -> Result<Self, PersistError> {
        let mut writer = BufWriter::new(writer);
        writeln!(writer, "{HEADER}").map_err(io_err)?;
        Ok(Self { writer })
    }

    pub fn format_row(&self, result: &TransactionResult) -> String {
        let (start_time, target_time, load_generator_id, virtual_user_id, response_time_ms) =
            match result {
                TransactionResult::Success {
                    metadata,
                    response_time,
                    ..
                } => (
                    metadata.start_time,
                    metadata.target_time,
                    metadata.loadgenerator_id.get(),
                    metadata.virtual_user_id,
                    response_time.as_millis(),
                ),
                TransactionResult::Failed {
                    metadata,
                    response_time,
                    ..
                } => (
                    metadata.start_time,
                    metadata.target_time,
                    metadata.loadgenerator_id.get(),
                    metadata.virtual_user_id,
                    response_time.as_millis(),
                ),
                TransactionResult::Timeout {
                    metadata,
                    response_time,
                    ..
                } => (
                    metadata.start_time,
                    metadata.target_time,
                    metadata.loadgenerator_id.get(),
                    metadata.virtual_user_id,
                    response_time.as_millis(),
                ),
                TransactionResult::Dropped {
                    metadata,
                    response_time,
                    ..
                } => (
                    metadata.start_time,
                    metadata.target_time,
                    metadata.loadgenerator_id.get(),
                    metadata.virtual_user_id,
                    response_time.as_millis(),
                ),
            };
        let (spec_id, service_time_ms, outcome, reason) = match result {
            TransactionResult::Success {
                metadata,
                service_time,
                ..
            } => (
                metadata.spec_id.to_string(),
                service_time.as_millis().to_string(),
                "success",
                String::new(),
            ),
            TransactionResult::Failed {
                metadata,
                service_time,
                code,
                ..
            } => (
                metadata.spec_id.to_string(),
                service_time.as_millis().to_string(),
                "failed",
                format!("{code:?}"),
            ),
            TransactionResult::Timeout {
                metadata,
                service_time,
                code,
                ..
            } => (
                metadata.spec_id.to_string(),
                service_time.as_millis().to_string(),
                "timeout",
                format!("{code:?}"),
            ),
            TransactionResult::Dropped { code, .. } => {
                (String::new(), String::new(), "dropped", format!("{code:?}"))
            }
        };
        let fields = [
            target_time.as_secs_f64().to_string(),
            start_time.as_secs_f64().to_string(),
            load_generator_id.to_string(),
            virtual_user_id.to_string(),
            spec_id,
            response_time_ms.to_string(),
            service_time_ms,
            outcome.to_string(),
            reason,
        ];
        fields
            .into_iter()
            .map(|field| escape_field(&field))
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
impl<W: Write + std::fmt::Debug> TransactionCsvPersister<W> {
    fn into_inner(self) -> W {
        self.writer
            .into_inner()
            .expect("in-memory transactions writer must flush")
    }
}

impl<W: Write + Send> Persister for TransactionCsvPersister<W> {
    fn persist(&mut self, report: &IntervalReport, _phase: Phase) -> Result<(), PersistError> {
        for result in report.stats.results() {
            writeln!(self.writer, "{}", self.format_row(result)).map_err(io_err)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), PersistError> {
        self.writer.flush().map_err(io_err)
    }
}

fn io_err(source: std::io::Error) -> PersistError {
    PersistError {
        name: FILE_NAME,
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::load::RelativeLoadTestTime;
    use crate::orchestrator::persist::test_utils::{
        dropped_result, failed_result, stats_with, success_result, timeout_result,
    };
    use crate::tracker::IntervalReport;
    use crate::transaction::{DroppedCode, FailedCode, TimeoutCode};

    #[test]
    fn construction_writes_header() {
        let persister = TransactionCsvPersister::new(Vec::new()).unwrap();
        let output = String::from_utf8(persister.into_inner()).unwrap();
        assert_eq!(
            output,
            "target_time,start_time,load_generator_id,virtual_user_id,spec_id,response_time_ms,service_time_ms,outcome,reason\n"
        );
    }

    #[ignore = "refactor"]
    #[test]
    fn success_row_renders_all_columns_with_empty_reason() {
        let result = success_result(0, 0, 250, 1000, 100);

        let persister = TransactionCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&result);

        assert_eq!(row, "1,0.25,1,0,0,100,success,");
    }

    #[ignore = "refactor"]
    #[test]
    fn dropped_row_renders_empty_spec_id_and_response_time_ms() {
        let result = dropped_result(3, 1900, 2000, DroppedCode::Error("wait timed out".into()));

        let persister = TransactionCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&result);

        assert_eq!(row, "2,1.9,1,3,,,dropped,\"Error(\"\"wait timed out\"\")\"");
    }

    #[ignore = "refactor"]
    #[test]
    fn failed_row_renders_debug_reason() {
        let result = failed_result(1, 1, 500, 1000, 200, FailedCode::Status(503));

        let persister = TransactionCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&result);

        assert_eq!(row, "1,0.5,1,1,1,200,failed,Status(503)");
    }

    #[ignore = "refactor"]
    #[test]
    fn timeout_row_renders_debug_reason() {
        let result = timeout_result(
            2,
            2,
            750,
            1000,
            300,
            TimeoutCode::Error("deadline exceeded".into()),
        );

        let persister = TransactionCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&result);

        assert_eq!(
            row,
            "1,0.75,1,2,2,300,timeout,\"Error(\"\"deadline exceeded\"\")\""
        );
    }

    #[ignore = "refactor"]
    #[test]
    fn reason_with_commas_and_quotes_is_rfc4180_escaped() {
        let result = failed_result(
            4,
            3,
            500,
            1000,
            150,
            FailedCode::Send("socket closed, retry".into()),
        );

        let persister = TransactionCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&result);

        assert_eq!(
            row,
            "1,0.5,1,4,3,150,failed,\"Send(\"\"socket closed, retry\"\")\""
        );
    }

    #[ignore = "refactor"]
    #[test]
    fn reason_message_with_embedded_quotes_keeps_debug_escaping() {
        let result = failed_result(
            4,
            3,
            500,
            1000,
            150,
            FailedCode::Extract("expected \"email\", got \"username\"".into()),
        );

        let persister = TransactionCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&result);

        assert_eq!(
            row,
            "1,0.5,1,4,3,150,failed,\"Extract(\"\"expected \\\"\"email\\\"\", got \\\"\"username\\\"\"\"\")\""
        );
    }

    #[ignore = "refactor"]
    #[test]
    fn persist_writes_header_followed_by_rows_incrementally() {
        let first = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(8)),
            100,
            stats_with(&[
                success_result(0, 0, 250, 1000, 100),
                failed_result(1, 1, 500, 1000, 200, FailedCode::Status(503)),
            ]),
            Some(RelativeLoadTestTime::new(Duration::from_secs(8))),
        );
        let second = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(6)),
            0,
            stats_with(&[dropped_result(
                3,
                1900,
                2000,
                DroppedCode::Error("wait timed out".into()),
            )]),
            None,
        );

        let mut persister = TransactionCsvPersister::new(Vec::new()).unwrap();
        persister.persist(&first, Phase::Load).unwrap();
        persister.persist(&second, Phase::Load).unwrap();

        let output = String::from_utf8(persister.into_inner()).unwrap();
        assert_eq!(
            output,
            "target_time,start_time,load_generator_id,virtual_user_id,spec_id,response_time_ms,outcome,reason\n\
             1,0.25,1,0,0,100,success,\n\
             1,0.5,1,1,1,200,failed,Status(503)\n\
             2,1.9,1,3,,,dropped,\"Error(\"\"wait timed out\"\")\"\n"
        );
    }
}
