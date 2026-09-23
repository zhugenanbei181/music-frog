//! DUAL-10-01: the bundled default script-engine adapter.
//!
//! `DirectiveDslScriptEngine` wraps the real domain `ScriptEngine` (a regex
//! directive recogniser, **not** a JavaScript interpreter) and exposes it
//! through the shared `ScriptEnginePort`. It negotiates
//! `supports_javascript_syntax = false`, so the shared read model can never
//! imply that arbitrary JavaScript executed.

use std::time::Duration;

use infiltrator_contract::script_sandbox::{ScriptEngineCapabilities, ScriptEngineKind};
use infiltrator_domain::script_engine::{
    HookStage, ScriptEngine, ScriptError, ScriptExecutionResult,
};
use infiltrator_ports::script_engine::ScriptEnginePort;

/// The default engine: the bundled directive DSL.
#[derive(Clone, Debug)]
pub struct DirectiveDslScriptEngine {
    engine: ScriptEngine,
}

impl Default for DirectiveDslScriptEngine {
    fn default() -> Self {
        Self::new(
            ScriptEngineCapabilities::DEFAULT_TIMEOUT_MS,
            ScriptEngineCapabilities::DEFAULT_MAX_MEMORY_BYTES,
        )
    }
}

impl DirectiveDslScriptEngine {
    pub fn new(timeout_ms: u64, max_memory_bytes: usize) -> Self {
        Self {
            engine: ScriptEngine::new()
                .with_timeout(Duration::from_millis(timeout_ms))
                .with_max_memory(max_memory_bytes),
        }
    }
}

impl ScriptEnginePort for DirectiveDslScriptEngine {
    fn kind(&self) -> ScriptEngineKind {
        ScriptEngineKind::DirectiveDsl
    }

    fn capabilities(&self) -> ScriptEngineCapabilities {
        ScriptEngineCapabilities {
            timeout_ms: self.engine.timeout().as_millis() as u64,
            max_memory_bytes: self.engine.max_memory_bytes(),
            ..ScriptEngineCapabilities::directive_dsl()
        }
    }

    fn execute(
        &self,
        script: &str,
        input_yaml: &str,
        stage: HookStage,
    ) -> Result<ScriptExecutionResult, ScriptError> {
        self.engine
            .execute_transform_detailed(script, input_yaml, stage)
    }
}
