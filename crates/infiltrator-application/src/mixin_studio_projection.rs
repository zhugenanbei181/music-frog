//! Shared display fold for the typed Mixin catalogue and real cascade output.
use infiltrator_domain::mixin_studio::{
    CascadeOverlayReport, CascadeStageId, CascadeStageReport, MixinColumn, MixinColumnRole,
    MixinPreflightReport, MixinPresetId,
};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub const fn preset_key(id: MixinPresetId) -> &'static str {
    match id {
        MixinPresetId::Ipv6 => "mixin_toggle_ipv6",
        MixinPresetId::AllowLan => "mixin_toggle_allow_lan",
        MixinPresetId::DnsFakeIp => "mixin_toggle_dns_fake_ip",
        MixinPresetId::Tun => "mixin_toggle_tun",
        MixinPresetId::Sniffer => "mixin_toggle_sniffer",
        MixinPresetId::DebugLog => "mixin_toggle_log_debug",
    }
}
pub const fn column_key(role: MixinColumnRole) -> &'static str {
    match role {
        MixinColumnRole::Base => "mixin_column_base",
        MixinColumnRole::Overlay => "mixin_column_overlay",
        MixinColumnRole::Composed => "mixin_column_composed",
    }
}
pub const fn stage_key(id: CascadeStageId) -> &'static str {
    match id {
        CascadeStageId::Base => "mixin_cascade_base",
        CascadeStageId::Subscription => "mixin_cascade_subscription",
        CascadeStageId::Merge => "mixin_cascade_merge",
        CascadeStageId::PreMixin => "mixin_cascade_pre_mixin",
        CascadeStageId::PostMixin => "mixin_cascade_post_mixin",
    }
}
pub fn line_count(lines: usize, locale: &str) -> String {
    format!("{lines} {}", Lang(locale).tr("mixin_studio_cascade_lines"))
}
pub fn column_caption(column: &MixinColumn, locale: &str) -> String {
    let lang = Lang(locale);
    format!(
        "{} · {} · {}",
        lang.tr(column_key(column.role)),
        line_count(column.line_count, locale),
        lang.tr(if column.editable {
            "mixin_column_editable"
        } else {
            "mixin_column_readonly"
        })
    )
}
pub fn stage_caption(stage: &CascadeStageReport, locale: &str) -> String {
    let lang = Lang(locale);
    let label = lang.tr(stage_key(stage.id));
    if stage.applied {
        format!("{label} {}", line_count(stage.line_count, locale))
    } else {
        format!("{label} ({})", lang.tr("mixin_studio_cascade_undeclared"))
    }
}
pub fn composed_error(error: Option<&str>, locale: &str) -> String {
    error
        .map(|error| {
            localize(
                locale,
                "mixin_column_blocked",
                &[("error", error.to_owned())],
            )
        })
        .unwrap_or_default()
}
pub fn cascade_caption(report: &CascadeOverlayReport, locale: &str) -> String {
    let lang = Lang(locale);
    let title = lang.tr("mixin_studio_cascade_title");
    if report.blocked {
        return format!(
            "{title}: {}",
            composed_error(report.error.as_deref(), locale)
        );
    }
    let stages: Vec<_> = report
        .stages
        .iter()
        .map(|stage| stage_caption(stage, locale))
        .collect();
    format!(
        "{title}: {} · {} {}",
        stages.join(" → "),
        lang.tr("mixin_studio_cascade_merged"),
        line_count(report.merged_line_count(), locale)
    )
}
pub fn preflight_key(report: &MixinPreflightReport) -> &'static str {
    if report.is_blocking() {
        "mixin_studio_preflight_blocked"
    } else {
        "mixin_studio_preflight_ok"
    }
}
pub fn preflight_detail(report: &MixinPreflightReport, locale: &str) -> String {
    if report.is_blocking() {
        report.error.clone().unwrap_or_default()
    } else {
        Lang(locale).tr("mixin_studio_preflight_hint").into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_domain::mixin_studio::{mixin_editor_columns, preview_cascade_from_yaml};
    #[test]
    fn captions_preserve_missing_stages_actual_line_counts_and_blocking_causes() {
        let columns = mixin_editor_columns("mode: rule\nproxies: []\n", "ipv6: true\n");
        assert_eq!(
            column_caption(&columns.overlay, "en-US"),
            "Mixin overlay · 1 lines · editable"
        );
        let report = preview_cascade_from_yaml("mode: rule\nproxies: []\n", "ipv6: true\n");
        assert!(cascade_caption(&report, "en-US").contains("Subscription (not declared)"));
        assert!(cascade_caption(&report, "zh-CN").contains("未声明"));
        assert_eq!(
            composed_error(Some("cause {error}/中文🙂"), "en-US"),
            "Composition blocked: cause {error}/中文🙂"
        );
    }
}
