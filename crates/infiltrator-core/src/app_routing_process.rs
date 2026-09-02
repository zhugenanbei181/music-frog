//! Thread-safe per-process traffic accounting.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{ProcessAliasRegistry, ProcessTrafficSnapshot, ProcessUsageTracker};

impl ProcessTrafficSnapshot {
    pub fn total_bytes(&self) -> u64 {
        self.upload_bytes.saturating_add(self.download_bytes)
    }
}

impl Default for ProcessUsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessUsageTracker {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
            registry: ProcessAliasRegistry::default(),
            auto_canonicalize: true,
        }
    }

    pub fn with_alias_registry(mut self, registry: ProcessAliasRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn with_auto_canonicalize(mut self, enabled: bool) -> Self {
        self.auto_canonicalize = enabled;
        self
    }

    fn resolve_name(&self, raw_name: &str) -> String {
        if self.auto_canonicalize {
            self.registry.canonicalize(raw_name)
        } else {
            raw_name.trim().to_string()
        }
    }

    fn current_epoch_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn record_process_traffic(&self, process_name: &str, up: u64, down: u64) {
        let name = self.resolve_name(process_name);
        if name.is_empty() {
            return;
        }
        let now = Self::current_epoch_secs();

        if let Ok(mut guard) = self.state.write() {
            let entry = guard.entry(name).or_default();
            entry.upload_bytes = entry.upload_bytes.saturating_add(up);
            entry.download_bytes = entry.download_bytes.saturating_add(down);
            entry.last_active_epoch_secs = now;
        }
    }

    pub fn record_connection(&self, process_name: &str) {
        let name = self.resolve_name(process_name);
        if name.is_empty() {
            return;
        }
        let now = Self::current_epoch_secs();

        if let Ok(mut guard) = self.state.write() {
            let entry = guard.entry(name).or_default();
            entry.connection_count = entry.connection_count.saturating_add(1);
            entry.active_connections = entry.active_connections.saturating_add(1);
            entry.last_active_epoch_secs = now;
        }
    }

    pub fn close_connection(&self, process_name: &str) {
        let name = self.resolve_name(process_name);
        if name.is_empty() {
            return;
        }

        if let Ok(mut guard) = self.state.write()
            && let Some(entry) = guard.get_mut(&name)
        {
            entry.active_connections = entry.active_connections.saturating_sub(1);
        }
    }

    pub fn record_flow(&self, process_name: &str, up: u64, down: u64, connections: u64) {
        let name = self.resolve_name(process_name);
        if name.is_empty() {
            return;
        }
        let now = Self::current_epoch_secs();

        if let Ok(mut guard) = self.state.write() {
            let entry = guard.entry(name).or_default();
            entry.upload_bytes = entry.upload_bytes.saturating_add(up);
            entry.download_bytes = entry.download_bytes.saturating_add(down);
            entry.connection_count = entry.connection_count.saturating_add(connections);
            entry.last_active_epoch_secs = now;
        }
    }

    pub fn get_process(&self, process_name: &str) -> Option<ProcessTrafficSnapshot> {
        let name = self.resolve_name(process_name);
        let guard = self.state.read().ok()?;
        let metrics = guard.get(&name)?;
        Some(ProcessTrafficSnapshot {
            process_name: name,
            upload_bytes: metrics.upload_bytes,
            download_bytes: metrics.download_bytes,
            total_bytes: metrics.upload_bytes.saturating_add(metrics.download_bytes),
            connection_count: metrics.connection_count,
            active_connections: metrics.active_connections,
            last_active_epoch_secs: metrics.last_active_epoch_secs,
        })
    }

    pub fn get_top_processes(&self, limit: usize) -> Vec<ProcessTrafficSnapshot> {
        if limit == 0 {
            return Vec::new();
        }

        let mut snapshots = self.get_all_processes();
        snapshots.sort_by(|a, b| {
            b.total_bytes
                .cmp(&a.total_bytes)
                .then_with(|| b.connection_count.cmp(&a.connection_count))
                .then_with(|| a.process_name.cmp(&b.process_name))
        });

        if snapshots.len() > limit {
            snapshots.truncate(limit);
        }
        snapshots
    }

    pub fn get_all_processes(&self) -> Vec<ProcessTrafficSnapshot> {
        let Ok(guard) = self.state.read() else {
            return Vec::new();
        };

        guard
            .iter()
            .map(|(name, metrics)| ProcessTrafficSnapshot {
                process_name: name.clone(),
                upload_bytes: metrics.upload_bytes,
                download_bytes: metrics.download_bytes,
                total_bytes: metrics.upload_bytes.saturating_add(metrics.download_bytes),
                connection_count: metrics.connection_count,
                active_connections: metrics.active_connections,
                last_active_epoch_secs: metrics.last_active_epoch_secs,
            })
            .collect()
    }

    pub fn total_traffic(&self) -> (u64, u64) {
        let Ok(guard) = self.state.read() else {
            return (0, 0);
        };
        let mut total_up = 0u64;
        let mut total_down = 0u64;
        for metrics in guard.values() {
            total_up = total_up.saturating_add(metrics.upload_bytes);
            total_down = total_down.saturating_add(metrics.download_bytes);
        }
        (total_up, total_down)
    }

    pub fn len(&self) -> usize {
        self.state.read().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.state.write() {
            guard.clear();
        }
    }

    pub fn reset_process(&self, process_name: &str) -> bool {
        let name = self.resolve_name(process_name);
        if let Ok(mut guard) = self.state.write() {
            guard.remove(&name).is_some()
        } else {
            false
        }
    }
}
