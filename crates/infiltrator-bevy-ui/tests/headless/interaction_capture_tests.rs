//! test-intent: behavior
//! The Bevy interaction capture performs the business's own gesture, and the
//! owner's readback proves the interaction really happened.
//!
//! These tests drive the very capture layer the screenshot rig drives, so a
//! capture that only repaints the page (or that panics while activating)
//! fails here before it can ever reach the pixel judge.
use crate::support::headless_plugins;
use bevy::app::App;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::interaction_capture::InteractionCapture;
use infiltrator_bevy_ui::pages::business_panel::{BusinessPanelState, PanelKind};
use infiltrator_bevy_ui::pages::dns_leak_actions::LeakActions;
use infiltrator_bevy_ui::pages::logs_search::LogsViewState;
use infiltrator_bevy_ui::pages::page_models::DoctorPageModelState;
use infiltrator_bevy_ui::pages::proxies_form::CustomNodeForm;
use infiltrator_bevy_ui::pages::rules_view::RulesViewState;
use infiltrator_bevy_ui::pages::settings_language::LanguageChoiceResource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::LatestSurfaceSnapshot;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::dns_leak::DnsLeakOperation;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::shell_readout::ShellObservation;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use infiltrator_presentation::language_choice::{LanguageChoicePhase, LanguageReadPhase};
use lurker_core_contract::error::ErrorCode;
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
    assert_eq!(
        view.filtered_indices.as_slice(),
        &[1usize][..],
        "the shared filter kept only the matching rule"
    );
}
