//! The workbench pages of the Slint surface.
//!
//! Every page is a fold of the shared owner snapshot and the shared
//! presentation reductions. The surface owns no business rule of its own.

use super::routing;
use super::{Tone, button, header, lock, row, row_action, text};
use crate::WorkbenchItem;
use crate::surface::action::workbench::{
    AGGREGATE_CANCEL, AGGREGATE_CREATE, AGGREGATE_PREVIEW, APP_ROUTING_CANCEL,
    APP_ROUTING_MODE_ALL, APP_ROUTING_MODE_BYPASS, APP_ROUTING_MODE_PROXY, APP_ROUTING_REFRESH,
    APP_ROUTING_RULE, DNS_CACHE_CANCEL, DNS_CACHE_CONFIRM, DNS_CACHE_RETRY, DNS_HOSTS_ADD,
    DNS_HOSTS_CANCEL, DNS_HOSTS_IMPORT, DNS_HOSTS_REMOVE, DNS_HOSTS_SAVE, DNS_LEAK_PROBE,
    DNS_QUERY_CANCEL, DNS_QUERY_RETRY, DNS_QUERY_RUN, DOCTOR_BOOTSTRAP, DOCTOR_DIAGNOSE,
    DOCTOR_REPAIR, DOCTOR_REPAIR_ALL, DOCTOR_RETRY, FILTER_CANCEL, FILTER_SAVE,
};
use crate::surface::workbenches::Workbenches;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::aggregation_preview_projection;
use infiltrator_presentation::app_routing_page_model::AppRoutingPageModel;
use infiltrator_presentation::dns_cache_projection;
use infiltrator_presentation::dns_hosts_projection;
use infiltrator_presentation::dns_leak_projection;
use infiltrator_presentation::dns_query_projection;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use infiltrator_presentation::doctor_actions::DoctorActionState;
use infiltrator_presentation::doctor_projection;
use infiltrator_presentation::routing_projection;
use infiltrator_shared::dns_observation::DnsObservationTone;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The DNS page: the leak alert, the query panel, the cache flush and the
/// Hosts editor.
pub(super) fn dns_page(
    snapshot: &SurfaceSnapshot,
    catalog: &Lang<'_>,
    workbenches: &Workbenches,
) -> (
    Vec<WorkbenchItem>,
    Option<String>,
    Option<String>,
    String,
    String,
) {
    let mut items = Vec::new();
    let leak = dns_leak_projection::project_leak(&snapshot.dns_leak, catalog);
    items.push(header(
        "dns-leak",
        &catalog.text("dns_leak_probe_title"),
        &leak.conclusion,
        leak_tone(leak.tone),
    ));
    items.push(text(
        "dns-leak-detail",
        &format!(
            "{} {}",
            leak.sources,
            leak.feedback.clone().unwrap_or_default()
        ),
    ));
    for observation in &leak.rows {
        items.push(row(
            &format!("dns-leak/{}", observation.resolver),
            &observation.question,
            &observation.outcome,
            &observation.authority,
        ));
    }
    items.push(button(
        "dns-leak-run",
        &catalog.text(if leak.feedback.is_some() {
            "dns_leak_btn_retry"
        } else {
            "dns_leak_btn_run"
        }),
        DNS_LEAK_PROBE,
        true,
    ));

    let query = lock(&workbenches.dns_query);
    let query_display = dns_query_projection::project_query(&query, catalog);
    items.push(header(
        "dns-query",
        &catalog.text("dns_query_title"),
        &query_display.status,
        observation_tone(query_display.tone),
    ));
    items.push(text(
        "dns-query-detail",
        &format!("{} {}", query_display.provenance, query_display.question),
    ));
    for record in query_display.records.lines() {
        if !record.trim().is_empty() {
            items.push(row(
                &format!("dns-query/{}", record),
                record,
                "",
                &query_display.counter,
            ));
        }
    }
    items.push(button(
        "dns-query-run",
        &catalog.text("dns_query_run"),
        DNS_QUERY_RUN,
        query_display.run,
    ));
    items.push(button(
        "dns-query-retry",
        &catalog.text("dns_query_retry"),
        DNS_QUERY_RETRY,
        query_display.retry,
    ));
    items.push(button(
        "dns-query-close",
        &catalog.text("dns_query_close"),
        DNS_QUERY_CANCEL,
        query_display.close,
    ));
    let query_draft = query.name.clone();

    let cache = lock(&workbenches.dns_cache);
    let cache_display = dns_cache_projection::project_cache(&cache, catalog);
    items.push(header(
        "dns-cache",
        &catalog.text("dns_cache_title"),
        &cache_display.status,
        if cache_display.error {
            Tone::Danger
        } else {
            Tone::Neutral
        },
    ));
    items.push(row(
        "dns-cache/fake-ip",
        &cache_display.fake_ip,
        &cache_display.results_label,
        "",
    ));
    items.push(row(
        "dns-cache/system",
        &cache_display.system,
        &cache_display.results_label,
        "",
    ));
    items.push(button(
        "dns-cache-confirm",
        &catalog.text("dns_cache_confirm"),
        DNS_CACHE_CONFIRM,
        cache_display.confirm,
    ));
    items.push(button(
        "dns-cache-retry",
        &catalog.text("dns_cache_retry"),
        DNS_CACHE_RETRY,
        cache_display.retry,
    ));
    items.push(button(
        "dns-cache-cancel",
        &catalog.text("dns_cache_cancel"),
        DNS_CACHE_CANCEL,
        cache_display.close,
    ));

    let mut hosts = lock(&workbenches.dns_hosts);
    // The DNS page is the Hosts editor surface, so the editor is open while
    // the page shows it.
    hosts.show();
    items.push(header(
        "dns-hosts",
        &catalog.text("dns_hosts_title"),
        &dns_hosts_projection::summary(&hosts, catalog),
        Tone::Neutral,
    ));
    items.push(text(
        "dns-hosts-feedback",
        &dns_hosts_projection::feedback(&hosts, catalog),
    ));
    for entry in &hosts.rows {
        items.push(row_action(
            &format!("dns-hosts/{}", entry.id),
            &entry.id.to_string(),
            &format!("{} {}", entry.entry.address, entry.entry.domain),
            &entry.entry.domain,
            &entry.entry.address,
            DNS_HOSTS_REMOVE,
            &catalog.text("dns_hosts_remove"),
        ));
    }
    items.push(button(
        "dns-hosts-save",
        &catalog.text("dns_hosts_apply"),
        DNS_HOSTS_SAVE,
        hosts.can_apply(),
    ));
    items.push(button(
        "dns-hosts-add",
        &catalog.text("dns_hosts_add"),
        DNS_HOSTS_ADD,
        hosts.is_open(),
    ));
    items.push(button(
        "dns-hosts-import",
        &catalog.text("dns_hosts_import_legacy"),
        DNS_HOSTS_IMPORT,
        hosts.is_open(),
    ));
    items.push(button(
        "dns-hosts-cancel",
        &catalog.text("common_confirm"),
        DNS_HOSTS_CANCEL,
        hosts.is_open(),
    ));
    let hosts_draft = {
        let drafts = lock(&workbenches.drafts);
        drafts.hosts_row().to_owned()
    };
    (
        items,
        Some(query_draft),
        Some(hosts_draft),
        "dns-query-name".to_owned(),
        "dns-hosts-row".to_owned(),
    )
}

