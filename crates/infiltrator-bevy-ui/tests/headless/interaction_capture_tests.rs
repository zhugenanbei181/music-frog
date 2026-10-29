//! test-intent: behavior
//! The Bevy interaction capture performs the business's own gesture, and the
//! owner's readback proves the interaction really happened.
//!
//! These tests drive the very capture layer the screenshot rig drives, so a
//! capture that only repaints the page (or that panics while activating)
//! fails here before it can ever reach the pixel judge.
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ui::widget::Text;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::interaction_capture::InteractionCapture;
use infiltrator_bevy_ui::pages::business_panel::{BusinessPanelState, PanelKind};
use infiltrator_bevy_ui::pages::dns_leak_actions::LeakActions;
use infiltrator_bevy_ui::pages::logs_search::LogsViewState;
use infiltrator_bevy_ui::pages::page_models::DoctorPageModelState;
use infiltrator_bevy_ui::pages::page_models::RulesPageModelState;
use infiltrator_bevy_ui::pages::profiles_script_workbench::ScriptWorkbenchState;
use infiltrator_bevy_ui::pages::proxies_form::CustomNodeForm;
use infiltrator_bevy_ui::pages::rules_outcome::RuleProviderFailureText;
use infiltrator_bevy_ui::pages::rules_view::RulesViewState;
use infiltrator_bevy_ui::pages::settings_language::LanguageChoiceResource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::LatestSurfaceSnapshot;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::app_routing::AppRoutingRule;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::dns_leak::DnsLeakOperation;
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::profile_document::DocumentInspection;
use infiltrator_contract::shell_readout::ShellObservation;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use infiltrator_presentation::language_choice::{LanguageChoicePhase, LanguageReadPhase};
use infiltrator_presentation::script_workbench::ScriptExportPhase;
use infiltrator_test_support::version_fixtures::CAPTURE_ROLLBACK_VERSION;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::page::PageStatus;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_ui_presentation::dns_leak_actions::DnsLeakActionPhase;
use std::sync::Arc;

/// A recording-only sink. The capture installs its own command sink; this one
/// only keeps the shared command pump's terminal-feedback path registered.
struct SilentSink;

impl UiCommandSink for SilentSink {
    fn submit(&self, _command: UiCommand) {}
    fn submit_tracked(&self, _command: UiCommand) -> Option<RequestId> {
        None
    }
}

/// The capture composition: the demo pages, the shared command pump and the
/// capture itself, mounted on the business's own page.
fn capture_app(feature: FeatureId, route: Route) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(Arc::new(SilentSink)));
    InteractionCapture::install(&mut app, feature);
    app.update();
    app.world_mut().commands().trigger(RouteChanged(route));
    app.update();
    app
}

