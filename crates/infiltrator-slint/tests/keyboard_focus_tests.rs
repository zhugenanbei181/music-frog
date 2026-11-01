//! Scenario tests for Slint keyboard focus, window cadence tracking,
//! IME composition guarding, and Tab/Esc/Enter navigation (FLPARITY-010).

use infiltrator_contract::cadence::RenderCadence;
use infiltrator_contract::ime::ImeCompositionAction;
use infiltrator_contract::ime::ImeCompositionEvent;
use infiltrator_contract::ime::ImeCursorRect;
use infiltrator_contract::ime::ImeCursorSource;
use infiltrator_contract::ime::ImeFocusPlan;
use infiltrator_contract::ime::ImePhase;
use infiltrator_contract::ime::ImeViewport;
use infiltrator_contract::shortcuts::KeyModifiers;
use infiltrator_contract::shortcuts::ShortcutAction;
use infiltrator_slint::surface::focus_ime::KeyDispatchOutcome;
use infiltrator_slint::surface::focus_ime::cursor_support;
use infiltrator_slint::surface::harness::SurfaceHarness;

#[test]
fn the_shell_tracks_window_focus_for_cadence() {
    let harness = SurfaceHarness::new(None).expect("surface harness");
    assert!(harness.window().get_window_focused());

    let cadence_inactive = harness.set_window_focus(false);
    assert_eq!(cadence_inactive, RenderCadence::Background);
    assert!(!harness.window().get_window_focused());
    assert_eq!(cadence_inactive.frame_time_ms(), 500);

    let cadence_active = harness.set_window_focus(true);
    assert_eq!(cadence_active, RenderCadence::Active);
    assert!(harness.window().get_window_focused());
    assert_eq!(cadence_active.frame_time_ms(), 16);
}

#[test]
fn ime_composition_guards_global_shortcuts() {
    let harness = SurfaceHarness::new(None).expect("surface harness");
    assert_eq!(harness.ime_phase(), ImePhase::Idle);

    // When idle, global shortcut chords resolve normally.
    let outcome = harness.dispatch_key("k", KeyModifiers::ctrl());
    assert_eq!(
        outcome,
        KeyDispatchOutcome::Shortcut(ShortcutAction::OpenCommandPalette)
    );

    let proxy_outcome = harness.dispatch_key("P", KeyModifiers::ctrl_alt());
    assert_eq!(
        proxy_outcome,
        KeyDispatchOutcome::Shortcut(ShortcutAction::ToggleSystemProxy)
    );

    // Begin IME composition (e.g. typing Chinese characters).
    let begin_action = harness.feed_ime_event(ImeCompositionEvent::Opened);
    assert_eq!(begin_action, ImeCompositionAction::Begin);
    assert_eq!(harness.ime_phase(), ImePhase::Composing);

    let preedit_action = harness.feed_ime_event(ImeCompositionEvent::Preedit("k".to_owned()));
    assert_eq!(
        preedit_action,
        ImeCompositionAction::UpdatePreedit("k".to_owned())
    );

    // During active composition, keystrokes belong to the IME and must NEVER trigger chords.
    let composing_outcome = harness.dispatch_key("k", KeyModifiers::ctrl());
    assert_eq!(composing_outcome, KeyDispatchOutcome::ConsumedByIme);

    let composing_proxy = harness.dispatch_key("P", KeyModifiers::ctrl_alt());
    assert_eq!(composing_proxy, KeyDispatchOutcome::ConsumedByIme);

    // Commit IME input.
    let commit_action = harness.feed_ime_event(ImeCompositionEvent::Commit("快照".to_owned()));
    assert_eq!(
        commit_action,
        ImeCompositionAction::Commit("快照".to_owned())
    );
    assert_eq!(harness.ime_phase(), ImePhase::Idle);

    // Once committed and idle again, shortcuts resolve.
    let idle_outcome = harness.dispatch_key("k", KeyModifiers::ctrl());
    assert_eq!(
        idle_outcome,
        KeyDispatchOutcome::Shortcut(ShortcutAction::OpenCommandPalette)
    );
}

#[test]
fn tab_navigation_and_escape_enter_handling() {
    let harness = SurfaceHarness::new(None).expect("surface harness");
    harness.set_focusable_count(3);
    assert_eq!(harness.current_focus_index(), 0);

    // Tab navigates forward.
    let step1 = harness.dispatch_key("Tab", KeyModifiers::default());
    assert_eq!(
        step1,
        KeyDispatchOutcome::FocusCycle {
            reverse: false,
            next_index: 1,
        }
    );
    assert_eq!(harness.current_focus_index(), 1);

    let step2 = harness.dispatch_key("Tab", KeyModifiers::default());
    assert_eq!(
        step2,
        KeyDispatchOutcome::FocusCycle {
            reverse: false,
            next_index: 2,
        }
    );
    assert_eq!(harness.current_focus_index(), 2);

    // Wrap around to beginning.
    let step3 = harness.dispatch_key("Tab", KeyModifiers::default());
    assert_eq!(
        step3,
        KeyDispatchOutcome::FocusCycle {
            reverse: false,
            next_index: 0,
        }
    );
    assert_eq!(harness.current_focus_index(), 0);

    // Shift+Tab navigates backward and wraps.
    let step_back = harness.dispatch_key("Tab", KeyModifiers::shift());
    assert_eq!(
        step_back,
        KeyDispatchOutcome::FocusCycle {
            reverse: true,
            next_index: 2,
        }
    );
    assert_eq!(harness.current_focus_index(), 2);

    // Escape and Enter semantics.
    assert_eq!(
        harness.dispatch_key("Escape", KeyModifiers::default()),
        KeyDispatchOutcome::Escape
    );
    assert_eq!(
        harness.dispatch_key("Return", KeyModifiers::default()),
        KeyDispatchOutcome::Enter
    );
    assert_eq!(
        harness.dispatch_key("Enter", KeyModifiers::default()),
        KeyDispatchOutcome::Enter
    );
}

#[test]
fn the_slint_surface_reports_toolkit_provided_ime_cursor_source() {
    let support = cursor_support();
    assert_eq!(
        support.source(),
        Some(ImeCursorSource::ToolkitProvided),
        "Slint text widgets manage their caret directly"
    );
    assert!(support.is_hosted());

    let harness = SurfaceHarness::new(None).expect("surface harness");
    let viewport = ImeViewport::new(800.0, 600.0);

    // When no cursor area is set, plan is DISABLED.
    assert_eq!(harness.ime_focus_plan(viewport), ImeFocusPlan::DISABLED);

    // With active caret, plan is enabled with clamped rect.
    let caret = ImeCursorRect::new(100.0, 200.0, 2.0, 16.0);
    harness.set_ime_cursor_rect(Some(caret));
    let plan = harness.ime_focus_plan(viewport);
    assert_eq!(plan.cursor(), Some(caret));
}
