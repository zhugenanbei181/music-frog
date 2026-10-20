//! SLINT-001.8: the interaction paths of the DNS, editor and filter businesses.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, hands the owner's typed terminal to the shared reducer
//! that owns the command, and asserts the typed outcome the reducer keeps.

#[path = "workbench_paths/fixture.rs"]
mod fixture;

use fixture::{
    local, refusal, refuse, rejection, single_command, unsupported, workbench_snapshot,
    workbench_surface,
};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::dns_cache::{DnsCacheOperation, DnsFlushOutcome};
use infiltrator_contract::dns_hosts::DnsHostsProfile;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::dns_query::DnsQueryOperationId;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::script_export::{ScriptExportKind, ScriptExportOutcome};
use infiltrator_contract::script_export_review::ScriptExportIdentity;
use infiltrator_contract::script_export_review::{
    ScriptExportDraft, ScriptExportReview, ScriptExportValidationScope,
};
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_presentation::dns_cache_actions::DnsCachePhase;
use infiltrator_presentation::dns_query_actions::DnsQueryPhase;
use infiltrator_presentation::script_workbench::{ScriptExecutionPhase, ScriptExportPhase};
use infiltrator_slint::surface::action::workbench;
use infiltrator_slint::surface::harness::SurfaceHarness;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::page::PageData;

// -- dns-leak-alert -------------------------------------------------------

#[test]
fn the_dns_leak_probe_reports_the_owner_refusal_without_a_verdict() {
    let surface = workbench_surface("Dns");

    let command = single_command(surface.workbench_action(workbench::DNS_LEAK_PROBE, ""));
    assert_eq!(command, CommandIntent::TestDnsLeak);

    refuse(&surface, &refusal("the echo authority refused the probe"));

    assert_eq!(surface.notice(), "the echo authority refused the probe");
    let item = surface
        .workbench_item("dns-leak")
        .expect("the leak panel is projected");
    assert_eq!(item.tone.as_str(), "success");
}

#[test]
fn the_dns_leak_panel_shows_the_typed_unsupported_conclusion() {
    let mut snapshot = workbench_snapshot();
    snapshot.dns_leak = DnsLeakReport::unsupported("no echo authority is configured");
    let surface = SurfaceHarness::new(Some(snapshot)).expect("headless surface");
    surface.open_page("Dns");

    let item = surface
        .workbench_item("dns-leak")
        .expect("the leak panel is projected");
    assert_eq!(item.tone.as_str(), "neutral");
    assert!(item.status.contains("no echo authority is configured"));
}

// -- dns-query-details ---------------------------------------------------

#[test]
fn the_dns_query_panel_refuses_the_close_while_the_query_is_in_flight() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft("example.org");
    single_command(surface.workbench_action(workbench::DNS_QUERY_RUN, ""));

    // The shared query projection hides the close button while the query is
    // in flight, so the gesture is a typed refusal and not a local
    // transition: the panel waits for the owner instead of vanishing.
    let failure = rejection(surface.workbench_action(workbench::DNS_QUERY_CANCEL, ""));
    assert_eq!(failure.code, ErrorCode::InvalidState);

    let query = surface
        .workbenches()
        .dns_query
        .lock()
        .expect("the query reducer");
    assert!(query.is_open());
}

#[test]
fn the_dns_query_panel_is_cancelled_without_the_owner() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft("example.org");
    single_command(surface.workbench_action(workbench::DNS_QUERY_RUN, ""));

    // The owner answered, so the panel may close and the close is a local
    // transition: it asks the owner for nothing.
    refuse(&surface, &refusal("the controller refused the query"));
    local(surface.workbench_action(workbench::DNS_QUERY_CANCEL, ""));

    let query = surface
        .workbenches()
        .dns_query
        .lock()
        .expect("the query reducer");
    assert!(!query.is_open());
}

