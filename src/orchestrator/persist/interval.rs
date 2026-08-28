use std::io::{BufWriter, Write};

use crate::tracker::IntervalReport;

use super::csv::escape_field;
use super::{PersistError, Persister, Phase};

pub const FILE_NAME: &str = "interval.csv";
const HEADER: &str = "target_time,load_level,successful_transactions,failed_transactions,timeout_transactions,dropped_transactions,avg_service_time,final_batch_time,phase";

pub struct IntervalCsvPersister<W: Write> {
    writer: BufWriter<W>,
}

impl<W: Write> IntervalCsvPersister<W> {
    pub fn new(writer: W) -> Result<Self, PersistError> {
        let mut writer = BufWriter::new(writer);
        writeln!(writer, "{HEADER}").map_err(io_err)?;
        Ok(Self { writer })
    }

    pub fn format_row(&self, report: &IntervalReport, phase: Phase) -> String {
        let stats = &report.stats;
        let final_batch_time = report
            .final_batch_time
            .map(|t| t.as_secs_f64().to_string())
            .unwrap_or_default();
        let phase = match phase {
            Phase::Warmup => "warmup",
            Phase::Pause => "pause",
            Phase::Load => "load",
        };
        let fields = [
            report.target_time.as_secs_f64().to_string(),
            report.load_level.to_string(),
            stats.successful_transactions.to_string(),
            stats.failed_transactions.to_string(),
            stats.timeout_transactions.to_string(),
            stats.dropped_transactions.to_string(),
            stats.average_response_time().to_string(),
            final_batch_time,
            phase.to_string(),
        ];
        fields
            .into_iter()
            .map(|field| escape_field(&field))
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
impl<W: Write + std::fmt::Debug> IntervalCsvPersister<W> {
    fn into_inner(self) -> W {
        self.writer
            .into_inner()
            .expect("in-memory interval writer must flush")
    }
}

impl<W: Write + Send> Persister for IntervalCsvPersister<W> {
    fn persist(&mut self, report: &IntervalReport, phase: Phase) -> Result<(), PersistError> {
        writeln!(self.writer, "{}", self.format_row(report, phase)).map_err(io_err)
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
    use crate::orchestrator::persist::test_utils::stats_with_measured_transactions;
    use crate::tracker::{IntervalReport, IntervalStatistics};

    #[test]
    fn construction_writes_header() {
        let persister = IntervalCsvPersister::new(Vec::new()).unwrap();
        let output = String::from_utf8(persister.into_inner()).unwrap();
        assert_eq!(
            output,
            "target_time,load_level,successful_transactions,failed_transactions,timeout_transactions,dropped_transactions,avg_service_time,final_batch_time,phase\n"
        );
    }

    #[test]
    fn pause_round_with_empty_statistics_renders_empty_final_batch_time() {
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(6)),
            0,
            IntervalStatistics::default(),
            None,
        );

        let persister = IntervalCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&report, Phase::Pause);

        assert_eq!(row, "6,0,0,0,0,0,0,,pause");
    }

    #[test]
    fn load_round_with_traffic_renders_counts_and_final_batch_time() {
        let stats = stats_with_measured_transactions(&[100, 100], &[100]);
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(8)),
            100,
            stats,
            Some(RelativeLoadTestTime::new(Duration::from_millis(8250))),
        );

        let persister = IntervalCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&report, Phase::Load);

        assert_eq!(row, "8,100,2,1,0,0,100,8.25,load");
    }

    #[test]
    fn warmup_round_renders_raw_load_test_relative_target_time() {
        let stats = stats_with_measured_transactions(&[100, 100, 100], &[]);
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(1)),
            10,
            stats,
            Some(RelativeLoadTestTime::new(Duration::from_secs(1))),
        );

        let persister = IntervalCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&report, Phase::Warmup);

        assert_eq!(row, "1,10,3,0,0,0,100,1,warmup");
    }

    #[test]
    fn dropped_only_round_renders_zero_average_response_time() {
        let mut stats = IntervalStatistics::default();
        stats.dropped_transactions = 4;
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(8)),
            100,
            stats,
            Some(RelativeLoadTestTime::new(Duration::from_secs(8))),
        );

        let persister = IntervalCsvPersister::new(Vec::new()).unwrap();
        let row = persister.format_row(&report, Phase::Load);

        assert_eq!(row, "8,100,0,0,0,4,0,8,load");
    }

    #[test]
    fn persist_writes_header_followed_by_rows() {
        let stats = stats_with_measured_transactions(&[100, 100], &[]);
        let report = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(8)),
            100,
            stats,
            Some(RelativeLoadTestTime::new(Duration::from_secs(8))),
        );
        let pause = IntervalReport::new(
            RelativeLoadTestTime::new(Duration::from_secs(6)),
            0,
            IntervalStatistics::default(),
            None,
        );

        let mut persister = IntervalCsvPersister::new(Vec::new()).unwrap();
        persister.persist(&report, Phase::Load).unwrap();
        persister.persist(&pause, Phase::Pause).unwrap();

        let output = String::from_utf8(persister.into_inner()).unwrap();
        assert_eq!(
            output,
            "target_time,load_level,successful_transactions,failed_transactions,timeout_transactions,dropped_transactions,avg_service_time,final_batch_time,phase\n\
             8,100,2,0,0,0,100,8,load\n\
             6,0,0,0,0,0,0,,pause\n"
        );
    }
}
