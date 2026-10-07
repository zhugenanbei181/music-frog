//! Filesystem adapter for the durable speedtest run history.
//!
//! The bounded history lives under the host home directory as JSON, so the
//! shared engine restores it on the next process start. The desktop host owns
//! the path; the application layer only sees the `SpeedtestHistoryStore` port.

use infiltrator_contract::speedtest::HistoricalSpeedtestRecord;
use infiltrator_ports::error::PortError;
use infiltrator_ports::speedtest_history::SpeedtestHistoryStore;
use mihomo_platform::paths::get_home_dir;
use std::fs::{create_dir_all, read_to_string, write};
use std::path::PathBuf;

pub struct FileSpeedtestHistoryStore {
    path: PathBuf,
}

impl FileSpeedtestHistoryStore {
    /// Store under the current host home directory.
    pub fn current() -> anyhow::Result<Self> {
        let home = get_home_dir()?;
        Ok(Self {
            path: home.join("speedtest_history.json"),
        })
    }

    /// Store at an explicit path (used by host tests).
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SpeedtestHistoryStore for FileSpeedtestHistoryStore {
    fn load(&self) -> Result<Vec<HistoricalSpeedtestRecord>, PortError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content =
            read_to_string(&self.path).map_err(|error| PortError::Io(error.to_string()))?;
        serde_json::from_str(&content).map_err(|error| PortError::Io(error.to_string()))
    }

    fn save(&self, records: &[HistoricalSpeedtestRecord]) -> Result<(), PortError> {
        if let Some(parent) = self.path.parent() {
            create_dir_all(parent).map_err(|error| PortError::Io(error.to_string()))?;
        }
        let content = serde_json::to_string_pretty(records)
            .map_err(|error| PortError::Io(error.to_string()))?;
        write(&self.path, content).map_err(|error| PortError::Io(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::speedtest::SpeedtestScope;
    #[cfg(test)]
    use std::env::temp_dir;
    #[cfg(test)]
    use std::fs::remove_dir_all;
    #[cfg(test)]
    use std::fs::remove_file;
    #[cfg(test)]
    use std::process::id;

    fn sample_records() -> Vec<HistoricalSpeedtestRecord> {
        vec![HistoricalSpeedtestRecord {
            run_id: 7,
            timestamp_epoch_ms: 1_700_000_000_000,
            scope: SpeedtestScope::AllGroups,
            target_url: "http://example.local".to_string(),
            total_nodes: 4,
            alive_nodes: 3,
            avg_latency_ms: Some(42.5),
            avg_jitter_ms: Some(1.5),
            avg_bandwidth_mbps: Some(120.0),
            overall_star_rating: 4,
        }]
    }

    #[test]
    fn file_store_round_trips_records() {
        let dir = temp_dir().join(format!("mf-speedtest-history-{}", id()));
        let store = FileSpeedtestHistoryStore::at(dir.join("history.json"));
        assert!(store.load().unwrap().is_empty());
        store.save(&sample_records()).unwrap();
        assert_eq!(store.load().unwrap(), sample_records());
        let _ = remove_dir_all(&dir);
    }

    #[test]
    fn file_store_missing_file_is_empty() {
        let path = temp_dir().join(format!("mf-speedtest-history-missing-{}.json", id()));
        let _ = remove_file(&path);
        let store = FileSpeedtestHistoryStore::at(path);
        assert!(store.load().unwrap().is_empty());
    }
}
