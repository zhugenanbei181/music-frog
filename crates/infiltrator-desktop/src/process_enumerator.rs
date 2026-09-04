//! Process enumeration and application identification for per-app routing and split tunneling.

pub mod desktop_entries;
pub mod process_classification;
pub mod process_filter;
pub mod process_hierarchy;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use sysinfo::{ProcessStatus, ProcessesToUpdate, System};

pub type DesktopEntry = desktop_entries::DesktopEntry;
pub type DesktopEntryScanner = desktop_entries::DesktopEntryScanner;
pub type ProcessFilter = process_filter::ProcessFilter;
pub type ProcessHierarchyTree = process_hierarchy::ProcessHierarchyTree;

/// Classification category for active processes and desktop applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProcessCategory {
    #[default]
    Other,
    Browser,
    Developer,
    Communication,
    Media,
    Gaming,
    Office,
    NetworkVpn,
    SystemDaemon,
}

impl fmt::Display for ProcessCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Browser => write!(f, "Web Browser"),
            Self::Developer => write!(f, "Developer Tools"),
            Self::Communication => write!(f, "Instant Messaging & Social"),
            Self::Media => write!(f, "Audio, Video & Streaming"),
            Self::Gaming => write!(f, "Gaming & Platforms"),
            Self::Office => write!(f, "Office & Productivity"),
            Self::NetworkVpn => write!(f, "VPN & Network Utilities"),
            Self::SystemDaemon => write!(f, "System Daemon & Service"),
            Self::Other => write!(f, "Application"),
        }
    }
}

/// Information about an active system or user process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub binary_path: Option<String>,
    pub is_system: bool,
    pub icon_hint: Option<String>,
}

/// Extended process metadata including hierarchy, memory, and category information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtendedProcessInfo {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
    pub display_name: String,
    pub canonical_name: String,
    pub binary_path: Option<String>,
    pub is_system: bool,
    pub category: ProcessCategory,
    pub icon_hint: Option<String>,
    pub memory_bytes: u64,
    pub total_memory_bytes: u64,
    pub child_pids: Vec<u32>,
}

impl From<ProcessInfo> for ExtendedProcessInfo {
    fn from(info: ProcessInfo) -> Self {
        let display_name = ProcessEnumerator::normalize_display_name(&info.name);
        let canonical_name =
            infiltrator_domain::app_routing::ProcessAliasRegistry::canonicalize_name(&info.name);
        let category =
            classify_process_category(&info.name, info.binary_path.as_deref(), info.is_system);

        Self {
            pid: info.pid,
            ppid: None,
            name: info.name,
            display_name,
            canonical_name,
            binary_path: info.binary_path,
            is_system: info.is_system,
            category,
            icon_hint: info.icon_hint,
            memory_bytes: 0,
            total_memory_bytes: 0,
            child_pids: Vec::new(),
        }
    }
}

/// Legacy process representation for backwards compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessItem {
    pub pid: u32,
    pub name: String,
    pub display_name: String,
    pub exe_path: Option<PathBuf>,
}

impl From<ProcessInfo> for ProcessItem {
    fn from(info: ProcessInfo) -> Self {
        let display_name = ProcessEnumerator::normalize_display_name(&info.name);
        Self {
            pid: info.pid,
            name: info.name,
            display_name,
            exe_path: info.binary_path.map(PathBuf::from),
        }
    }
}

impl From<ProcessItem> for ProcessInfo {
    fn from(item: ProcessItem) -> Self {
        let binary_path = item.exe_path.map(|p| p.to_string_lossy().to_string());
        let is_system =
            ProcessEnumerator::is_system_process(&item.name, binary_path.as_deref(), item.pid);
        let icon_hint = ProcessEnumerator::resolve_icon_hint(&item.name, binary_path.as_deref());
        Self {
            pid: item.pid,
            name: item.name,
            binary_path,
            is_system,
            icon_hint,
        }
    }
}

/// Classifies a process into semantic categories for grouping in UI.
pub fn classify_process_category(
    name: &str,
    binary_path: Option<&str>,
    is_system: bool,
) -> ProcessCategory {
    process_classification::classify_process_category(name, binary_path, is_system)
}

/// Identifies whether a process is a background system daemon, kernel thread, or OS service.
pub fn is_system_process(name: &str, binary_path: Option<&str>, pid: u32) -> bool {
    process_classification::is_system_process(name, binary_path, pid)
}

/// Determines if an executable or process name belongs to a recognized desktop user application.
pub fn is_known_user_app(name: &str) -> bool {
    process_classification::is_known_user_app(name)
}

/// Resolves an appropriate icon hint identifier for UI presentation.
pub fn resolve_icon_hint(name: &str, binary_path: Option<&str>) -> Option<String> {
    process_classification::resolve_icon_hint(name, binary_path)
}

/// Normalizes raw executable names to friendly user-facing labels.
pub fn normalize_display_name(raw_name: &str) -> String {
    process_classification::normalize_display_name(raw_name)
}

