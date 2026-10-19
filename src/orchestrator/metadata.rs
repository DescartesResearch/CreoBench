use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::summary::DropReport;
use crate::wire::configure::GeneratorInfo;

pub const FILE_NAME: &str = "run.json";

/// Metadata of a load test run, written to [`FILE_NAME`] in the output directory.
///
/// The input files themselves are copied next to it, so together they
/// describe how the run was configured, where it ran, and whether it is valid.
#[derive(Debug, serde::Serialize)]
pub struct RunMetadata {
    /// The CreoBench version of the orchestrator.
    pub version: &'static str,
    pub started_at: String,
    pub finished_at: String,
    /// Whether no load-phase transaction was dropped.
    pub valid: bool,
    pub virtual_user_count: u32,
    /// The HTTP request timeout, or `None` if requests never time out.
    pub timeout_ms: Option<u64>,
    pub seed: u64,
    pub generators: Vec<GeneratorMetadata>,
    /// The dropped transactions, only present if any were dropped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drops: Option<DropReport>,
}

/// A load generator instance of a run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GeneratorMetadata {
    pub id: u8,
    #[serde(flatten)]
    pub info: GeneratorInfo,
}

/// The current time in RFC 3339 format, in UTC.
pub fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("formatting a UTC time as RFC 3339 does not fail")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::summary::PhaseDrops;

    fn metadata(drops: Option<DropReport>) -> RunMetadata {
        RunMetadata {
            version: "0.1.0",
            started_at: "2026-10-19T18:00:00Z".into(),
            finished_at: "2026-10-19T18:01:15Z".into(),
            valid: drops.is_none(),
            virtual_user_count: 100,
            timeout_ms: None,
            seed: 5,
            generators: vec![GeneratorMetadata {
                id: 0,
                info: GeneratorInfo::test_info(),
            }],
            drops,
        }
    }

    #[test]
    fn serializes_generator_info_inline() {
        let json = serde_json::to_value(metadata(None)).unwrap();

        assert_eq!(json["generators"][0]["id"], 0);
        assert_eq!(json["generators"][0]["worker_threads"], 7);
        assert_eq!(json["timeout_ms"], serde_json::Value::Null);
    }

    #[test]
    fn omits_drops_without_drops() {
        let json = serde_json::to_value(metadata(None)).unwrap();

        assert_eq!(json["valid"], true);
        assert!(json.get("drops").is_none(), "{json}");
    }

    #[test]
    fn includes_drops_if_any() {
        let drops = DropReport {
            load: PhaseDrops {
                transactions: 10,
                late: 1,
                ..PhaseDrops::default()
            },
            warmup: PhaseDrops::default(),
        };

        let json = serde_json::to_value(metadata(Some(drops))).unwrap();

        assert_eq!(json["valid"], false);
        assert_eq!(json["drops"]["load"]["late"], 1);
        assert_eq!(json["drops"]["warmup"]["transactions"], 0);
    }

    #[test]
    fn formats_now_as_rfc3339() {
        let now = now();

        // For example `2026-10-19T18:00:00.123456Z`.
        assert_eq!(now.as_bytes()[10], b'T', "{now}");
        assert!(now.ends_with('Z'), "{now}");
    }
}
