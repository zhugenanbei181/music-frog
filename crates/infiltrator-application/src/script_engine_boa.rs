//! DUAL-10-01 migration (§5 of `docs/SCRIPT_ENGINE_DECISION.md`): the real
//! ECMAScript adapter, compiled by default.
//!
//! This module is compiled when the `script-engine-boa` feature is enabled,
//! which is part of the default feature set (`default = ["script-engine-boa"]`).
//! The bundled directive DSL remains the default *selected* engine per host,
//! but the workspace build/test now compiles and exercises [`BoaScriptEngine`]:
//! a genuine ECMAScript interpreter ([`boa_engine`]) behind the same
//! [`ScriptEnginePort`] seam, so the shared read model reports
//! [`ScriptEngineKind::JavascriptEngine`] with `supports_javascript_syntax =
//! true` and both surfaces render the JS label without a surface edit. The
//! adapter is not QuickJS; see the honest limits below. An explicit
//! `--no-default-features` build drops this module and links no JS engine.
//!
//! Honest limits of this adapter (recorded in the decision record):
//!
//! * **Timeout** is enforced with Boa's loop-iteration budget
//!   ([`boa_engine::vm::RuntimeLimits::set_loop_iteration_limit`]) because Boa
//!   exposes no preemptive wall-clock interrupt; exceeding it maps to the
//!   shared typed [`ScriptError::Timeout`]. The wall-clock `timeout_ms` is
//!   reported, but the hard stop is the iteration budget.
//! * **Memory** is *not* enforced: Boa exposes no heap quota, so the adapter
//!   negotiates `enforces_memory_limit = false` instead of pretending the
//!   64MB ceiling is real.
//! * The regex **directive DSL library is not ported** to JavaScript here, so
//!   `supports_directive_dsl = false` and a run reports no matched directives.

use std::cell::RefCell;
use std::time::Instant;

use boa_engine::error::JsNativeErrorKind;
use boa_engine::object::ObjectInitializer;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsValue, NativeFunction, Source, js_string};
use infiltrator_contract::script_sandbox::{ScriptEngineCapabilities, ScriptEngineKind};
use infiltrator_domain::script_engine::{HookStage, ScriptError, ScriptExecutionResult};
use infiltrator_ports::script_engine::ScriptEnginePort;

/// Loop iterations allowed per millisecond of the negotiated timeout. Boa has
/// no wall-clock interrupt, so this is the enforceable stop for a runaway
/// `while (true) {}`; the product ceiling stays 500ms / 50M iterations.
const LOOP_ITERATIONS_PER_MS: u64 = 100_000;