#[test]
fn the_dns_query_failure_offers_the_retry_of_the_same_command() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft("example.org");
    single_command(surface.workbench_action(workbench::DNS_QUERY_RUN, ""));

    refuse(&surface, &refusal("the controller refused the query"));

    let query = surface
        .workbenches()
        .dns_query
        .lock()
        .expect("the query reducer");
    assert!(matches!(query.phase(), DnsQueryPhase::Failed { .. }));
    drop(query);

    let command = single_command(surface.workbench_action(workbench::DNS_QUERY_RETRY, ""));
    let CommandIntent::QueryDns { request, .. } = command else {
        panic!("expected the shared DNS query command");
    };
    assert_eq!(request.name, "example.org");
}

#[test]
fn the_dns_query_panel_shows_the_typed_unsupported_service() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft("example.org");
    single_command(surface.workbench_action(workbench::DNS_QUERY_RUN, ""));

    refuse(&surface, &unsupported("this host has no DNS controller"));

    let item = surface
        .workbench_item("dns-query")
        .expect("the query panel is projected");
    assert_eq!(item.tone.as_str(), "neutral");
    assert!(item.status.contains("this host has no DNS controller"));
}

#[test]
fn the_dns_query_panel_keeps_its_facts_when_a_foreign_readback_arrives() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft("example.org");
    single_command(surface.workbench_action(workbench::DNS_QUERY_RUN, ""));

    let mut foreign = workbench_snapshot();
    let mut readback = fixture::dns_query_snapshot();
    readback.revision = 9;
    readback.operation_id = Some(DnsQueryOperationId(999));
    readback.report_id = Some(DnsQueryOperationId(999));
    foreign.dns_query = readback;
    surface.publish_snapshot(foreign);

    let query = surface
        .workbenches()
        .dns_query
        .lock()
        .expect("the query reducer");
    assert!(matches!(query.phase(), DnsQueryPhase::Submitting { .. }));
}

#[test]
fn the_dns_query_panel_shows_the_empty_answer_of_the_controller() {
    let mut snapshot = workbench_snapshot();
    let mut readback = fixture::dns_query_snapshot();
    if let Some(report) = readback.report.as_mut() {
        report.response.answers.clear();
    }
    snapshot.dns_query = readback;
    let surface = SurfaceHarness::new(Some(snapshot)).expect("headless surface");
    surface.open_page("Dns");

    let query = surface
        .workbenches()
        .dns_query
        .lock()
        .expect("the query reducer");
    assert!(query.rows().is_empty());
    drop(query);
    let records: Vec<String> = surface
        .workbench_items()
        .into_iter()
        .filter(|item| item.id.starts_with("dns-query/"))
        .map(|item| item.title.to_string())
        .collect();
    assert!(
        records.iter().any(|record| record.contains("no records")),
        "the empty section is shown, got {records:?}"
    );
}

// -- dns-fake-ip-flush-confirm --------------------------------------------

#[test]
fn the_cache_flush_failure_reaches_the_panel_and_offers_the_retry() {
    let surface = workbench_surface("Dns");
    single_command(surface.workbench_action(workbench::DNS_CACHE_CONFIRM, ""));

    refuse(&surface, &refusal("the flush target was refused"));

    let cache = surface
        .workbenches()
        .dns_cache
        .lock()
        .expect("the cache reducer");
    assert!(matches!(cache.phase(), DnsCachePhase::Failed { .. }));
    drop(cache);

    let command = single_command(surface.workbench_action(workbench::DNS_CACHE_RETRY, ""));
    assert!(matches!(command, CommandIntent::ClearDnsCache { .. }));
}

