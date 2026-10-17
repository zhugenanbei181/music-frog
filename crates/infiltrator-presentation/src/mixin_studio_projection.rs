//! Shared display fold for the typed Mixin catalogue and real cascade output.
use crate::text::localize;
use infiltrator_contract::profile_mixin_studio::{
    ProfileMixinCascade, ProfileMixinCascadeStage, ProfileMixinCascadeStageId, ProfileMixinColumn,
    ProfileMixinColumnRole, ProfileMixinPreflight, ProfileMixinStageDeclaration,
    ProfileMixinToggleId,
};
use lurker_ui_presentation::copy::TextCatalog;

pub const fn preset_key(id: ProfileMixinToggleId) -> &'static str {
    match id {
        ProfileMixinToggleId::Ipv6 => "mixin_toggle_ipv6",
        ProfileMixinToggleId::AllowLan => "mixin_toggle_allow_lan",
        ProfileMixinToggleId::DnsFakeIp => "mixin_toggle_dns_fake_ip",
        ProfileMixinToggleId::Tun => "mixin_toggle_tun",
        ProfileMixinToggleId::Sniffer => "mixin_toggle_sniffer",
        ProfileMixinToggleId::DebugLog => "mixin_toggle_log_debug",
    }
}
pub const fn column_key(role: ProfileMixinColumnRole) -> &'static str {
    match role {
        ProfileMixinColumnRole::Base => "mixin_column_base",
        ProfileMixinColumnRole::Overlay => "mixin_column_overlay",
        ProfileMixinColumnRole::Composed => "mixin_column_composed",
    }
}
pub const fn stage_key(id: ProfileMixinCascadeStageId) -> &'static str {
    match id {
        ProfileMixinCascadeStageId::Base => "mixin_cascade_base",
        ProfileMixinCascadeStageId::Subscription => "mixin_cascade_subscription",
        ProfileMixinCascadeStageId::Merge => "mixin_cascade_merge",
        ProfileMixinCascadeStageId::PreMixin => "mixin_cascade_pre_mixin",
        ProfileMixinCascadeStageId::PostMixin => "mixin_cascade_post_mixin",
    }
}
pub fn line_count(lines: usize, catalog: &dyn TextCatalog) -> String {
    format!("{lines} {}", catalog.text("mixin_studio_cascade_lines"))
}
pub fn column_caption(column: &ProfileMixinColumn, catalog: &dyn TextCatalog) -> String {
    column_caption_parts(column.role, column.line_count, column.editable, catalog)
}
pub fn column_caption_parts(
    role: ProfileMixinColumnRole,
    lines: usize,
    editable: bool,
    catalog: &dyn TextCatalog,
) -> String {
    format!(
        "{} · {} · {}",
        catalog.text(column_key(role)),
        line_count(lines, catalog),
        catalog.text(if editable {
            "mixin_column_editable"
        } else {
            "mixin_column_readonly"
        })
    )
}
pub fn stage_caption(stage: &ProfileMixinCascadeStage, catalog: &dyn TextCatalog) -> String {
    let label = catalog.text(stage_key(stage.id));
    match stage.declaration {
        ProfileMixinStageDeclaration::Applied => {
            format!("{label} {}", line_count(stage.line_count, catalog))
        }
        ProfileMixinStageDeclaration::Undeclared => {
            format!(
                "{label} ({})",
                catalog.text("mixin_studio_cascade_undeclared")
            )
        }
    }
}
pub fn composed_error(error: Option<&str>, catalog: &dyn TextCatalog) -> String {
    error
        .map(|error| {
            localize(
                catalog,
                "mixin_column_blocked",
                &[("error", error.to_owned())],
            )
        })
        .unwrap_or_default()
}
pub fn cascade_caption(report: &ProfileMixinCascade, catalog: &dyn TextCatalog) -> String {
    let title = catalog.text("mixin_studio_cascade_title");
    match report {
        ProfileMixinCascade::Blocked { reason, .. } => {
            format!("{title}: {}", composed_error(Some(reason), catalog))
        }
        ProfileMixinCascade::Merged {
            stages,
            line_count: merged,
        } => {
            let stages = stages
                .iter()
                .map(|stage| stage_caption(stage, catalog))
                .collect::<Vec<_>>();
            format!(
                "{title}: {} · {} {}",
                stages.join(" → "),
                catalog.text("mixin_studio_cascade_merged"),
                line_count(*merged, catalog)
            )
        }
    }
}

pub const fn preflight_key(preflight: &ProfileMixinPreflight) -> &'static str {
    match preflight {
        ProfileMixinPreflight::Accepted => "mixin_studio_preflight_ok",
        ProfileMixinPreflight::Blocked { .. } => "mixin_studio_preflight_blocked",
    }
}

pub fn preflight_detail(preflight: &ProfileMixinPreflight, catalog: &dyn TextCatalog) -> String {
    match preflight {
        ProfileMixinPreflight::Accepted => catalog.text("mixin_studio_preflight_hint").into_owned(),
        ProfileMixinPreflight::Blocked { reason } => reason.clone(),
    }
}

#[cfg(test)]
#[path = "mixin_studio_projection_tests.rs"]
mod tests;