/// The configuration filter and aggregation page.
pub(super) fn filters_page(
    snapshot: &SurfaceSnapshot,
    catalog: &Lang<'_>,
    workbenches: &Workbenches,
) -> (
    Vec<WorkbenchItem>,
    Option<String>,
    Option<String>,
    String,
    String,
) {
    let mut items = Vec::new();
    let filter = lock(&workbenches.filter);
    items.push(header(
        "subscription-filter",
        &catalog.text("field_filter_include"),
        &catalog.text(if filter.dirty() {
            "filter_form_dirty"
        } else {
            "filter_form_unobserved"
        }),
        Tone::Neutral,
    ));
    items.push(text(
        "subscription-filter-detail",
        &format!(
            "{} · {} · {}",
            filter.draft.include, filter.draft.exclude, filter.draft.renames
        ),
    ));
    if let Some(failure) = filter.failure() {
        items.push(text("subscription-filter-error", &failure.message));
    }
    items.push(button(
        "subscription-filter-save",
        &catalog.text("common_save_apply"),
        FILTER_SAVE,
        filter.can_edit(),
    ));
    items.push(button(
        "subscription-filter-cancel",
        &catalog.text("common_confirm"),
        FILTER_CANCEL,
        filter.can_edit(),
    ));
    let filter_draft = filter.draft.include.clone();

    let page = snapshot.pages.profiles.data.as_ref();
    items.push(header(
        "profile-aggregator",
        &catalog.text("aggregator_desc"),
        &catalog.text("aggregation_preview_pending"),
        Tone::Neutral,
    ));
    if let Some(page) = page {
        items.push(text(
            "profile-aggregator-counters",
            &aggregation_preview_projection::aggregation_counters(
                page.aggregation.as_ref(),
                catalog,
            ),
        ));
        items.push(text(
            "profile-aggregator-groups",
            &aggregation_preview_projection::aggregation_groups(page.aggregation.as_ref(), catalog),
        ));
        for template in &page.aggregation_templates {
            items.push(row(
                &format!("profile-aggregator/{}", template.name),
                &template.name,
                &template.updated_at,
                &template.draft.target_name,
            ));
        }
    }
    items.push(button(
        "profile-aggregator-preview",
        &catalog.text("aggregator_btn_preview"),
        AGGREGATE_PREVIEW,
        true,
    ));
    items.push(button(
        "profile-aggregator-create",
        &catalog.text("aggregator_btn_merge"),
        AGGREGATE_CREATE,
        true,
    ));
    items.push(button(
        "profile-aggregator-cancel",
        &catalog.text("common_confirm"),
        AGGREGATE_CANCEL,
        true,
    ));
    let aggregation_target = {
        let drafts = lock(&workbenches.drafts);
        drafts.aggregation_target().to_owned()
    };

    (
        items,
        Some(filter_draft),
        Some(aggregation_target),
        "filter-include".to_owned(),
        "aggregator-target".to_owned(),
    )
}

