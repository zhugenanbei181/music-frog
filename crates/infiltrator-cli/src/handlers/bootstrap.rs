use infiltrator_core::bootstrap::{self, BootstrapStep};

/// Ensure the default configs directory, profile, and controller settings
/// exist. Idempotent; already-satisfied steps are reported as skipped.
pub(crate) async fn handle() -> anyhow::Result<()> {
    let report = bootstrap::ensure_bootstrap().await?;
    for step in &report.steps {
        println!("{}", render_step(step));
    }
    Ok(())
}

pub(crate) fn render_step(step: &BootstrapStep) -> String {
    let state = if step.executed {
        "executed"
    } else {
        "skipped"
    };
    format!("{state:>8}  {}: {}", step.id, step.detail)
}

#[cfg(test)]
mod tests {
    use infiltrator_core::bootstrap::BootstrapStep;

    use super::render_step;

    #[test]
    fn executed_steps_are_labeled_as_executed() {
        let step = BootstrapStep {
            id: "configs_dir",
            executed: true,
            detail: "configs directory '/home/x/configs'".to_string(),
        };
        let line = render_step(&step);
        assert!(line.contains("executed"), "{line}");
        assert!(line.contains("configs_dir"), "{line}");
        assert!(line.contains("/home/x/configs"), "{line}");
    }

    #[test]
    fn satisfied_steps_are_labeled_as_skipped() {
        let step = BootstrapStep {
            id: "default_config",
            executed: false,
            detail: "current profile config already exists".to_string(),
        };
        assert!(render_step(&step).contains("skipped"));
    }
}
