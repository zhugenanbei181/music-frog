//! SLINT-001.8: Rule toggling, provider refresh, language, theme and accent
//! colour behaviour of the real Slint surface.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness. The theme whitelist and the hexadecimal colour parser are
//! owned by `infiltrator_slint::surface::action` and
//! `infiltrator_slint::theme_util`; these tests assert the exact typed owner
//! command the surface emits and the typed failure it raises.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::shell_sidebar_preference::SidebarExpansionIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{RulesPageSnapshot, SurfaceSnapshot};
use infiltrator_contract::system_proxy::SystemProxyRecoveryStatus;
use infiltrator_contract::theme::{SystemAppearance, ThemePreference, ThemeSkin};
use infiltrator_contract::theme_palette::{ResolvedThemePalette, SystemDynamicColor};
use infiltrator_slint::SlintAction;
use infiltrator_slint::surface::harness;
use infiltrator_slint::theme_util;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;
use serde_json::json;

fn desktop_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    let page: RulesPageSnapshot = serde_json::from_value(json!({
        "total_rules": 2,
        "default_action": "MATCH",
        "providers": [],
        "rules": [
            {
                "id": 0,
                "rule_type": "DOMAIN",
                "payload": "example.com",
                "proxy": "PROXIES",
                "raw": "DOMAIN,example.com,PROXIES",
                "is_enabled": true
            },
            {
                "id": 1,
                "rule_type": "DOMAIN",
                "payload": "disabled.example",
                "proxy": "PROXIES",
                "raw": "DOMAIN,disabled.example,PROXIES",
                "is_enabled": false
            }
        ]
    }))
    .expect("rules page JSON");
    snapshot.pages.rules = PageData::ready(page);
    snapshot
}

#[test]
fn the_sidebar_collapse_gesture_sends_the_one_typed_preference_intent() {
    // The surface owns no collapsed flag: the button sends the typed toggle to
    // the settings owner, and the owner readback decides the next shape.
    let surface = harness::SurfaceHarness::new(None).unwrap();
    match surface.toggle_sidebar_expansion() {
        harness::SurfaceOutcome::Dispatched(actions) => {
            assert!(
                actions.iter().any(|action| matches!(
                    action,
                    SlintAction::Command(CommandIntent::ApplySidebarExpansion {
                        intent: SidebarExpansionIntent::Toggle,
                    })
                )),
                "{actions:?}"
            );
        }
        other => panic!("the collapse gesture must reach the owner: {other:?}"),
    }
}

#[test]
fn the_published_theme_readback_drives_the_projected_theme() {
    let surface = harness::SurfaceHarness::new(None).unwrap();
    let mut snapshot = desktop_snapshot();
    snapshot.theme_readback = ResolvedThemePalette::resolve(
        ThemePreference::Fixed(ThemeSkin::Dark),
        SystemAppearance::Unobserved,
        SystemDynamicColor::Unobserved,
    );
    surface.publish_snapshot(snapshot);
    assert_eq!(
        surface.window().get_theme_mode(),
        infiltrator_slint::ThemeMode::Dark
    );
    assert!(surface.window().get_application_dark_mode());
}

/// The 网络设置 page used to print "当前版本：" with nothing after it and keep
/// its retry control permanently disabled: `PageNetwork` declared both inputs,
/// but nothing in the tree fed them. They are now plumbed from the owner's
/// typed facts, and this pins that the projection really reads them.
#[test]
fn the_network_page_consumes_the_owner_core_version_and_recovery_status() {
    let surface = harness::SurfaceHarness::new(None).unwrap();
    let mut snapshot = desktop_snapshot();
    snapshot.core.core_version = Some("v1.19.18".into());
    snapshot.system_proxy_recovery.status = SystemProxyRecoveryStatus::Failed {
        failure: Failure::new(ErrorCode::InvalidInput, "system proxy busy", true),
    };

    surface.publish_snapshot(snapshot.clone());
    assert_eq!(surface.window().get_core_version(), "v1.19.18");
    assert!(
        surface.window().get_proxy_recovery_pending(),
        "a failed recovery must offer the owner's retry command"
    );

    // A recovery the owner did not need to run is not something the surface
    // offers to retry.
    snapshot.system_proxy_recovery.status = SystemProxyRecoveryStatus::NotNeeded;
    surface.publish_snapshot(snapshot);
    assert!(
        !surface.window().get_proxy_recovery_pending(),
        "only a failed attempt may offer a retry"
    );
}

