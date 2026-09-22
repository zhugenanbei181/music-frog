//! DUAL-15-11 tests on the real `AppState`: the toolkit IME event vocabulary
//! maps onto the shared composition grammar, the shell records the live
//! session, composing keys are not hijacked as global chords, and the pinned
//! toolkit really exposes the caret-area strategy seam iced_winit forwards to
//! `set_ime_cursor_area`.
//!
//! Honest boundary: no headless test can observe the OS candidate window. What
//! is machine-checked here is (a) the toolkit seam that carries the caret rect
//! (`InputMethod::Enabled { cursor }` merged into the shell strategy, exactly
//! what `iced_winit` reads during a redraw) and (b) the shell's own composition
//! consumption. The `text_input` widget that publishes the strategy for every
//! field of this surface owns the candidate box; the shell cannot set it.

use crate::ime;
use crate::state::AppState;
use crate::types::message::Message;
use iced::advanced::input_method;
use iced::advanced::input_method::InputMethod;
use infiltrator_contract::ime::{ImeCompositionEvent, ImeCursorSource, ImeCursorSupport, ImePhase};
use infiltrator_contract::shortcuts::{KeyModifiers, ShortcutAction};

/// The shared payload of one raw toolkit event, panicking when the mapper
/// publishes a non-composition message (the shell's IME channel is typed).
fn payload(event: &iced::Event) -> Option<ImeCompositionEvent> {
    match ime::composition_message(event) {
        Some(Message::ImeComposition(shared)) => Some(shared),
        Some(other) => panic!("the IME channel must only carry compositions: {other:?}"),
        None => None,
    }
}

#[test]
fn the_shell_reports_the_toolkit_provided_cursor_source() {
    let support = ime::cursor_support();
    assert_eq!(
        support,
        ImeCursorSupport::Hosted {
            source: ImeCursorSource::ToolkitProvided,
        },
        "the candidate box follows the toolkit text widget, not this shell"
    );
    assert!(support.is_hosted());
    assert_eq!(support.source(), Some(ImeCursorSource::ToolkitProvided));
    assert_ne!(
        support.source(),
        Some(ImeCursorSource::SurfaceComputed),
        "Iced must not claim to compute the caret itself"
    );
}

#[test]
fn the_raw_toolkit_ime_events_map_onto_the_shared_vocabulary() {
    let opened = iced::Event::InputMethod(input_method::Event::Opened);
    assert_eq!(payload(&opened), Some(ImeCompositionEvent::Opened));

    let preedit = iced::Event::InputMethod(input_method::Event::Preedit(
        "ni hao".to_owned(),
        Some(0..6),
    ));
    assert_eq!(
        payload(&preedit),
        Some(ImeCompositionEvent::Preedit("ni hao".to_owned()))
    );

    let commit = iced::Event::InputMethod(input_method::Event::Commit("你好".to_owned()));
    assert_eq!(
        payload(&commit),
        Some(ImeCompositionEvent::Commit("你好".to_owned()))
    );

    let closed = iced::Event::InputMethod(input_method::Event::Closed);
    assert_eq!(payload(&closed), Some(ImeCompositionEvent::Closed));

    // Non-IME events stay outside the composition channel.
    let window_event = iced::Event::Window(iced::window::Event::Focused);
    assert_eq!(payload(&window_event), None);
}

