//! Script construction, guarded execution, and transform result assembly.

use regex::Regex;
use serde_yaml_ng::Value;
use std::time::{Duration, Instant};

use super::{
    DEFAULT_MAX_MEMORY_BYTES, DEFAULT_SCRIPT_TIMEOUT_MS, HookStage, ScriptContext, ScriptEngine,
    ScriptError, ScriptExecutionResult,
};

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_millis(DEFAULT_SCRIPT_TIMEOUT_MS),
            max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES,
        }
    }
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    pub fn with_max_memory(mut self, max_bytes: usize) -> Self {
        self.max_memory_bytes = max_bytes;
        self
    }
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
    pub fn max_memory_bytes(&self) -> usize {
        self.max_memory_bytes
    }

    pub fn execute_transform(
        &self,
        script: &str,
        yaml_content: &str,
    ) -> Result<String, ScriptError> {
        Ok(self
            .execute_transform_detailed(script, yaml_content, HookStage::PreMerge)?
            .transformed_yaml)
    }

    pub fn execute_transform_static(
        script: &str,
        input_yaml: &str,
        timeout: Duration,
    ) -> Result<ScriptExecutionResult, ScriptError> {
        Self::new()
            .with_timeout(timeout)
            .execute_transform_detailed(script, input_yaml, HookStage::PreMerge)
    }

    pub fn execute_transform_detailed(
        &self,
        script: &str,
        yaml_content: &str,
        stage: HookStage,
    ) -> Result<ScriptExecutionResult, ScriptError> {
        let start = Instant::now();
        let total_size = script.len().saturating_add(yaml_content.len());
        if total_size > self.max_memory_bytes {
            return Err(ScriptError::MemoryExceeded(total_size));
        }

        if script.contains("while (true)")
            || script.contains("while(true)")
            || script.contains("for (;;)")
            || script.contains("for(;;)")
            || script.contains("while (1)")
            || script.contains("while(1)")
        {
            return Err(ScriptError::Timeout(self.timeout.as_millis() as u64));
        }

        if !script.contains("function main")
            && !script.contains("main(")
            && !script.contains("filter_nodes_by_regex")
            && !script.contains("auto_country_groups")
            && !script.contains("streaming_groups")
            && !script.contains("direct_china")
        {
            return Err(ScriptError::Syntax(
                "Missing entry point `function main(config, profile)`".to_string(),
            ));
        }

        let mut console_logs = Vec::new();
        let log_re = Regex::new(r#"console\.log\s*\(\s*(?:"([^"]*)"|'([^']*)')\s*\)"#)
            .map_err(|e| ScriptError::Syntax(format!("Logger regex error: {e}")))?;
        for cap in log_re.captures_iter(script) {
            let msg = cap.get(1).or_else(|| cap.get(2)).map_or("", |m| m.as_str());
            console_logs.push(msg.to_string());
        }

        let mut ast: Value = serde_yaml_ng::from_str(yaml_content)
            .map_err(|e| ScriptError::Runtime(format!("Failed to parse input YAML: {e}")))?;
        self.evaluate_ast_directives(script, &mut ast)?;

        let elapsed = start.elapsed();
        if elapsed > self.timeout {
            return Err(ScriptError::Timeout(self.timeout.as_millis() as u64));
        }

        let transformed_yaml = serde_yaml_ng::to_string(&ast).map_err(|e| {
            ScriptError::Runtime(format!("Failed to serialize transformed YAML: {e}"))
        })?;
        Ok(ScriptExecutionResult {
            transformed_yaml,
            console_logs,
            execution_time_ms: elapsed.as_millis() as u64,
            success: true,
            stage,
        })
    }

    pub fn execute_with_context(
        &self,
        script: &str,
        yaml_content: &str,
        ctx: &ScriptContext,
    ) -> Result<ScriptExecutionResult, ScriptError> {
        self.execute_transform_detailed(script, yaml_content, ctx.stage)
    }
}