/// Asserts that the surface dispatched exactly one owner command.
fn single_command(outcome: harness::SurfaceOutcome) -> CommandIntent {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            assert_eq!(
                actions.len(),
                1,
                "expected one owner action, got {actions:?}"
            );
            match &actions[0] {
                SlintAction::Command(intent) => intent.clone(),
                other => panic!("expected an owner command, got {other:?}"),
            }
        }
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner command, got the failure {failure:?}")
        }
    }
}

/// Asserts that the surface refused the gesture with the given typed failure.
fn rejection(outcome: harness::SurfaceOutcome) -> Failure {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            panic!("expected a refusal, got the owner actions {actions:?}")
        }
        harness::SurfaceOutcome::Rejected { failure, actions } => {
            assert!(
                actions.is_empty(),
                "a refused gesture must not dispatch owner actions, got {actions:?}"
            );
            failure
        }
    }
}

#[test]
fn toggling_a_disabled_rule_dispatches_the_shared_toggle_command() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let command = single_command(surface.set_rule_enabled("1", true));
    assert_eq!(command, CommandIntent::ToggleRuleEnabled { index: 1 });
    assert_eq!(surface.rule_pending_index(), "1");
}

#[test]
fn toggling_a_rule_that_already_matches_dispatches_nothing() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    match surface.set_rule_enabled("0", true) {
        harness::SurfaceOutcome::Dispatched(actions) => {
            assert!(
                actions.is_empty(),
                "an observed rule that already matches must not ask the owner to change it, got {actions:?}"
            );
        }
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected a local no-op, got the failure {failure:?}")
        }
    }
    assert_eq!(surface.rule_pending_index(), "");
}

#[test]
fn toggling_an_unobserved_rule_is_refused_as_not_ready() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let failure = rejection(surface.set_rule_enabled("99", true));
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(failure.message, "Rule was not observed");
    assert_eq!(surface.notice(), "Rule was not observed");
}

#[test]
fn toggling_a_rule_with_an_invalid_identity_is_refused_as_invalid_input() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let failure = rejection(surface.set_rule_enabled("not-a-number", true));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "Invalid rule identity");
}

#[test]
fn updating_one_provider_refreshes_the_shared_provider_batch() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let command = single_command(surface.update_provider("my-provider"));
    assert_eq!(command, CommandIntent::RefreshRuleProviders);
}

#[test]
fn language_preference_dispatches_the_shared_language_command() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let command = single_command(surface.set_application_language("zh-CN"));
    assert_eq!(
        command,
        CommandIntent::SetLanguage {
            preference: LanguagePreference::SimplifiedChinese
        }
    );

    let command = single_command(surface.set_application_language("en"));
    assert_eq!(
        command,
        CommandIntent::SetLanguage {
            preference: LanguagePreference::English
        }
    );
}

#[test]
fn an_unsupported_language_is_refused_with_the_typed_failure() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let failure = rejection(surface.set_application_language("fr-FR"));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "unsupported language preference");
    assert_eq!(surface.notice(), "unsupported language preference");
}

#[test]
fn every_supported_theme_mode_dispatches_the_shared_theme_setting() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    for mode in ["system", "light", "dark", "forest", "amoled"] {
        let command = single_command(surface.set_theme_mode(mode));
        assert_eq!(
            command,
            CommandIntent::UpdateSetting {
                key: "theme".to_owned(),
                value: mode.to_owned()
            }
        );
    }
}

#[test]
fn an_unknown_theme_mode_is_refused_with_the_typed_failure() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let failure = rejection(surface.set_theme_mode("neon"));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "Unknown theme mode");
    assert_eq!(surface.notice(), "Unknown theme mode");
}

#[test]
fn a_valid_accent_colour_dispatches_the_shared_accent_setting() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let command = single_command(surface.set_theme_color("#016888"));
    assert_eq!(
        command,
        CommandIntent::UpdateSetting {
            key: "custom_accent_color".to_owned(),
            value: "#016888".to_owned()
        }
    );
    assert_eq!(surface.theme_color_code(), "#016888");
    assert_eq!(surface.theme_color_error(), "");
}

#[test]
fn an_invalid_accent_colour_is_refused_and_projected_to_the_colour_error() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let failure = rejection(surface.set_theme_color("#FFF"));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(
        failure.message,
        "Enter a six-digit hexadecimal color such as #016888"
    );
    assert_eq!(surface.theme_color_error(), failure.message);
    assert_eq!(surface.notice(), failure.message);
}

