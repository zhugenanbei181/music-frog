//! DUAL-10-01: the pluggable script-engine seam.
//!
//! The application executes script sandboxes through this trait, never through
//! a concrete engine. The shipped default is the bundled directive DSL (a
//! regex directive recogniser, **not** a JavaScript interpreter); a future
//! engine can be injected as another [`ScriptEnginePort`], and the shared read
//! model then reports the new [`ScriptEngineKind`] plus its negotiated
//! [`ScriptEngineCapabilities`] without any surface edit.
//!
//! Runtime-neutral on purpose: synchronous, no Tokio, no UI or platform types.
//! The domain engine's typed [`ScriptError`] is preserved so the application
//! keeps mapping timeout / memory / syntax / runtime failures onto the shared
//! status vocabulary instead of flattening them into one error.

use infiltrator_contract::script_sandbox::{ScriptEngineCapabilities, ScriptEngineKind};
use infiltrator_domain::script_engine::{HookStage, ScriptError, ScriptExecutionResult};

/// One script engine behind the application sandbox.
pub trait ScriptEnginePort: Send + Sync {
    /// Which engine this is; the shared read model reports it verbatim.
    fn kind(&self) -> ScriptEngineKind;

    /// The capability negotiation for this engine.
    fn capabilities(&self) -> ScriptEngineCapabilities;

    /// Run `script` against `input_yaml` at `stage`, returning the real
    /// transform result or a typed engine error.
    fn execute(
        &self,
        script: &str,
        input_yaml: &str,
        stage: HookStage,
    ) -> Result<ScriptExecutionResult, ScriptError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A stand-in "second engine": reports the JavaScript negotiation slot and
    /// returns the input untouched. It exists to prove the seam is a real
    /// trait-object boundary, not a single-implementation wrapper.
    struct FakeJavascriptEngine;

    impl ScriptEnginePort for FakeJavascriptEngine {
        fn kind(&self) -> ScriptEngineKind {
            ScriptEngineKind::JavascriptEngine
        }

        fn capabilities(&self) -> ScriptEngineCapabilities {
            ScriptEngineCapabilities {
                supports_javascript_syntax: true,
                supports_directive_dsl: false,
                ..ScriptEngineCapabilities::directive_dsl()
            }
        }

        fn execute(
            &self,
            _script: &str,
            input_yaml: &str,
            stage: HookStage,
        ) -> Result<ScriptExecutionResult, ScriptError> {
            Ok(ScriptExecutionResult {
                transformed_yaml: input_yaml.to_string(),
                console_logs: Vec::new(),
                execution_time_ms: 1,
                success: true,
                stage,
                matched_directives: Vec::new(),
            })
        }
    }

    #[test]
    fn a_second_engine_negotiates_its_kind_through_the_trait_object() {
        let engine: Arc<dyn ScriptEnginePort> = Arc::new(FakeJavascriptEngine);
        assert_eq!(engine.kind(), ScriptEngineKind::JavascriptEngine);
        assert!(engine.kind().is_real_javascript());
        assert!(engine.capabilities().supports_javascript_syntax);
        assert!(!engine.capabilities().supports_directive_dsl);
        assert!(
            engine
                .capabilities()
                .bottom_line_zh()
                .contains("支持 JavaScript")
        );
        let result = engine
            .execute("whatever", "port: 7890\n", HookStage::PreMerge)
            .expect("fake engine runs");
        assert_eq!(result.transformed_yaml, "port: 7890\n");
    }
}
