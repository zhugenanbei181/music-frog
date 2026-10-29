//! The business panels of the rules workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::rule_provider_projection::rules_summary;
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The business panels the rules page carries.
pub(super) fn panels(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    let page = snapshot.pages.rules.data.as_ref();
    let rules = page.map(|page| page.rules.len()).unwrap_or(0);
    let providers = page.map(|page| page.providers.len()).unwrap_or(0);
    let summary = page
        .map(|page| rules_summary(page.rules.len(), page.providers.len(), catalog))
        .unwrap_or_default();
    let provider_names = page
        .map(|page| {
            page.providers
                .iter()
                .map(|provider| provider.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let rule_rows = page
        .map(|page| {
            page.rules
                .iter()
                .map(|rule| format!("{} · {}", rule.rule_type, rule.payload))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tracer = snapshot.rule_trace.clone();
    let hit_audit = page.and_then(|page| page.hit_audit.clone());

    let failure = super::page_failure(&snapshot.pages.rules.status);
    let mut list = Panel::new(
        BusinessId::RulesListEditor,
        catalog.text("rules_flow_title"),
        summary,
        vec![
            catalog.text("rules_draft_unavailable").into_owned(),
            format!("{rules}"),
        ],
    );
    list.facts.extend(rule_rows);
    list.failure = failure.clone();
    list.empty = super::page_empty(&snapshot.pages.rules.status);

    let provider_cache = catalog
        .text("provider_cache_ready")
        .replace("{dir}", "cache/providers")
        .replace("{count}", &providers.to_string())
        .replace("{bytes}", &format_bytes(providers as u64 * 1024));
    let mut providers_panel = Panel::new(
        BusinessId::RulesProviderRefreshError,
        catalog.text("rules_providers_title"),
        format!("{providers}"),
        vec![
            catalog.text("rules_refresh_providers_action").into_owned(),
            provider_cache,
        ],
    );
    providers_panel.facts.extend(provider_names);
    providers_panel.failure = failure.clone();
    providers_panel.empty = super::page_empty(&snapshot.pages.rules.status);

    vec![
        list,
        {
            let mut panel = Panel::new(
                BusinessId::RulesOverrideEditor,
                catalog.text("rules_json_editor_title"),
                catalog.text("rules_draft_unnamed_profile").into_owned(),
                vec![
                    catalog.text("common_save_apply").into_owned(),
                    catalog.text("rules_form_discard").into_owned(),
                ],
            );
            panel.failure = failure.clone();
            panel.empty = super::page_empty(&snapshot.pages.rules.status);
            panel
        },
        providers_panel,
        {
            let mut panel = Panel::new(
                BusinessId::RulesStatisticsInspector,
                catalog.text("rule_hit_title"),
                catalog.text("rule_hits_unobserved").into_owned(),
                vec![
                    catalog.text("rules_stats_summary_tab").into_owned(),
                    catalog.text("rules_stats_cleanup_title").into_owned(),
                ],
            );
            panel.failure = failure.clone();
            // The owner's hit audit is the statistics the panel shows, so an
            // unobserved audit leaves the panel in its unknown state.
            panel.empty = hit_audit.is_none();
            panel
        },
        {
            let mut panel = Panel::new(
                BusinessId::RulesTracerDrawer,
                catalog.text("rules_tracer_title"),
                catalog.text("rule_tracer_desc").into_owned(),
                vec![
                    format!("{tracer:?}"),
                    catalog.text("rule_tracer_run_action").into_owned(),
                ],
            );
            panel.failure = failure.clone();
            panel.empty = super::page_empty(&snapshot.pages.rules.status);
            panel
        },
    ]
}