#[test]
fn the_cache_flush_panel_shows_the_typed_unsupported_target() {
    let surface = workbench_surface("Dns");
    single_command(surface.workbench_action(workbench::DNS_CACHE_CONFIRM, ""));
    let operation = {
        let cache = surface
            .workbenches()
            .dns_cache
            .lock()
            .expect("the cache reducer");
        cache.requested_operation().expect("the pending operation")
    };

    let mut readback = fixture::dns_cache_snapshot();
    readback.revision = 2;
    readback.operation_id = Some(operation);
    readback.report_id = Some(operation);
    readback.operation = DnsCacheOperation::Unsupported;
    readback.report.fake_ip = DnsFlushOutcome::Unsupported {
        reason: "this host has no drivable Fake-IP cache".to_owned(),
    };
    readback.report.os_cache = DnsFlushOutcome::Unsupported {
        reason: "this host has no drivable resolver cache".to_owned(),
    };
    let mut snapshot = workbench_snapshot();
    snapshot.dns_cache = readback;
    surface.publish_snapshot(snapshot);

    let item = surface
        .workbench_item("dns-cache")
        .expect("the cache panel is projected");
    assert!(item.status.contains("no available cache clearing adapter"));
}

#[test]
fn the_cache_flush_keeps_its_facts_when_a_foreign_readback_arrives() {
    let surface = workbench_surface("Dns");
    single_command(surface.workbench_action(workbench::DNS_CACHE_CONFIRM, ""));

    let mut foreign = workbench_snapshot();
    let mut readback = fixture::dns_cache_snapshot();
    readback.revision = 9;
    foreign.dns_cache = readback;
    surface.publish_snapshot(foreign);

    let cache = surface
        .workbenches()
        .dns_cache
        .lock()
        .expect("the cache reducer");
    assert!(matches!(cache.phase(), DnsCachePhase::Submitting { .. }));
}

// -- dns-hosts-editor ----------------------------------------------------

#[test]
fn the_hosts_editor_drops_its_draft_without_the_owner() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft_2("8.8.8.8 new.test");
    local(surface.workbench_action(workbench::DNS_HOSTS_ADD, ""));

    local(surface.workbench_action(workbench::DNS_HOSTS_CANCEL, ""));

    let hosts = surface
        .workbenches()
        .dns_hosts
        .lock()
        .expect("the hosts reducer");
    assert!(!hosts.is_dirty());
    assert_eq!(hosts.rows.len(), 1);
}

#[test]
fn the_hosts_write_refusal_offers_the_write_again() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft_2("8.8.8.8 new.test");
    local(surface.workbench_action(workbench::DNS_HOSTS_ADD, ""));
    single_command(surface.workbench_action(workbench::DNS_HOSTS_SAVE, ""));

    refuse(&surface, &refusal("the profile write was refused"));

    let hosts = surface
        .workbenches()
        .dns_hosts
        .lock()
        .expect("the hosts reducer");
    assert!(hosts.failure().is_some());
    drop(hosts);

    let command = single_command(surface.workbench_action(workbench::DNS_HOSTS_SAVE, ""));
    let CommandIntent::ApplyDnsSettings { patch } = command else {
        panic!("expected the shared DNS settings patch");
    };
    assert!(
        patch
            .hosts
            .as_ref()
            .is_some_and(|entries| entries.iter().any(|entry| entry.domain == "new.test"))
    );
}

#[test]
fn the_hosts_editor_migrates_the_preserved_legacy_rows() {
    let surface = workbench_surface("Dns");
    local(surface.workbench_action(workbench::DNS_HOSTS_IMPORT, ""));

    let command = single_command(surface.workbench_action(workbench::DNS_HOSTS_SAVE, ""));
    let CommandIntent::ApplyDnsSettings { patch } = command else {
        panic!("expected the shared DNS settings patch");
    };
    assert!(patch.remove_legacy_hosts);
    assert!(
        patch
            .hosts
            .as_ref()
            .is_some_and(|entries| entries.iter().any(|entry| entry.domain == "legacy.test"))
    );
}

