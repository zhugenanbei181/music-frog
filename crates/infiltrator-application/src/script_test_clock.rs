//! test-intent: behavior
//! Real directive evaluation with an explicit elapsed-time fixture, never a fabricated transform.
use infiltrator_contract::script_sandbox::{ScriptEngineCapabilities, ScriptEngineKind};
use infiltrator_domain::script_engine::{
    HookStage, ScriptEngine, ScriptError, ScriptExecutionResult,
};
use infiltrator_ports::script_engine::ScriptEnginePort;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct ClockedDirectiveEngine(pub Duration);
impl ScriptEnginePort for ClockedDirectiveEngine {
    fn kind(&self) -> ScriptEngineKind {
        ScriptEngineKind::DirectiveDsl
    }
    fn capabilities(&self) -> ScriptEngineCapabilities {
        ScriptEngineCapabilities::directive_dsl()
    }
    fn execute(
        &self,
        script: &str,
        input_yaml: &str,
        stage: HookStage,
    ) -> Result<ScriptExecutionResult, ScriptError> {
        let start = Instant::now();
        let mut started = false;
        ScriptEngine::new().execute_transform_detailed_with_clock(script, input_yaml, stage, || {
            if started {
                start + self.0
            } else {
                started = true;
                start
            }
        })
    }
}