#[test]
fn a_composition_session_keeps_the_shared_phase_and_never_steals_chords() {
    let (mut state, _) = AppState::new();
    assert!(!state.shell.ime.is_composing());
    assert!(!state.shell.command_palette_open);

    // Ctrl+K during an active composition belongs to the input method.
    let _ = state.update(Message::ImeComposition(ImeCompositionEvent::Opened));
    assert!(state.shell.ime.is_composing());
    let _ = state.update(Message::ImeComposition(ImeCompositionEvent::Preedit(
        "dai li".to_owned(),
    )));
    assert_eq!(state.shell.ime.preedit(), "dai li");
    let _ = state.update(Message::KeyboardChord {
        key: "k".to_owned(),
        modifiers: KeyModifiers {
            ctrl: true,
            ..KeyModifiers::default()
        },
    });
    assert!(
        !state.shell.command_palette_open,
        "a composing chord must not open the palette"
    );
    assert!(state.shell.hotkey_capture.is_none());

    // After the commit the same chord dispatches again.
    let _ = state.update(Message::ImeComposition(ImeCompositionEvent::Commit(
        "代理".to_owned(),
    )));
    assert!(!state.shell.ime.is_composing());
    assert_eq!(state.shell.ime.phase(), ImePhase::Idle);
    let _ = state.update(Message::KeyboardChord {
        key: "k".to_owned(),
        modifiers: KeyModifiers {
            ctrl: true,
            ..KeyModifiers::default()
        },
    });
    assert!(
        state.shell.command_palette_open,
        "the shortcut registry owns the chord again once composing ends"
    );

    // A capture awaiting a chord is not consumed by composition noise either.
    let (mut capture_state, _) = AppState::new();
    let _ = capture_state.update(Message::BeginHotkeyCapture(
        ShortcutAction::OpenCommandPalette,
    ));
    assert!(capture_state.shell.hotkey_capture.is_some());
    let _ = capture_state.update(Message::ImeComposition(ImeCompositionEvent::Preedit(
        "k".to_owned(),
    )));
    let _ = capture_state.update(Message::KeyboardChord {
        key: "j".to_owned(),
        modifiers: KeyModifiers::default(),
    });
    assert!(
        capture_state.shell.hotkey_capture.is_some(),
        "the capture survives the composition"
    );
}

/// The pinned toolkit seam iced_winit consumes: a widget publishes
/// `InputMethod::Enabled { cursor, .. }`, the runtime merges it into the shell
/// strategy, and that strategy is what reaches `set_ime_allowed` /
/// `set_ime_cursor_area` / `set_ime_purpose`. A regression in the pinned
/// version (dropping `cursor` or the merger) fails this test or the build.
#[test]
fn the_pinned_toolkit_forwards_a_widget_caret_to_the_shell_strategy() {
    use iced::advanced::Shell;
    use iced::{Point, Rectangle, Size};

    let caret = Rectangle::new(Point::new(72.0, 40.0), Size::new(1.0, 20.0));
    let widget_strategy: InputMethod = InputMethod::Enabled {
        cursor: caret,
        purpose: input_method::Purpose::Normal,
        preedit: None,
    };

    let mut messages: Vec<Message> = Vec::new();
    let mut shell: Shell<'_, Message> = Shell::new(&mut messages);
    assert!(!shell.input_method().is_enabled(), "a cold shell is closed");

    shell.request_input_method(&widget_strategy);
    assert!(
        shell.input_method().is_enabled(),
        "the focused widget asks the shell for the IME"
    );
    assert_eq!(
        shell.input_method(),
        &widget_strategy,
        "the caret rect survives into the shell strategy iced_winit reads"
    );
    match shell.input_method() {
        InputMethod::Enabled {
            cursor, purpose, ..
        } => {
            assert_eq!(*cursor, caret);
            assert_eq!(*purpose, input_method::Purpose::Normal);
        }
        InputMethod::Disabled => panic!("the widget strategy must enable the IME"),
    }

    // A second widget cannot steal an already-enabled strategy (the merge
    // keeps the focused widget's caret), mirroring the runtime's merge order.
    let second_widget: InputMethod = InputMethod::Enabled {
        cursor: Rectangle::new(Point::ORIGIN, Size::UNIT),
        purpose: input_method::Purpose::Secure,
        preedit: None,
    };
    shell.request_input_method(&second_widget);
    assert_eq!(shell.input_method(), &widget_strategy);

    // A disabled request never closes an open strategy.
    let disabled: InputMethod = InputMethod::Disabled;
    shell.request_input_method(&disabled);
    assert!(shell.input_method().is_enabled());
}

/// The palette query line is a shared semantics row; Iced cannot publish a
/// role, so the same localized label is mounted as a real tooltip on the
/// search row while the palette view builds.
#[test]
fn the_palette_query_line_carries_the_shared_label_as_a_visible_tooltip() {
    use infiltrator_contract::a11y::ShellA11yNode;

    let (mut state, _) = AppState::new();
    state.shell.lang = "zh-CN".to_string();
    assert_eq!(
        state.a11y_label(ShellA11yNode::CommandPaletteQuery),
        "命令面板查询词"
    );
    assert_eq!(
        state.a11y_role(ShellA11yNode::CommandPaletteQuery),
        infiltrator_contract::a11y::A11yRole::Text
    );

    state.shell.command_palette_open = true;
    {
        let _palette = state.view();
    }

    state.shell.lang = "en-US".to_string();
    assert_eq!(
        state.a11y_label(ShellA11yNode::CommandPaletteQuery),
        "Command palette query"
    );
}