#[test]
fn the_hosts_editor_keeps_its_draft_when_the_profile_changed_underneath() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft_2("8.8.8.8 new.test");
    local(surface.workbench_action(workbench::DNS_HOSTS_ADD, ""));
    single_command(surface.workbench_action(workbench::DNS_HOSTS_SAVE, ""));

    let mut changed = workbench_snapshot();
    changed.dns_hosts = PageData::ready(DnsHostsProfile {
        profile: "other".to_owned(),
        entries: Vec::new(),
        legacy_entries: Vec::new(),
    });
    surface.publish_snapshot(changed);

    let hosts = surface
        .workbenches()
        .dns_hosts
        .lock()
        .expect("the hosts reducer");
    assert!(hosts.read_failure().is_some());
    assert!(hosts.rows.iter().any(|row| row.entry.domain == "new.test"));
}

#[test]
fn the_hosts_editor_clears_the_mappings_of_an_empty_draft() {
    let surface = workbench_surface("Dns");
    let row = {
        let hosts = surface
            .workbenches()
            .dns_hosts
            .lock()
            .expect("the hosts reducer");
        hosts.rows[0].id
    };
    local(surface.workbench_action(workbench::DNS_HOSTS_REMOVE, &row.to_string()));

    let command = single_command(surface.workbench_action(workbench::DNS_HOSTS_SAVE, ""));
    let CommandIntent::ApplyDnsSettings { patch } = command else {
        panic!("expected the shared DNS settings patch");
    };
    assert!(patch.clear_hosts);
    assert!(patch.hosts.is_none());
}

#[test]
fn the_hosts_editor_shows_the_typed_unavailable_reader() {
    let mut snapshot = workbench_snapshot();
    snapshot.dns_hosts = PageData::unavailable(unsupported("no profile-backed Hosts reader"));
    let surface = SurfaceHarness::new(Some(snapshot)).expect("headless surface");
    surface.open_page("Dns");

    let hosts = surface
        .workbenches()
        .dns_hosts
        .lock()
        .expect("the hosts reducer");
    assert!(hosts.read_failure().is_some());
    assert!(!hosts.can_apply());
}

// -- profiles-script-sandbox ---------------------------------------------

#[test]
fn the_script_sandbox_stays_unobserved_until_the_owner_publishes_a_report() {
    let mut snapshot = workbench_snapshot();
    snapshot.script_sandbox = None;
    let surface = SurfaceHarness::new(Some(snapshot)).expect("headless surface");
    surface.open_page("Editor");

    assert!(
        surface.workbench_item("script-sandbox-report").is_none(),
        "an unobserved sandbox must not show a report"
    );
    let item = surface
        .workbench_item("script-sandbox")
        .expect("the sandbox panel is projected");
    assert!(!item.status.is_empty());
}

#[test]
fn the_script_sandbox_failure_offers_the_retry_of_the_same_command() {
    let surface = workbench_surface("Editor");
    surface.set_workbench_draft("preset('remove-ads')");
    single_command(surface.workbench_action(workbench::SCRIPT_RUN, ""));

    refuse(&surface, &refusal("the sandbox refused the script"));

    let script = surface
        .workbenches()
        .script
        .lock()
        .expect("the script reducer");
    assert!(matches!(
        script.execution_phase(),
        ScriptExecutionPhase::Failed { .. }
    ));
    drop(script);

    let command = single_command(surface.workbench_action(workbench::SCRIPT_RETRY, ""));
    let CommandIntent::RunScriptSandbox { request } = command else {
        panic!("expected the shared script sandbox command");
    };
    assert_eq!(request.script_code, "preset('remove-ads')");
}

#[test]
fn the_script_sandbox_is_busy_while_the_owner_runs_the_script() {
    let surface = workbench_surface("Editor");
    surface.set_workbench_draft("preset('remove-ads')");
    single_command(surface.workbench_action(workbench::SCRIPT_RUN, ""));

    let failure = rejection(surface.workbench_action(workbench::SCRIPT_RUN, ""));

    assert_eq!(failure.code, ErrorCode::InvalidState);
    let script = surface
        .workbenches()
        .script
        .lock()
        .expect("the script reducer");
    assert!(script.busy());
}