/// Deduplicates active processes by canonical application name.
///
/// When multiple child / worker processes exist (e.g. browser renderers or helpers),
/// selects the primary process (preferring the lowest PID with a valid binary path).
pub fn deduplicate_processes(processes: Vec<ProcessInfo>) -> Vec<ProcessInfo> {
    let mut map: HashMap<String, ProcessInfo> = HashMap::new();

    for proc in processes {
        let key = proc.name.to_ascii_lowercase();
        match map.get_mut(&key) {
            Some(existing) => {
                if existing.binary_path.is_none() && proc.binary_path.is_some() {
                    existing.binary_path = proc.binary_path.clone();
                }
                if existing.icon_hint.is_none() && proc.icon_hint.is_some() {
                    existing.icon_hint = proc.icon_hint.clone();
                }
                if proc.pid < existing.pid && proc.pid != 0 {
                    existing.pid = proc.pid;
                }
                if existing.is_system && !proc.is_system {
                    existing.is_system = false;
                }
            }
            None => {
                map.insert(key, proc);
            }
        }
    }

    let mut result: Vec<ProcessInfo> = map.into_values().collect();
    result.sort_by(|a, b| {
        a.is_system
            .cmp(&b.is_system)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    result
}

/// Enumerates all currently active processes on the system, identifying system vs user processes.
pub fn enumerate_active_processes() -> Result<Vec<ProcessInfo>> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut raw_list = Vec::new();

    for (pid, process) in sys.processes() {
        let status = process.status();
        if status == ProcessStatus::Dead || status == ProcessStatus::Zombie {
            continue;
        }

        let name = process.name().to_string_lossy().to_string();
        if name.is_empty() {
            continue;
        }

        let pid_u32 = pid.as_u32();
        let binary_path = process.exe().map(|p| p.to_string_lossy().to_string());
        let is_system = is_system_process(&name, binary_path.as_deref(), pid_u32);
        let icon_hint = resolve_icon_hint(&name, binary_path.as_deref());

        raw_list.push(ProcessInfo {
            pid: pid_u32,
            name,
            binary_path,
            is_system,
            icon_hint,
        });
    }

    Ok(deduplicate_processes(raw_list))
}

/// Enumerates extended process information including memory and PPID.
pub fn enumerate_extended_processes() -> Result<Vec<ExtendedProcessInfo>> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut raw_list = Vec::new();

    for (pid, process) in sys.processes() {
        let status = process.status();
        if status == ProcessStatus::Dead || status == ProcessStatus::Zombie {
            continue;
        }

        let name = process.name().to_string_lossy().to_string();
        if name.is_empty() {
            continue;
        }

        let pid_u32 = pid.as_u32();
        let ppid = process.parent().map(|p| p.as_u32());
        let binary_path = process.exe().map(|p| p.to_string_lossy().to_string());
        let is_system = is_system_process(&name, binary_path.as_deref(), pid_u32);
        let icon_hint = resolve_icon_hint(&name, binary_path.as_deref());
        let display_name = normalize_display_name(&name);
        let canonical_name =
            infiltrator_domain::app_routing::ProcessAliasRegistry::canonicalize_name(&name);
        let category = classify_process_category(&name, binary_path.as_deref(), is_system);
        let memory_bytes = process.memory();

        raw_list.push(ExtendedProcessInfo {
            pid: pid_u32,
            ppid,
            name,
            display_name,
            canonical_name,
            binary_path,
            is_system,
            category,
            icon_hint,
            memory_bytes,
            total_memory_bytes: memory_bytes,
            child_pids: Vec::new(),
        });
    }

    Ok(raw_list)
}

/// Enumerates only active user-facing applications (excluding system daemons and kernel threads).
pub fn enumerate_user_applications() -> Result<Vec<ProcessInfo>> {
    let all = enumerate_active_processes()?;
    Ok(all.into_iter().filter(|p| !p.is_system).collect())
}

pub struct ProcessEnumerator;

impl ProcessEnumerator {
    /// Enumerates all active processes on the host.
    pub fn enumerate_active_processes() -> Result<Vec<ProcessInfo>> {
        enumerate_active_processes()
    }

    /// Enumerates extended processes with hierarchy metadata.
    pub fn enumerate_extended_processes() -> Result<Vec<ExtendedProcessInfo>> {
        enumerate_extended_processes()
    }

    /// Enumerates active user applications.
    pub fn enumerate_user_applications() -> Result<Vec<ProcessInfo>> {
        enumerate_user_applications()
    }

    /// Backwards compatible method returning `ProcessItem` list.
    pub fn enumerate_running_processes() -> Vec<ProcessItem> {
        enumerate_active_processes()
            .unwrap_or_default()
            .into_iter()
            .map(ProcessItem::from)
            .collect()
    }

    /// Normalizes raw executable name.
    pub fn normalize_display_name(raw_name: &str) -> String {
        normalize_display_name(raw_name)
    }

    /// Checks if a process is a system daemon.
    pub fn is_system_process(name: &str, binary_path: Option<&str>, pid: u32) -> bool {
        is_system_process(name, binary_path, pid)
    }

    /// Resolves an icon hint.
    pub fn resolve_icon_hint(name: &str, binary_path: Option<&str>) -> Option<String> {
        resolve_icon_hint(name, binary_path)
    }

    /// Classifies process category.
    pub fn classify_process_category(
        name: &str,
        binary_path: Option<&str>,
        is_system: bool,
    ) -> ProcessCategory {
        classify_process_category(name, binary_path, is_system)
    }

    /// Deduplicates process items.
    pub fn deduplicate_processes(processes: Vec<ProcessInfo>) -> Vec<ProcessInfo> {
        deduplicate_processes(processes)
    }
}

#[cfg(test)]
#[path = "process_enumerator_test.rs"]
mod process_enumerator_test;