thread_local! {
    /// Console capture for the single synchronous `execute` on this thread.
    static CONSOLE_LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// A real ECMAScript engine behind the shared [`ScriptEnginePort`].
#[derive(Clone, Debug)]
pub struct BoaScriptEngine {
    timeout_ms: u64,
    max_memory_bytes: usize,
}

impl Default for BoaScriptEngine {
    fn default() -> Self {
        Self::new(
            ScriptEngineCapabilities::DEFAULT_TIMEOUT_MS,
            ScriptEngineCapabilities::DEFAULT_MAX_MEMORY_BYTES,
        )
    }
}

impl BoaScriptEngine {
    pub fn new(timeout_ms: u64, max_memory_bytes: usize) -> Self {
        Self {
            timeout_ms,
            max_memory_bytes,
        }
    }

    fn loop_iteration_budget(&self) -> u64 {
        self.timeout_ms
            .saturating_mul(LOOP_ITERATIONS_PER_MS)
            .max(LOOP_ITERATIONS_PER_MS)
    }

    /// Map a Boa error onto the shared typed engine error vocabulary.
    fn map_error(&self, error: &boa_engine::JsError, syntax_hint: bool) -> ScriptError {
        if let Some(engine_error) = error.as_engine()
            && matches!(
                engine_error,
                boa_engine::error::EngineError::RuntimeLimit(_)
            )
        {
            return ScriptError::Timeout(self.timeout_ms);
        }
        let syntax = syntax_hint
            || error
                .as_native()
                .is_some_and(|native| matches!(native.kind(), JsNativeErrorKind::Syntax));
        if syntax {
            ScriptError::Syntax(error.to_string())
        } else {
            ScriptError::Runtime(error.to_string())
        }
    }
}

impl ScriptEnginePort for BoaScriptEngine {
    fn kind(&self) -> ScriptEngineKind {
        ScriptEngineKind::JavascriptEngine
    }

    fn capabilities(&self) -> ScriptEngineCapabilities {
        ScriptEngineCapabilities {
            supports_javascript_syntax: true,
            supports_directive_dsl: false,
            captures_console: true,
            enforces_timeout: true,
            // Honest: Boa has no heap quota; the 64MB figure is the product
            // ceiling, not an enforced engine limit.
            enforces_memory_limit: false,
            timeout_ms: self.timeout_ms,
            max_memory_bytes: self.max_memory_bytes,
        }
    }

    fn execute(
        &self,
        script: &str,
        input_yaml: &str,
        stage: HookStage,
    ) -> Result<ScriptExecutionResult, ScriptError> {
        let start = Instant::now();

        let json: serde_json::Value = serde_yaml_ng::from_str(input_yaml).map_err(|error| {
            ScriptError::Runtime(format!("input config is not JSON-compatible: {error}"))
        })?;

        let mut context = Context::default();
        context
            .runtime_limits_mut()
            .set_loop_iteration_limit(self.loop_iteration_budget());

        install_console(&mut context);

        CONSOLE_LOG.with(|logs| logs.borrow_mut().clear());

        // 1. Evaluate the script: it defines `main(config, profile)`.
        context
            .eval(Source::from_bytes(script))
            .map_err(|error| self.map_error(&error, false))?;

        // 2. Resolve the entry point.
        let main = context
            .global_object()
            .get(js_string!("main"), &mut context)
            .map_err(|error| self.map_error(&error, false))?
            .as_callable()
            .ok_or_else(|| ScriptError::Syntax("missing entry point `main`".to_string()))?;

        // 3. Call it with the parsed config and a small stage profile object.
        let config = JsValue::from_json(&json, &mut context)
            .map_err(|error| self.map_error(&error, false))?;
        let profile = serde_json::json!({ "stage": stage.as_str() });
        let profile = JsValue::from_json(&profile, &mut context)
            .map_err(|error| self.map_error(&error, false))?;
        let returned = main
            .call(&JsValue::undefined(), &[config, profile], &mut context)
            .map_err(|error| self.map_error(&error, false))?;

        // 4. Serialize the returned config back to YAML. `undefined` means the
        //    script mutated the argument in place and returned nothing.
        let transformed_yaml = match returned.to_json(&mut context) {
            Ok(Some(value)) => serde_yaml_ng::to_string(&value)
                .map_err(|error| ScriptError::Runtime(format!("cannot render config: {error}")))?,
            Ok(None) => input_yaml.to_string(),
            Err(error) => return Err(self.map_error(&error, false)),
        };

        let console_logs = CONSOLE_LOG.with(|logs| logs.borrow().clone());
        Ok(ScriptExecutionResult {
            transformed_yaml,
            console_logs,
            execution_time_ms: start.elapsed().as_millis() as u64,
            success: true,
            stage,
            matched_directives: Vec::new(),
        })
    }
}

/// Register a `console` object whose four methods append to the thread-local
/// capture. Zero-capture `Copy` closures keep Boa's GC out of the picture.
fn install_console(context: &mut Context) {
    fn method(level: &'static str) -> NativeFunction {
        NativeFunction::from_copy_closure(move |_this, args, context| {
            let mut parts = Vec::with_capacity(args.len());
            for arg in args {
                parts.push(arg.to_string(context)?.to_std_string_escaped());
            }
            CONSOLE_LOG.with(|logs| {
                logs.borrow_mut()
                    .push(format!("[{level}] {}", parts.join(" ")))
            });
            Ok(JsValue::undefined())
        })
    }

    let console = ObjectInitializer::new(context)
        .function(method("log"), js_string!("log"), 0)
        .function(method("info"), js_string!("info"), 0)
        .function(method("warn"), js_string!("warn"), 0)
        .function(method("error"), js_string!("error"), 0)
        .build();
    // `register_global_property` only fails on an invalid property key; the
    // literal `console` key is always valid.
    let _ = context.register_global_property(js_string!("console"), console, Attribute::all());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiates_a_real_javascript_engine() {
        let engine = BoaScriptEngine::default();
        assert_eq!(engine.kind(), ScriptEngineKind::JavascriptEngine);
        assert!(engine.kind().is_real_javascript());
        let capabilities = engine.capabilities();
        assert!(capabilities.supports_javascript_syntax);
        assert!(!capabilities.supports_directive_dsl);
        assert!(capabilities.captures_console);
        assert!(capabilities.enforces_timeout);
        // Honest: Boa exposes no heap quota.
        assert!(!capabilities.enforces_memory_limit);
        assert_eq!(capabilities.timeout_ms, 500);
    }

    #[test]
    fn executes_real_ecmascript_and_mutates_the_config() {
        let engine = BoaScriptEngine::default();
        let result = engine
            .execute(
                "function main(config, profile) { config.port = 8080; return config; }",
                "port: 7890\nmode: rule\n",
                HookStage::PreMerge,
            )
            .expect("boa runs the script");
        assert!(result.success);
        assert!(result.transformed_yaml.contains("port: 8080"));
        assert!(result.transformed_yaml.contains("mode: rule"));
        assert!(result.matched_directives.is_empty());
    }

    #[test]
    fn captures_console_output() {
        let engine = BoaScriptEngine::default();
        let result = engine
            .execute(
                "function main(config) { console.log(\"grouping\", 3); console.error(\"boom\"); return config; }",
                "port: 7890\n",
                HookStage::PreMerge,
            )
            .expect("boa runs the script");
        assert!(
            result
                .console_logs
                .iter()
                .any(|line| line.contains("grouping 3"))
        );
        assert!(result.console_logs.iter().any(|line| line.contains("boom")));
    }

    #[test]
    fn a_runaway_loop_maps_to_the_typed_timeout() {
        let engine = BoaScriptEngine::default();
        let error = engine
            .execute(
                "function main() { while (true) {} }",
                "port: 7890\n",
                HookStage::PreMerge,
            )
            .expect_err("the loop budget must stop the script");
        assert_eq!(error, ScriptError::Timeout(500));
    }

    #[test]
    fn a_parse_error_is_a_typed_syntax_error() {
        let engine = BoaScriptEngine::default();
        let error = engine
            .execute("function main( {", "port: 7890\n", HookStage::PreMerge)
            .expect_err("a malformed script must fail");
        assert!(matches!(error, ScriptError::Syntax(_)));
    }

    #[test]
    fn a_missing_entry_point_is_a_typed_syntax_error() {
        let engine = BoaScriptEngine::default();
        let error = engine
            .execute("let x = 1;", "port: 7890\n", HookStage::PreMerge)
            .expect_err("no main means no entry point");
        assert!(matches!(error, ScriptError::Syntax(_)));
    }
}
