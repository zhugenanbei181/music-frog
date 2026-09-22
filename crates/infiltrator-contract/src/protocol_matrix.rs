//! DUAL-05-15: the shared protocol-codec regression matrix contract.
//!
//! Groups 06/12 close with a single deterministic matrix the two surfaces both
//! assert on; group 05 uses the same shape here. The matrix is **honest about
//! coverage**: a `planned` item (05-09 dialer chains, 05-10 cycle detection,
//! 05-13 custom CA) is registered as `covered: false` with the real reason, and
//! the aggregate API only ever claims what passed.
//!
//! The execution lives in
//! `infiltrator_application::protocol_codec_matrix_application` because every
//! scenario has to run the real codec + application, never a hard-coded
//! booleans.

use serde::{Deserialize, Serialize};

/// One item's row in the protocol-codec regression matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCodecMatrixScenario {
    /// `DUAL-05-01` … `DUAL-05-15`.
    pub id: String,
    /// What the scenario exercises, in one line.
    pub item: String,
    /// `false` for the still-`planned` items; those carry a real reason.
    pub covered: bool,
    /// Only meaningful when `covered`; always computed by running the codec.
    pub passed: bool,
    pub detail: String,
}

/// The whole matrix, as executed by the shared application.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCodecMatrixReport {
    pub scenarios: Vec<ProtocolCodecMatrixScenario>,
}

impl ProtocolCodecMatrixReport {
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
            "协议编解码矩阵 {}/{} 项通过 · {} 项未覆盖",
            self.covered_passed_count(),
            self.covered_count(),
            self.not_covered_ids().len()
        )
    }
}
