use super::{ExtendedProcessInfo, ProcessCategory};

/// Filter criteria for querying and searching active processes.
#[derive(Debug, Clone, Default)]
pub struct ProcessFilter {
    pub query: Option<String>,
    pub category: Option<ProcessCategory>,
    pub exclude_system: bool,
}

impl ProcessFilter {
    /// Evaluates if an extended process matches the active filter criteria.
    pub fn matches(&self, proc: &ExtendedProcessInfo) -> bool {
        if self.exclude_system && proc.is_system {
            return false;
        }

        if let Some(cat) = self.category {
            if proc.category != cat {
                return false;
            }
        }

        if let Some(ref q) = self.query {
            let needle = q.trim().to_ascii_lowercase();
            if !needle.is_empty() {
                let match_name = proc.name.to_ascii_lowercase().contains(&needle);
                let match_display = proc.display_name.to_ascii_lowercase().contains(&needle);
                let match_canonical = proc.canonical_name.to_ascii_lowercase().contains(&needle);
                let match_path = proc
                    .binary_path
                    .as_ref()
                    .map(|p| p.to_ascii_lowercase().contains(&needle))
                    .unwrap_or(false);

                if !match_name && !match_display && !match_canonical && !match_path {
                    return false;
                }
            }
        }

        true
    }

    /// Filters a list of processes based on these criteria.
    pub fn filter(&self, processes: &[ExtendedProcessInfo]) -> Vec<ExtendedProcessInfo> {
        processes
            .iter()
            .filter(|p| self.matches(p))
            .cloned()
            .collect()
    }
}