/// Drive the capture's own activation until it settles or the budget is spent.
fn drive(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// The logs capture starts its isolated Core and publishes the real log
/// records. Before the fix the capture panicked here: the isolated owner had
/// no lifecycle capability matrix, so the shared gate denied `StartCore`.
#[test]
fn capture_logs_starts_the_isolated_core_and_publishes_the_real_log_records() {
    let mut app = capture_app(FeatureId::LogsSearchHighlight, Route::Logs);
    drive(&mut app, 8);
    let view = app.world().resource::<LogsViewState>();
    assert_eq!(
        view.0.source_count(),
        3,
        "the capture published the isolated log session's own records"
    );
    assert_eq!(
        view.0.matched_count(),
        3,
        "every isolated record is visible before the capture types the query"
    );
    assert_eq!(
        app.world().resource::<InteractionCapture>().feature,
        FeatureId::LogsSearchHighlight
    );
}

/// The telemetry capture reads the real telemetry port through the isolated
/// owner. Before the fix the capture panicked here for the same missing
/// lifecycle capability matrix.
#[test]
fn capture_telemetry_reads_the_real_stale_zero_transition() {
    let mut app = capture_app(FeatureId::RuntimeTelemetryObservation, Route::Overview);
    drive(&mut app, 8);
    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    assert!(
        matches!(
            &latest.0.shell_readout.upload_bps,
            ShellObservation::Stale(value) if *value == 0.0
        ),
        "the denied telemetry read left the owner's stale zero: {:?}",
        latest.0.shell_readout.upload_bps
    );
    assert!(
        latest.0.shell_readout.rate_failure.is_some(),
        "the owner reports the denied telemetry read"
    );
}

/// The rollback capture must open the review first, submit the destructive
/// command through the page's confirmation control, and then publish the
/// version owner's changed selection rather than the fixture's initial values.
#[test]
fn capture_core_rollback_confirms_and_publishes_the_owner_readback() {
    let mut app = capture_app(FeatureId::CoreVersionRollbackConfirm, Route::Settings);
    drive(&mut app, 12);

    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    assert_eq!(
        latest.0.versions.rollback.current.as_deref(),
        Some(CAPTURE_ROLLBACK_VERSION),
        "the owner readback must show the selected rollback version as active"
    );
    assert_eq!(
        latest.0.versions.rollback.target, None,
        "the previous target is no longer a rollback candidate after activation"
    );
    assert_eq!(
        app.world().resource::<InteractionCapture>().feature,
        FeatureId::CoreVersionRollbackConfirm
    );
}

/// The DNS leak capture submits the shared `TestDnsLeak` command through the
/// real command bus. Before the fix the capture's command application had no
/// host capability provider, so the shared host gate denied the probe.
#[test]
fn capture_dns_leak_probe_runs_the_shared_command_and_reports_the_real_verdict() {
    let mut app = capture_app(FeatureId::DnsLeakAlert, Route::Dns);
    drive(&mut app, 12);
    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    assert_eq!(
        latest.0.dns_leak.operation,
        DnsLeakOperation::Completed,
        "the shared probe really ran"
    );
    assert!(
        latest.0.dns_leak.conclusion().is_divergent(),
        "the isolated echo really produced two distinct resolver identities"
    );
    assert_eq!(latest.0.dns_leak.observations.len(), 2);
    assert!(
        matches!(
            app.world().resource::<LeakActions>().state.phase,
            DnsLeakActionPhase::Idle
        ),
        "the probe settled"
    );
}

/// The doctor capture submits the shared `RunDoctorDiagnostics` command. Before
/// the fix the capture's command application had no host capability provider,
/// so the shared host gate denied the diagnosis.
#[test]
fn capture_doctor_diagnosis_reaches_the_real_permission_failure() {
    let mut app = capture_app(FeatureId::DoctorFailureRecovery, Route::Doctor);
    drive(&mut app, 12);
    // The doctor phase now lives in the shared page model.
    let actions = app.world().resource::<DoctorPageModelState>();
    match &actions.0.action().phase {
        DoctorActionPhase::Failed { failure, .. } => {
            assert_eq!(
                failure.code,
                ErrorCode::Permission,
                "the isolated port denied the diagnosis"
            );
        }
        DoctorActionPhase::Idle | DoctorActionPhase::Pending(_) => {
            panic!("the diagnosis must reach its terminal failure")
        }
    }
}

/// The language capture saves through the shared `SetLanguage` command and
/// reads the persisted preference back. Before the fix the capture never
/// republished the stored settings, so the owner stayed in its
/// `AwaitingObservation` phase and never settled.
#[test]
fn capture_language_choice_saves_and_reads_the_persisted_preference_back() {
    let mut app = capture_app(FeatureId::SettingsLanguageChoice, Route::Settings);
    drive(&mut app, 12);
    let choice = app.world().resource::<LanguageChoiceResource>();
    assert_eq!(
        choice.model.read,
        LanguageReadPhase::Observed {
            preference: Some(LanguagePreference::English)
        },
        "the owner reads the persisted preference back"
    );
    assert_eq!(
        choice.model.phase,
        LanguageChoicePhase::Ready,
        "the language choice settled"
    );
    assert_eq!(
        app.world().resource::<UiLocale>().code(),
        "en-US",
        "the surface really switched to the saved language"
    );
}

/// The URI import capture opens the real custom-node panel, types the share
/// link into the native field and activates the real import control. Before
/// the fix the capture never republished the decoded studio, so the editor's
/// readback fence never accepted the imported draft.
#[test]
fn capture_uri_import_publishes_the_decoded_draft_as_the_owner_studio() {
    let mut app = capture_app(FeatureId::ProxiesUriImportPreview, Route::Proxies);
    drive(&mut app, 16);
    let form = app.world().resource::<CustomNodeForm>();
    let draft = form
        .studio
        .draft
        .as_ref()
        .expect("the shared codec published the decoded draft");
    assert_eq!(draft.name, "Imported-Node");
    assert!(
        form.studio.uri_preview.is_some(),
        "the owner studio carries the real share link"
    );
    assert!(
        form.studio.last_error.is_none(),
        "the import really succeeded: {:?}",
        form.studio.last_error
    );
}

/// The business panel capture activates the panel's own launcher control, so
/// the modal really opens through the surface's own observer.
#[test]
fn capture_business_panel_opens_the_panel_the_business_is_about() {
    let mut diff = capture_app(FeatureId::ProfilesDiffViewer, Route::Profiles);
    drive(&mut diff, 8);
    assert_eq!(
        diff.world().resource::<BusinessPanelState>().0,
        Some(PanelKind::SnapshotDiff),
        "the capture opened the snapshot diff viewer"
    );
    let mut wizard = capture_app(FeatureId::ProfilesAggregatorWizard, Route::Profiles);
    drive(&mut wizard, 8);
    assert_eq!(
        wizard.world().resource::<BusinessPanelState>().0,
        Some(PanelKind::Aggregator),
        "the capture opened the aggregation wizard"
    );
}

/// The rules search capture types the keyword into the list's own search
/// field, and the shared filter reduction really narrows the mounted rows.
#[test]
fn capture_rules_search_types_the_keyword_and_the_shared_filter_narrows_the_list() {
    let mut app = capture_app(FeatureId::RulesSearchHighlight, Route::Rules);
    drive(&mut app, 8);
    let view = app.world().resource::<RulesViewState>();
    println!(
        "DEBUG filter={:?} filtered={:?} for={:?}",
        app.world()
            .resource::<RulesPageModelState>()
            .0
            .filter_text(),
        view.filtered_indices,
        view.filtered_for
    );
    assert_eq!(
        view.filtered_indices.as_slice(),
        &[1usize][..],
        "the shared filter kept only the matching rule"
    );
}

/// The app-routing capture activates the row's own policy control, so the
/// shared `SetAppRule` command really reaches the routing owner and the
/// owner's readback comes back on the page's own projection.
#[test]
fn capture_app_routing_policy_editor_writes_the_rule_through_the_shared_command() {
    let mut app = capture_app(FeatureId::AppRoutingPolicyEditor, Route::AppRouting);
    drive(&mut app, 12);
    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    let page = latest
        .0
        .pages
        .app_routing
        .data
        .as_ref()
        .expect("the app-routing page carries the owner's readback");
    let rule = page
        .apps
        .iter()
        .find(|app| app.id == "app-1")
        .map(|app| app.rule);
    assert_eq!(
        rule,
        Some(AppRoutingRule::Direct),
        "the shared SetAppRule command really persisted the owner's rule"
    );
}

/// The lifecycle recovery capture activates the doctor page's own bootstrap
/// control, so the shared `BootstrapDoctor` command really reaches the
/// diagnostic owner and the panel really reports the terminal answer.
#[test]
fn capture_lifecycle_recovery_panel_runs_the_shared_bootstrap_command() {
    let mut app = capture_app(FeatureId::LifecycleRecoveryPanel, Route::Doctor);
    drive(&mut app, 16);
    let actions = app.world().resource::<DoctorPageModelState>();
    match &actions.0.action().phase {
        DoctorActionPhase::Failed {
            failure,
            retry_action,
        } => {
            assert_eq!(
                *retry_action,
                DoctorAction::Bootstrap,
                "the failed action is the lifecycle recovery itself"
            );
            assert!(
                !failure.message.is_empty(),
                "the owner reported why the recovery failed"
            );
        }
        _ => panic!("the shared bootstrap command must reach its terminal answer"),
    }
    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    assert!(
        matches!(latest.0.pages.doctor.status, PageStatus::Failed { .. }),
        "the owner published the failed recovery readback"
    );
}

/// The script sandbox capture activates the workbench's own preset and run
/// controls, so the shared `RunScriptSandbox` command really runs the script
/// owner and the console really shows the run's own rows.
#[test]
fn capture_script_sandbox_runs_the_shared_script_command() {
    let mut app = capture_app(FeatureId::ProfilesScriptSandbox, Route::Profiles);
    drive(&mut app, 16);
    let state = app.world().resource::<ScriptWorkbenchState>();
    let snapshot = state
        .model
        .snapshot()
        .expect("the shared script owner really ran the selected preset");
    assert_eq!(
        snapshot.selected_preset.as_deref(),
        Some("auto-country-groups"),
        "the capture really selected the catalogue preset"
    );
    assert_eq!(
        snapshot.hook_stage, "pre_merge",
        "the shared engine really ran the preset at its declared stage"
    );
    assert!(
        !state.rows.is_empty(),
        "the workbench console really shows the run's own rows"
    );
    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    assert!(
        latest.0.script_sandbox.is_some(),
        "the owner published the sandbox readback"
    );
}

/// The script export capture activates the workbench's own export control, so
/// the shared `PrepareScriptExport` command really reaches the export owner
/// and the review really carries the owner's artifact bytes.
#[test]
fn capture_script_export_review_prepares_the_shared_export_review() {
    let mut app = capture_app(FeatureId::ProfilesScriptExportReview, Route::Profiles);
    drive(&mut app, 16);
    let state = app.world().resource::<ScriptWorkbenchState>();
    assert!(
        matches!(state.model.export_phase(), ScriptExportPhase::Review { .. }),
        "the shared export owner really prepared the review"
    );
    assert!(
        state.model.is_export_visible(),
        "the workbench really opened the export review"
    );
    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    let export = latest
        .0
        .script_export
        .as_ref()
        .expect("the owner published the export readback");
    assert!(
        !export.content.is_empty(),
        "the review carries the owner's real artifact bytes"
    );
    assert!(
        !export.file_name.is_empty(),
        "the review carries the owner's real file name"
    );
}

/// The rule provider refresh capture activates the provider row's refresh control,
/// submits the shared `RefreshRuleProvider` command and renders the owner's typed refusal.
#[test]
fn capture_rules_provider_refresh_error_renders_failure_line() {
    let mut app = capture_app(FeatureId::RulesProviderRefreshError, Route::Rules);
    drive(&mut app, 16);
    let failures = app
        .world_mut()
        .query::<(Entity, &RuleProviderFailureText, &Text)>()
        .iter(app.world())
        .find(|(_, marker, _)| marker.0 == 0)
        .map(|(_, _, text)| text.0.clone())
        .expect("provider 0 failure text mounted");
    assert!(
        failures.contains("the rule provider host refused"),
        "the failure line must render the owner's typed refusal: {failures}"
    );
    assert_eq!(
        app.world().resource::<InteractionCapture>().feature,
        FeatureId::RulesProviderRefreshError
    );
}

/// The snippet insertion capture activates the profile editor's snippet button.
#[test]
fn capture_editor_snippet_insertion_activates_snippet_button() {
    let mut app = capture_app(FeatureId::EditorSnippetInsertion, Route::Profiles);
    drive(&mut app, 16);
    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    assert!(
        latest.0.profile_editor.document.is_some(),
        "the profile editor document must be mounted"
    );
    assert_eq!(
        app.world().resource::<InteractionCapture>().feature,
        FeatureId::EditorSnippetInsertion
    );
}

/// The YAML error navigation capture renders the syntax error diagnostic on the profile editor.
#[test]
fn capture_editor_yaml_error_navigation_renders_syntax_diagnostic() {
    let mut app = capture_app(FeatureId::EditorYamlErrorNavigation, Route::Profiles);
    drive(&mut app, 16);
    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    let doc = latest
        .0
        .profile_editor
        .document
        .as_ref()
        .expect("profile document must be mounted");
    assert!(
        matches!(
            &doc.inspection,
            DocumentInspection::Inspected(DocumentValidation::Invalid { .. })
        ),
        "document inspection must report syntax error"
    );
    assert_eq!(
        app.world().resource::<InteractionCapture>().feature,
        FeatureId::EditorYamlErrorNavigation
    );
}