/// The diagnostics and the lifecycle recovery page.
pub(super) fn doctor_page(
    snapshot: &SurfaceSnapshot,
    catalog: &Lang<'_>,
    workbenches: &Workbenches,
) -> (
    Vec<WorkbenchItem>,
    Option<String>,
    Option<String>,
    String,
    String,
) {
    let mut items = Vec::new();
    let page = snapshot.pages.doctor.data.as_ref();
    items.push(header(
        "doctor-report",
        &catalog.text("doctor_section_title"),
        &page.map_or_else(
            || catalog.text("doctor_not_observed").into_owned(),
            |page| {
                doctor_projection::summary(
                    page.checks.iter().map(|check| check.state),
                    page.report_finished_at,
                    catalog,
                )
            },
        ),
        if page.is_some_and(|page| page.overall_healthy) {
            Tone::Success
        } else {
            Tone::Warning
        },
    ));
    if let Some(page) = page {
        for check in &page.checks {
            items.push(row_action(
                &format!("doctor/{}", check.id),
                &check.id,
                &doctor_projection::check_name(check.kind, &check.name, &check.category, catalog),
                &doctor_projection::check_detail(
                    check.detail_copy_key.as_deref(),
                    &check.detail,
                    catalog,
                ),
                &catalog.text(doctor_projection::status_key(check.state)),
                if check.fix_available {
                    DOCTOR_REPAIR
                } else {
                    ""
                },
                &catalog.text("doctor_btn_fix"),
            ));
        }
    }
    let doctor = lock(&workbenches.doctor);
    // The diagnostic panel reports the owner's answer for its own action, so
    // the recovery business shows the typed refusal it has to recover from.
    // A running operation shows the shared pending copy, so the repair
    // progress is visible while the owner still owes its terminal.
    if let Some(feedback) = doctor_feedback(&doctor, catalog) {
        items.push(text("doctor-failure", &feedback));
    }
    items.push(button(
        "doctor-diagnose",
        &catalog.text("doctor_btn_check"),
        DOCTOR_DIAGNOSE,
        !matches!(&doctor.phase, DoctorActionPhase::Pending(_)),
    ));
    items.push(button(
        "doctor-repair-all",
        &catalog.text("doctor_repair_row"),
        DOCTOR_REPAIR_ALL,
        page.is_some_and(|page| page.checks.iter().any(|check| check.fix_available)),
    ));
    items.push(button(
        "doctor-bootstrap",
        &catalog.text("doctor_btn_bootstrap"),
        DOCTOR_BOOTSTRAP,
        true,
    ));
    items.push(button(
        "doctor-retry",
        &catalog.text("script_workbench_retry"),
        DOCTOR_RETRY,
        doctor.can_retry(),
    ));

    let watchdog = doctor_projection::watchdog_status_text(&snapshot.core.watchdog, catalog);
    items.push(header(
        "lifecycle-recovery",
        &catalog.text("doctor_suite_title"),
        &watchdog,
        Tone::Neutral,
    ));
    (items, None, None, String::new(), String::new())
}