#[test]
fn the_script_sandbox_loads_a_built_in_preset() {
    let surface = workbench_surface("Editor");

    local(surface.workbench_action(workbench::SCRIPT_PRESET, "remove-ads"));

    let script = surface
        .workbenches()
        .script
        .lock()
        .expect("the script reducer");
    assert_eq!(script.selected_preset(), Some("remove-ads"));
    assert!(script.script_code().contains("filter_nodes_by_regex"));
}

#[test]
fn the_script_sandbox_keeps_its_draft_when_the_language_changes() {
    let surface = workbench_surface("Editor");
    surface.set_workbench_draft("preset('remove-ads')");
    single_command(surface.workbench_action(workbench::SCRIPT_RUN, ""));
    let english = surface
        .workbench_item("script-sandbox")
        .expect("the sandbox panel is projected")
        .status
        .to_string();

    let mut chinese = workbench_snapshot();
    chinese.language_settings.preference = Some(LanguagePreference::SimplifiedChinese);
    surface.publish_snapshot(chinese);

    let translated = surface
        .workbench_item("script-sandbox")
        .expect("the sandbox panel is projected")
        .status
        .to_string();
    assert_ne!(english, translated, "the copy follows the language");
    let script = surface
        .workbenches()
        .script
        .lock()
        .expect("the script reducer");
    assert_eq!(script.script_code(), "preset('remove-ads')");
}

// -- profiles-script-export-review ---------------------------------------

#[test]
fn the_script_export_review_is_cancelled_without_writing_the_artifact() {
    let surface = workbench_surface("Editor");
    single_command(surface.workbench_action(workbench::SCRIPT_EXPORT_PREPARE, ""));
    surface.publish_owner_output(export_prepared());

    let command = single_command(surface.workbench_action(workbench::SCRIPT_EXPORT_CANCEL, ""));
    let CommandIntent::CancelScriptExport { identity } = command else {
        panic!("expected the shared script export command");
    };
    assert_eq!(identity.owner, 1);
}

#[test]
fn the_script_export_preparation_refusal_offers_the_retry() {
    let surface = workbench_surface("Editor");
    single_command(surface.workbench_action(workbench::SCRIPT_EXPORT_PREPARE, ""));

    refuse(&surface, &refusal("the artifact could not be composed"));

    let script = surface
        .workbenches()
        .script
        .lock()
        .expect("the script reducer");
    assert!(matches!(
        script.export_phase(),
        ScriptExportPhase::PrepareFailed { .. }
    ));
    drop(script);

    let command = single_command(surface.workbench_action(workbench::SCRIPT_RETRY, ""));
    assert!(matches!(command, CommandIntent::PrepareScriptExport { .. }));
}

#[test]
fn the_script_export_review_is_pending_while_the_owner_composes_the_artifact() {
    let surface = workbench_surface("Editor");
    single_command(surface.workbench_action(workbench::SCRIPT_EXPORT_PREPARE, ""));

    let failure = rejection(surface.workbench_action(workbench::SCRIPT_EXPORT_PREPARE, ""));

    assert_eq!(failure.code, ErrorCode::InvalidState);
}

#[test]
fn the_script_export_panel_shows_the_typed_unsupported_host() {
    let mut snapshot = workbench_snapshot();
    let mut export = fixture::script_export();
    export.outcome = ScriptExportOutcome::Unsupported {
        reason: "this host has no save port".to_owned(),
    };
    snapshot.script_export = Some(export);
    let surface = SurfaceHarness::new(Some(snapshot)).expect("headless surface");
    surface.open_page("Editor");

    let item = surface
        .workbench_item("script-export")
        .expect("the export panel is projected");
    assert!(item.status.contains("this host has no save port"));
}

