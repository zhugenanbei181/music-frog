//! Shared script report fold; native renderers do not recalculate labels or limits.
use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn engine_meta_rows(lang: &Lang<'_>, snapshot: &ScriptSandboxSnapshot) -> (String, String) {
    let english = lang.0.starts_with("en");
    let engine = if english {
        snapshot.engine_label_en()
    } else {
        snapshot.engine_label_zh()
    };
    let capabilities = if english {
        snapshot.engine_capability_label_en()
    } else {
        snapshot.engine_capability_label_zh()
    };
    (engine.to_owned(), capabilities.to_owned())
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
    language: &str,
) -> ScriptReportPresentation {
    let lang = Lang(language);
    let (engine, capabilities) = engine_meta_rows(&lang, snapshot);
    let state = lang
        .tr(if snapshot.circuit_breaker.tripped {
            "script_breaker_tripped"
        } else {
            "script_breaker_closed"
        })
        .into_owned();
    let breaker = localize(
        language,
        "script_sandbox_breaker_state",
        &[
            ("state", state),
            (
                "fails",
                snapshot.circuit_breaker.consecutive_failures.to_string(),
            ),
            (
                "threshold",
                snapshot.circuit_breaker.failure_threshold.to_string(),
            ),
            ("cooldown", snapshot.circuit_breaker.cooldown_ms.to_string()),
            (
                "remaining",
                snapshot.circuit_breaker.remaining_cooldown_ms.to_string(),
            ),
        ],
    );
    let limits = localize(
        language,
        "script_observed_limits",
        &[
            (
                "memory_mb",
                (snapshot.max_memory_limit_bytes / (1024 * 1024)).to_string(),
            ),
            ("timeout", snapshot.timeout_limit_ms.to_string()),
            ("elapsed", snapshot.execution_time_ms.to_string()),
            ("bytes", snapshot.memory_used_bytes.to_string()),
        ],
    );
    let directives = snapshot
        .matched_directives
        .iter()
        .map(|directive| {
            localize(
                language,
                "script_sandbox_directive_row",
                &[
                    ("id", directive.id.clone()),
                    ("label", directive.label.clone()),
                    ("affected", directive.affected.to_string()),
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
pub fn report_rows(snapshot: Option<&ScriptSandboxSnapshot>, language: &str) -> Vec<String> {
    let lang = Lang(language);
    let Some(snapshot) = snapshot else {
        return vec![lang.tr("script_workbench_unobserved").into_owned()];
    };
    let view = project_report(snapshot, language);
    let mut rows = vec![
        lang.tr(if snapshot.has_error() {
            "script_sandbox_error_title"
        } else {
            "script_sandbox_success_title"
        })
        .into_owned(),
        format!(
            "{} {} · {} {}",
            lang.tr("script_sandbox_engine"),
            view.engine,
            lang.tr("script_sandbox_capabilities"),
            view.capabilities
        ),
        format!("{} {}", lang.tr("script_sandbox_hook_stage"), view.hook),
        format!("{} {}", lang.tr("script_sandbox_breaker"), view.breaker),
        view.limits,
    ];
    if let Some(error) = &snapshot.error_detail {
        rows.push(error.clone());
        rows.push(lang.tr("script_sandbox_degraded").into_owned());
    }
    if view.directives.is_empty() {
        rows.push(lang.tr("script_sandbox_no_match").into_owned());
    } else {
        rows.extend(view.directives);
    }
    rows.push(format!(
        "{} {}",
        lang.tr("script_sandbox_logs"),
        view.logs.len()
    ));
    rows.extend(view.logs.into_iter().take(32));
    rows.push(format!(
        "{}\n{}",
        lang.tr("script_sandbox_input_preview"),
        preview(&view.input, &lang)
    ));
    rows.push(format!(
        "{}\n{}",
        lang.tr("script_sandbox_output"),
        preview(&view.output, &lang)
    ));
    rows
}
fn preview(value: &str, lang: &Lang<'_>) -> String {
    let mut prefix: String = value.chars().take(1600).collect();
    if value.chars().count() > 1600 {
        prefix.push_str(&format!("\n{}", lang.tr("script_preview_truncated")));
    }
    prefix
}