#[test]
fn the_accent_colour_parser_is_the_shared_surface_parser() {
    // The surface, the settings callback and this test share one parser, so a
    // colour that the parser rejects can never reach the owner as a setting.
    let parsed = theme_util::parse_color("#016888").expect("six digit colour");
    assert_eq!(parsed, slint::Color::from_rgb_u8(0x01, 0x68, 0x88));
    assert_eq!(
        theme_util::parse_color("016888"),
        Some(slint::Color::from_rgb_u8(0x01, 0x68, 0x88))
    );
    assert_eq!(theme_util::parse_color("#FFF"), None);
    assert_eq!(theme_util::parse_color("#GGGGGG"), None);
    assert_eq!(theme_util::parse_color(""), None);
}

#[test]
fn the_contrast_rule_is_the_shared_surface_rule() {
    assert!(!theme_util::contrast_is_light("#016888"));
    assert!(theme_util::contrast_is_light("#F0F0F0"));
}

#[test]
fn the_rule_rows_come_from_the_shared_snapshot_projection() {
    let mut snapshot = desktop_snapshot();
    let page: RulesPageSnapshot = serde_json::from_value(json!({
        "total_rules": 1,
        "default_action": "MATCH",
        "providers": [],
        "rules": [
            {
                "id": 7,
                "rule_type": "DOMAIN",
                "payload": "example.com",
                "proxy": "PROXIES",
                "raw": "DOMAIN,example.com,PROXIES",
                "is_enabled": true
            }
        ]
    }))
    .expect("rules page JSON");
    snapshot.pages.rules = PageData::ready(page);

    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot.clone());

    let rows = surface.rule_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "7");
    assert_eq!(rows[0].title, "DOMAIN,example.com,PROXIES");
}

#[test]
fn the_rule_search_narrows_the_published_window_and_leaves_an_empty_result() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    // A keyword that matches one rule keeps exactly that rule.
    match surface.search_rules("example.com") {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "the search narrows the published window and must not ask the owner for anything, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the search to run, got the failure {failure:?}")
        }
    }
    let rows = surface.rule_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "0");
    assert_eq!(rows[0].title, "DOMAIN,example.com,PROXIES");

    // A keyword that matches nothing leaves the published window empty.
    match surface.search_rules("no-such-rule") {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "the search narrows the published window and must not ask the owner for anything, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the search to run, got the failure {failure:?}")
        }
    }
    assert!(surface.rule_rows().is_empty());
}

/// MUSICFROG-001: the hero is driven by the typed motion projection, not by a
/// private `is_playing` flag in the .slint tree. A user cue starts a flourish
/// with notes immediately, and the owner's readback settles it.
#[test]
fn the_mascot_answers_a_start_cue_with_notes_then_follows_the_owner_readback() {
    use infiltrator_contract::mascot_motion;
    use infiltrator_contract::mascot_motion::MotionCue;
    use infiltrator_presentation::mascot_motion_projection;
    use infiltrator_presentation::mascot_motion_projection::project_mascot;
    use infiltrator_slint::HeroGlyph;
    use infiltrator_slint::MascotMotionSpec;
    use infiltrator_slint::MascotPhase;
    use lurker_core_contract::lifecycle::CoreLifecycle;
    use slint::Global;

    let surface = harness::SurfaceHarness::new(None).unwrap();
    let mut state = mascot_motion_projection::MascotMotionState::default();
    let starting = project_mascot(
        &mut state,
        CoreLifecycle::Starting,
        Some(MotionCue::StartRequested),
        100,
        false,
    );
    assert!(
        starting.note_count > 0,
        "a tap must release notes immediately"
    );

    // The UI driver's mapping lands in the window's one projected spec.
    let ui = infiltrator_slint::MascotMotionState::get(surface.window());
    assert_eq!(
        ui.get_current().note_count,
        0,
        "no cue was sent to the window yet"
    );
    ui.set_current(MascotMotionSpec {
        phase: MascotPhase::Starting,
        sway: starting.sway,
        pulse: starting.pulse,
        note_count: starting.note_count as i32,
        note_1: starting.note_progress[0],
        note_2: starting.note_progress[1],
        note_3: starting.note_progress[2],
        glyph: HeroGlyph::Pause,
    });
    assert_eq!(ui.get_current().glyph, HeroGlyph::Pause);
    assert!(ui.get_current().note_count > 0);
    assert!(
        ui.get_is_running(),
        "the hero control shows the target state"
    );

    let running = project_mascot(&mut state, CoreLifecycle::Running, None, 600, false);
    assert_eq!(running.control_glyph, mascot_motion::HeroGlyph::Pause);
    assert_eq!(running.state, mascot_motion::MascotMotion::Running);
}