#[test]
fn the_script_export_review_keeps_its_state_when_the_language_changes() {
    let surface = workbench_surface("Editor");
    single_command(surface.workbench_action(workbench::SCRIPT_EXPORT_PREPARE, ""));
    surface.publish_owner_output(export_prepared());
    let english = surface
        .workbench_item("script-export")
        .expect("the export panel is projected")
        .status
        .to_string();

    let mut chinese = workbench_snapshot();
    chinese.language_settings.preference = Some(LanguagePreference::SimplifiedChinese);
    surface.publish_snapshot(chinese);

    let translated = surface
        .workbench_item("script-export")
        .expect("the export panel is projected")
        .status
        .to_string();
    assert_ne!(english, translated, "the copy follows the language");
    let script = surface
        .workbenches()
        .script
        .lock()
        .expect("the script reducer");
    assert!(matches!(
        script.export_phase(),
        ScriptExportPhase::Review { .. }
    ));
}

// -- profiles-diff-viewer ------------------------------------------------

#[test]
fn the_configuration_difference_view_is_closed_without_the_owner() {
    let mut snapshot = workbench_snapshot();
    snapshot.yaml_ast_diff = Some(YamlAstDiffSnapshot::demo_fixture());
    let surface = SurfaceHarness::new(Some(snapshot)).expect("headless surface");
    surface.open_page("Editor");
    assert!(
        surface.workbench_item("config-diff-summary").is_some(),
        "the published difference is shown"
    );

    local(surface.workbench_action(workbench::DIFF_CLOSE, ""));

    assert!(surface.workbench_item("config-diff-summary").is_none());
}

// -- profiles-filter-editor ----------------------------------------------

#[test]
fn the_subscription_filter_draft_is_discarded_without_the_owner() {
    let surface = workbench_surface("Filters");
    local(surface.workbench_draft_edited("filter-include", "hk|jp"));

    local(surface.workbench_action(workbench::FILTER_CANCEL, ""));

    let filter = surface
        .workbenches()
        .filter
        .lock()
        .expect("the filter reducer");
    assert!(!filter.dirty());
    assert_eq!(filter.draft.include, "");
}

#[test]
fn the_subscription_filter_write_refusal_reaches_the_panel() {
    let surface = workbench_surface("Filters");
    local(surface.workbench_draft_edited("filter-include", "hk|jp"));
    single_command(surface.workbench_action(workbench::FILTER_SAVE, ""));

    refuse(&surface, &refusal("the filter write was refused"));

    let filter = surface
        .workbenches()
        .filter
        .lock()
        .expect("the filter reducer");
    assert!(filter.failure().is_some());
}

// -- profiles-aggregator-wizard / preview --------------------------------

#[test]
fn the_aggregation_wizard_drops_its_draft_without_the_owner() {
    let surface = workbench_surface("Filters");
    surface.set_workbench_draft_2("Merged");

    local(surface.workbench_action(workbench::AGGREGATE_CANCEL, ""));

    assert_eq!(surface.workbench_draft_2(), "");
}

#[test]
fn the_aggregation_wizard_refuses_to_create_a_profile_without_a_name() {
    let surface = workbench_surface("Filters");
    surface.set_workbench_draft_2("  ");

    let failure = rejection(surface.workbench_action(workbench::AGGREGATE_CREATE, ""));

    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

/// The reviewed export the owner publishes for the prepared artifact.
fn export_prepared() -> CommandOutput {
    let draft = ScriptExportDraft {
        kind: ScriptExportKind::DirectiveDslScript,
        validation_scope: ScriptExportValidationScope::Unscoped,
        profile: None,
        base_yaml: String::new(),
        mixin_yaml: String::new(),
        script_code: String::new(),
        preset: None,
    };
    let review = ScriptExportReview {
        identity: ScriptExportIdentity {
            owner: 1,
            sequence: 1,
            sha256: "c".repeat(64),
        },
        draft,
        snapshot: fixture::script_export(),
    };
    CommandOutput::ScriptExportPrepared(Box::new(review))
}
