//! Versioned filesystem adapter for typed speedtest history.

use infiltrator_contract::speedtest::{HistoricalSpeedtestRecord, SpeedtestRunId};
use infiltrator_platform::paths::get_home_dir;
use infiltrator_ports::speedtest_history::SpeedtestHistoryStore;
use lurker_core_ports::error::PortError;
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, read_to_string, write};
use std::path::PathBuf;

const CURRENT_SCHEMA_VERSION: u16 = 2;

pub struct FileSpeedtestHistoryStore {
    path: PathBuf,
}

impl FileSpeedtestHistoryStore {
    pub fn current() -> anyhow::Result<Self> {
        let home = get_home_dir()?;
        Ok(Self {
            path: home.join("speedtest_history.json"),
        })
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    fn write_current(&self, records: &[HistoricalSpeedtestRecord]) -> Result<(), PortError> {
        if let Some(parent) = self.path.parent() {
            create_dir_all(parent).map_err(|error| PortError::Io(error.to_string()))?;
        }
        let envelope = HistoryEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            records: records.to_vec(),
        };
        let content = serde_json::to_string_pretty(&envelope)
            .map_err(|error| PortError::Io(error.to_string()))?;
        write(&self.path, content).map_err(|error| PortError::Io(error.to_string()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryEnvelope {
    schema_version: u16,
    records: Vec<HistoricalSpeedtestRecord>,
}

#[derive(Deserialize)]
struct VersionHeader {
    schema_version: u16,
}

/// Persistence-only DTO for version 1. It is read exactly once during the
/// versioned upgrade and is never exposed to application or UI code.
#[derive(Clone, Debug, Deserialize)]
struct LegacyHistoryV1 {
    run_id: u64,
    timestamp_epoch_ms: u64,
    #[serde(default)]
    scope: Option<LegacyScopeV1>,
    #[serde(default)]
    target_url: Option<String>,
    total_nodes: usize,
    alive_nodes: usize,
    avg_latency_ms: Option<f64>,
    avg_jitter_ms: Option<f64>,
    #[serde(default)]
    avg_bandwidth_mbps: Option<f64>,
    #[serde(default)]
    overall_star_rating: Option<u8>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LegacyScopeV1 {
    AllGroups,
    SingleGroup(String),
    SingleNode(String),
}

impl TryFrom<LegacyHistoryV1> for HistoricalSpeedtestRecord {
    type Error = PortError;

    fn try_from(record: LegacyHistoryV1) -> Result<Self, Self::Error> {
        let run_id = SpeedtestRunId::new(record.run_id).map_err(|_| {
            PortError::Io("legacy speedtest history contains a zero run ID".to_owned())
        })?;
        match record.scope {
            None | Some(LegacyScopeV1::AllGroups) => {}
            Some(LegacyScopeV1::SingleGroup(name)) | Some(LegacyScopeV1::SingleNode(name)) => {
                drop(name);
            }
        }
        let _historical_probe_url = record.target_url;
        let _historical_bandwidth = record.avg_bandwidth_mbps;
        let _historical_rating = record.overall_star_rating;
        Ok(Self {
            run_id,
            timestamp_epoch_ms: record.timestamp_epoch_ms,
            target: None,
            total_targets: record.total_nodes,
            completed_targets: record.alive_nodes,
            avg_latency_ms: record.avg_latency_ms,
            avg_jitter_ms: record.avg_jitter_ms,
        })
    }
}

impl SpeedtestHistoryStore for FileSpeedtestHistoryStore {
    fn load(&self) -> Result<Vec<HistoricalSpeedtestRecord>, PortError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content =
            read_to_string(&self.path).map_err(|error| PortError::Io(error.to_string()))?;
        let value: serde_json::Value =
            serde_json::from_str(&content).map_err(|error| PortError::Io(error.to_string()))?;

        if value.is_array() {
            let legacy = serde_json::from_value::<Vec<LegacyHistoryV1>>(value)
                .map_err(|error| PortError::Io(error.to_string()))?;
            let records = legacy
                .into_iter()
                .map(HistoricalSpeedtestRecord::try_from)
                .collect::<Result<Vec<_>, _>>()?;
            self.write_current(&records)?;
            return Ok(records);
        }

        let header = serde_json::from_value::<VersionHeader>(value.clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        match header.schema_version {
            CURRENT_SCHEMA_VERSION => {
                let envelope = serde_json::from_value::<HistoryEnvelope>(value)
                    .map_err(|error| PortError::Io(error.to_string()))?;
                Ok(envelope.records)
            }
            1 => {
                let legacy = serde_json::from_value::<LegacyEnvelopeV1>(value)
                    .map_err(|error| PortError::Io(error.to_string()))?;
                let _legacy_schema_version = legacy.schema_version;
                let records = legacy
                    .records
                    .into_iter()
                    .map(HistoricalSpeedtestRecord::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                self.write_current(&records)?;
                Ok(records)
            }
            version => Err(PortError::Io(format!(
                "unsupported speedtest history schema version {version}"
            ))),
        }
    }

    fn save(&self, records: &[HistoricalSpeedtestRecord]) -> Result<(), PortError> {
        self.write_current(records)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyEnvelopeV1 {
    schema_version: u16,
    records: Vec<LegacyHistoryV1>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs::{create_dir_all, read_to_string, remove_dir_all, remove_file, write};
    use std::process::id;

    fn sample_records() -> Vec<HistoricalSpeedtestRecord> {
        vec![HistoricalSpeedtestRecord {
            run_id: SpeedtestRunId::new(7).expect("valid fixture run ID"),
            timestamp_epoch_ms: 1_700_000_000_000,
            target: None,
            total_targets: 4,
            completed_targets: 3,
            avg_latency_ms: Some(42.5),
            avg_jitter_ms: Some(1.5),
        }]
    }

    #[test]
    fn current_history_round_trips_in_a_versioned_envelope() {
        let dir = temp_dir().join(format!("mf-speedtest-history-{}", id()));
        let store = FileSpeedtestHistoryStore::at(dir.join("history.json"));
        assert!(store.load().unwrap().is_empty());
        store.save(&sample_records()).unwrap();
        assert_eq!(store.load().unwrap(), sample_records());
        let content = read_to_string(dir.join("history.json")).unwrap();
        let envelope: HistoryEnvelope = serde_json::from_str(&content).unwrap();
        assert_eq!(envelope.schema_version, CURRENT_SCHEMA_VERSION);
        let _ = remove_dir_all(&dir);
    }

    #[test]
    fn legacy_history_migrates_once_and_discards_unrecoverable_name_identity() {
        let dir = temp_dir().join(format!("mf-speedtest-history-v1-{}", id()));
        let path = dir.join("history.json");
        create_dir_all(&dir).unwrap();
        write(
            &path,
            r#"[{"run_id":7,"timestamp_epoch_ms":1700000000000,"scope":{"single_group":"HongKong"},"target_url":"https://probe.example.test","total_nodes":4,"alive_nodes":3,"avg_latency_ms":42.5,"avg_jitter_ms":1.5,"avg_bandwidth_mbps":120.0,"overall_star_rating":4}]"#,
        )
        .unwrap();
        let store = FileSpeedtestHistoryStore::at(path.clone());
        let migrated = store.load().unwrap();
        assert_eq!(migrated, sample_records());
        let persisted = read_to_string(&path).unwrap();
        assert!(persisted.contains("\"schema_version\": 2"));
        assert_eq!(store.load().unwrap(), migrated);
        let _ = remove_dir_all(&dir);
    }

    #[test]
    fn unsupported_history_version_fails_closed() {
        let path = temp_dir().join(format!("mf-speedtest-history-vx-{}.json", id()));
        write(&path, r#"{"schema_version":99,"records":[]}"#).unwrap();
        let store = FileSpeedtestHistoryStore::at(path.clone());
        assert!(matches!(store.load(), Err(PortError::Io(_))));
        let _ = remove_file(path);
    }
}