/// The application routing policy page.
pub(super) fn app_routing_page(
    snapshot: &SurfaceSnapshot,
    catalog: &Lang<'_>,
    workbenches: &Workbenches,
) -> (
    Vec<WorkbenchItem>,
    Option<String>,
    Option<String>,
    String,
    String,
) {
    let mut items = Vec::new();
    // The routing mode is the shared page model's fact: the surface asks the
    // model which mode the owner published and renders the shared copy for
    // it, so an unobserved mode is never shown as a default one.
    let mut model = AppRoutingPageModel::default();
    if let Some(page) = snapshot.pages.app_routing.data.as_ref() {
        model.observe_page(page.clone());
    }
    items.push(header(
        "app-routing",
        &catalog.text("app_routing_title"),
        &catalog.text(model.mode_copy_key()),
        Tone::Neutral,
    ));
    items.push(text(
        "app-routing-summary",
        &routing::summary(model.mode(), model.apps().len(), catalog),
    ));
    for app in model.apps() {
        items.push(row_action(
            &format!("app-routing/{}", app.id),
            &app.id,
            &app.name,
            &routing_projection::process_copy(&app.process_name, catalog),
            &catalog.text(routing::rule_label(app.rule)),
            APP_ROUTING_RULE,
            &catalog.text("app_routing_add_action"),
        ));
    }
    let observed = model.mode_observed();
    items.push(button(
        "app-routing-mode-all",
        &catalog.text("app_routing_mode_global"),
        APP_ROUTING_MODE_ALL,
        observed,
    ));
    items.push(button(
        "app-routing-mode-proxy",
        &catalog.text("app_routing_mode_selected_proxy"),
        APP_ROUTING_MODE_PROXY,
        observed,
    ));
    items.push(button(
        "app-routing-mode-bypass",
        &catalog.text("app_routing_mode_selected_direct"),
        APP_ROUTING_MODE_BYPASS,
        observed,
    ));
    items.push(button(
        "app-routing-cancel",
        &catalog.text("common_confirm"),
        APP_ROUTING_CANCEL,
        true,
    ));
    items.push(button(
        "app-routing-refresh",
        &catalog.text("snapshot_diff_refresh"),
        APP_ROUTING_REFRESH,
        true,
    ));
    let routing_rule = {
        let drafts = lock(&workbenches.drafts);
        drafts.routing_rule().to_owned()
    };
    (
        items,
        None,
        Some(routing_rule),
        String::new(),
        "app-routing-rule".to_owned(),
    )
}

/// The typed refusal the diagnostic panel shows for its own action.
///
/// The shared reducer owns the phase, so the panel reports the owner's answer
/// instead of an empty state.
/// The feedback the diagnostic panel shows for its own action.
///
/// A running operation shows the shared pending copy and a refused operation
/// shows the typed refusal it has to recover from, so the panel that owns the
/// business never shows a state the owner did not publish.
fn doctor_feedback(doctor: &DoctorActionState, catalog: &Lang<'_>) -> Option<String> {
    match &doctor.phase {
        DoctorActionPhase::Idle => None,
        DoctorActionPhase::Pending(_) => {
            Some(catalog.text("doctor_operation_pending").into_owned())
        }
        DoctorActionPhase::Failed { failure, .. } => Some(failure.message.clone()),
    }
}

/// The tone of the shared DNS leak conclusion.
fn leak_tone(tone: dns_leak_projection::LeakTone) -> Tone {
    match tone {
        dns_leak_projection::LeakTone::Neutral => Tone::Neutral,
        dns_leak_projection::LeakTone::Success => Tone::Success,
        dns_leak_projection::LeakTone::Warning => Tone::Warning,
        dns_leak_projection::LeakTone::Danger => Tone::Danger,
    }
}

/// The tone of the shared DNS observation.
fn observation_tone(tone: DnsObservationTone) -> Tone {
    match tone {
        DnsObservationTone::Neutral => Tone::Neutral,
        DnsObservationTone::Success => Tone::Success,
        DnsObservationTone::Warning => Tone::Warning,
        DnsObservationTone::Danger => Tone::Danger,
    }
}
