//! Shared script report fold; native renderers do not recalculate labels or limits.
use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;
use lurker_ui_presentation::copy::{TextCatalog, interpolate};

pub fn engine_meta_rows(
    catalog: &dyn TextCatalog,
    snapshot: &ScriptSandboxSnapshot,
) -> (String, String) {
    let engine = catalog.text(snapshot.engine_label_key()).into_owned();
    let capabilities = catalog
        .text(snapshot.engine_capability_label_key())
        .into_owned();
    (engine, capabilities)
}

pub struct ScriptReportPresentation {
    pub engine: String,
    pub capabilities: String,
    pub hook: String,
    pub breaker: String,
    pub limits: String,
    pub directives: Vec<String>,
    pub logs: Vec<String>,
    pub input: String,
    pub output: String,
}

pub fn project_report(
    snapshot: &ScriptSandboxSnapshot,
    catalog: &dyn TextCatalog,
) -> ScriptReportPresentation {
    let (engine, capabilities) = engine_meta_rows(catalog, snapshot);
    let state = catalog
        .text(if snapshot.circuit_breaker.tripped {
            "script_breaker_tripped"
        } else {
            "script_breaker_closed"
        })
        .into_owned();
    let fails = snapshot.circuit_breaker.consecutive_failures.to_string();
    let threshold = snapshot.circuit_breaker.failure_threshold.to_string();
    let cooldown = snapshot.circuit_breaker.cooldown_ms.to_string();
    let remaining = snapshot.circuit_breaker.remaining_cooldown_ms.to_string();
    let breaker = interpolate(
        catalog.text("script_sandbox_breaker_state").as_ref(),
        &[
            ("state", &state),
            ("fails", &fails),
            ("threshold", &threshold),
            ("cooldown", &cooldown),
            ("remaining", &remaining),
        ],
    );
    let memory_mb = (snapshot.max_memory_limit_bytes / (1024 * 1024)).to_string();
    let timeout = snapshot.timeout_limit_ms.to_string();
    let elapsed = snapshot.execution_time_ms.to_string();
    let bytes = snapshot.memory_used_bytes.to_string();
    let limits = interpolate(
        catalog.text("script_observed_limits").as_ref(),
        &[
            ("memory_mb", &memory_mb),
            ("timeout", &timeout),
            ("elapsed", &elapsed),
            ("bytes", &bytes),
        ],
    );
    let directives = snapshot
        .matched_directives
        .iter()
        .map(|directive| {
            let affected = directive.affected.to_string();
            interpolate(
                catalog.text("script_sandbox_directive_row").as_ref(),
                &[
                    ("id", &directive.id),
                    ("label", &directive.label),
                    ("affected", &affected),
                ],
            )
        })
        .collect();
    let logs = snapshot
        .console_logs
        .iter()
        .map(|entry| format!("[{}ms] {}", entry.timestamp_ms, entry.message))
        .collect();
    ScriptReportPresentation {
        engine,
        capabilities,
        hook: format!("{} ({})", snapshot.hook_stage_label, snapshot.hook_stage),
        breaker,
        limits,
        directives,
        logs,
        input: snapshot.input_yaml.clone(),
        output: snapshot.transformed_yaml.clone().unwrap_or_default(),
    }
}

pub fn report_rows(
    snapshot: Option<&ScriptSandboxSnapshot>,
    catalog: &dyn TextCatalog,
) -> Vec<String> {
    let Some(snapshot) = snapshot else {
        return vec![catalog.text("script_workbench_unobserved").into_owned()];
    };
    let view = project_report(snapshot, catalog);
    let mut rows = vec![
        catalog
            .text(if snapshot.has_error() {
                "script_sandbox_error_title"
            } else {
                "script_sandbox_success_title"
            })
            .into_owned(),
        format!(
            "{} {} · {} {}",
            catalog.text("script_sandbox_engine"),
            view.engine,
            catalog.text("script_sandbox_capabilities"),
            view.capabilities
        ),
        format!(
            "{} {}",
            catalog.text("script_sandbox_hook_stage"),
            view.hook
        ),
        format!(
            "{} {}",
            catalog.text("script_sandbox_breaker"),
            view.breaker
        ),
        view.limits,
    ];
    if let Some(error) = &snapshot.error_detail {
        rows.push(error.clone());
        rows.push(catalog.text("script_sandbox_degraded").into_owned());
    }
    if view.directives.is_empty() {
        rows.push(catalog.text("script_sandbox_no_match").into_owned());
    } else {
        rows.extend(view.directives);
    }
    rows.push(format!(
        "{} {}",
        catalog.text("script_sandbox_logs"),
        view.logs.len()
    ));
    rows.extend(view.logs.into_iter().take(32));
    rows.push(format!(
        "{}\n{}",
        catalog.text("script_sandbox_input_preview"),
        preview(&view.input, catalog)
    ));
    rows.push(format!(
        "{}\n{}",
        catalog.text("script_sandbox_output"),
        preview(&view.output, catalog)
    ));
    rows
}

fn preview(value: &str, catalog: &dyn TextCatalog) -> String {
    let mut prefix: String = value.chars().take(1600).collect();
    if value.chars().count() > 1600 {
        prefix.push_str(&format!("\n{}", catalog.text("script_preview_truncated")));
    }
    prefix
}
