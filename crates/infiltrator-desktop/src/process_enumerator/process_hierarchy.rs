use super::ExtendedProcessInfo;
use std::collections::{HashMap, HashSet};

/// Hierarchical process tree organizer that groups child processes under parent applications.
#[derive(Debug, Clone, Default)]
pub struct ProcessHierarchyTree {
    roots: Vec<ExtendedProcessInfo>,
    by_pid: HashMap<u32, ExtendedProcessInfo>,
}

impl ProcessHierarchyTree {
    /// Builds a process tree from a flat list of process items.
    pub fn from_processes(processes: Vec<ExtendedProcessInfo>) -> Self {
        let mut by_pid: HashMap<u32, ExtendedProcessInfo> = HashMap::new();
        let mut parent_to_children: HashMap<u32, Vec<u32>> = HashMap::new();

        for proc in &processes {
            by_pid.insert(proc.pid, proc.clone());
            if let Some(ppid) = proc.ppid {
                parent_to_children.entry(ppid).or_default().push(proc.pid);
            }
        }

        // Aggregate children and memory into parents
        for (pid, children) in &parent_to_children {
            let child_mem: u64 = children
                .iter()
                .filter_map(|c_pid| by_pid.get(c_pid).map(|c| c.memory_bytes))
                .sum();
            if let Some(parent) = by_pid.get_mut(pid) {
                parent.child_pids = children.clone();
                parent.total_memory_bytes = parent.memory_bytes.saturating_add(child_mem);
            }
        }

        let pids: HashSet<u32> = by_pid.keys().copied().collect();
        let mut roots = Vec::new();

        for proc in by_pid.values() {
            // A node is considered a root if it has no PPID or its PPID is not present in the process list
            let is_root = match proc.ppid {
                None => true,
                Some(ppid) => !pids.contains(&ppid) || ppid == 0 || ppid == 1,
            };
            if is_root {
                roots.push(proc.clone());
            }
        }

        roots.sort_by(|a, b| {
            a.is_system
                .cmp(&b.is_system)
                .then_with(|| b.total_memory_bytes.cmp(&a.total_memory_bytes))
                .then_with(|| a.display_name.cmp(&b.display_name))
        });

        Self { roots, by_pid }
    }

    /// Returns the root applications in the hierarchy.
    pub fn roots(&self) -> &[ExtendedProcessInfo] {
        &self.roots
    }

    /// Looks up a specific process by PID.
    pub fn get_by_pid(&self, pid: u32) -> Option<&ExtendedProcessInfo> {
        self.by_pid.get(&pid)
    }

    /// Returns the total number of tracked processes.
    pub fn total_process_count(&self) -> usize {
        self.by_pid.len()
    }

    /// Flattens the tree into user applications (excluding system processes).
    pub fn user_applications(&self) -> Vec<ExtendedProcessInfo> {
        self.roots
            .iter()
            .filter(|p| !p.is_system)
            .cloned()
            .collect()
    }
}
