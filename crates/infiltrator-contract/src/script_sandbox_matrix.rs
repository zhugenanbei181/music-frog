//! DUAL-10-15: the shared scripting-sandbox regression matrix contract.
//!
//! Group 10 closes with one deterministic matrix both surfaces assert on. Like
//! the group 05/11 matrices, it is **honest about coverage**: an item without
//! a real, runnable backend row (the QuickJS engine claim, the mirrored
//! console viewport, the three-column editor) is registered as `covered: false`
//! with the real reason, and the aggregate API only claims what actually ran.
//!
//! The execution lives in
//! `infiltrator_application::script_sandbox_matrix_application`; every covered
//! row runs the real domain engine / application service, never a hard-coded
//! boolean.

use serde::{Deserialize, Serialize};

/// One item's row in the scripting-sandbox regression matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptSandboxMatrixScenario {
    /// `DUAL-10-01` … `DUAL-10-15`.
    pub id: String,
    /// What the scenario exercises, in one line.
    pub item: String,
    /// `false` for items with no runnable backend; those carry a real reason.
    pub covered: bool,
    /// Only meaningful when `covered`; always computed by running real code.
    pub passed: bool,
    pub detail: String,
}

/// The whole matrix, as executed by the shared application.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptSandboxMatrixReport {
    pub scenarios: Vec<ScriptSandboxMatrixScenario>,
}

impl ScriptSandboxMatrixReport {
    pub fn covered_count(&self) -> usize {
        self.scenarios.iter().filter(|row| row.covered).count()
    }

    pub fn covered_passed_count(&self) -> usize {
        self.scenarios
            .iter()
            .filter(|row| row.covered && row.passed)
            .count()
    }

    pub fn not_covered_ids(&self) -> Vec<&str> {
        self.scenarios
            .iter()
            .filter(|row| !row.covered)
            .map(|row| row.id.as_str())
            .collect()
    }

    pub fn failed_ids(&self) -> Vec<&str> {
        self.scenarios
            .iter()
            .filter(|row| row.covered && !row.passed)
            .map(|row| row.id.as_str())
            .collect()
    }

    /// `true` only when every covered item passed; planned items never make
    /// this `false` (they are honest gaps, not regressions).
    pub fn all_covered_passed(&self) -> bool {
        self.failed_ids().is_empty()
    }

    pub fn summary_zh(&self) -> String {
        format!(
            "脚本沙箱矩阵 {}/{} 项通过 · {} 项未覆盖",
            self.covered_passed_count(),
            self.covered_count(),
            self.not_covered_ids().len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, covered: bool, passed: bool) -> ScriptSandboxMatrixScenario {
        ScriptSandboxMatrixScenario {
            id: id.to_string(),
            item: "test".to_string(),
            covered,
            passed,
            detail: String::new(),
        }
    }

    #[test]
    fn aggregate_only_claims_covered_rows() {
        let report = ScriptSandboxMatrixReport {
            scenarios: vec![
                row("DUAL-10-01", false, false),
                row("DUAL-10-03", true, true),
            ],
        };
        assert_eq!(report.covered_count(), 1);
        assert_eq!(report.covered_passed_count(), 1);
        assert_eq!(report.not_covered_ids(), vec!["DUAL-10-01"]);
        assert!(report.all_covered_passed());
        assert!(report.summary_zh().contains("1/1"));
    }

    #[test]
    fn a_failed_covered_row_fails_the_matrix() {
        let report = ScriptSandboxMatrixReport {
            scenarios: vec![row("DUAL-10-03", true, false)],
        };
        assert!(!report.all_covered_passed());
        assert_eq!(report.failed_ids(), vec!["DUAL-10-03"]);
    }
}
