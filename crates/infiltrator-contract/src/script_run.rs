//! Correlated sandbox requests and monotonic session observations.
use crate::error::{ErrorCode, Failure};
use crate::script_sandbox::ScriptSandboxSnapshot;
use serde::{Deserialize, Serialize};

pub const MAX_SCRIPT_INPUT_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptEditorField {
    Code,
    InputYaml,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptOperationId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptRunRequest {
    pub operation: ScriptOperationId,
    pub script_code: String,
    pub input_yaml: String,
    pub preset: Option<String>,
}
impl ScriptRunRequest {
    pub fn validate(&self) -> Result<(), Failure> {
        if self.operation.0 == 0
            || self.script_code.len().saturating_add(self.input_yaml.len()) > MAX_SCRIPT_INPUT_BYTES
            || self
                .preset
                .as_ref()
                .is_some_and(|preset| preset.len() > 128)
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Invalid or oversized sandbox request",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptRunResult {
    pub operation: ScriptOperationId,
    pub revision: u64,
    pub snapshot: ScriptSandboxSnapshot,
}
impl ScriptRunResult {
    pub fn validate(&self, request: &ScriptRunRequest) -> Result<(), Failure> {
        request.validate()?;
        if self.operation != request.operation
            || self.revision == 0
            || self.snapshot.script_code != request.script_code
            || self.snapshot.input_yaml != request.input_yaml
            || self.snapshot.selected_preset != request.preset
            || !self.snapshot.engine_kind_matches_capabilities()
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Unrelated sandbox result",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptRunObservation {
    pub revision: u64,
    pub result: Option<ScriptRunResult>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptClearReceipt {
    pub operation: ScriptOperationId,
    pub revision: u64,
}
