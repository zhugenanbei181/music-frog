//! test-intent: behavior
use super::*;
use bevy::ecs::message::Messages;
use bevy::input::{ButtonInput, keyboard::KeyCode};
use infiltrator_bevy_ui::pages::connections_clipboard::{
    ClipboardHost, ClipboardPasteIntent, ClipboardPasteOutcome, ClipboardPasteReport,
    ClipboardPort, ClipboardRead, ClipboardWrite, CopyConnectionHostButton,
};
use infiltrator_bevy_ui::toast::ShellToast;
use infiltrator_bevy_widgets::drawer::DrawerCloseButton;
use infiltrator_bevy_widgets::text_input::TextFieldFocused;
use infiltrator_bevy_widgets::toast::ToastKind;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::connection::timing_availability;

fn open(app: &mut App) {
    let entity = app
        .world_mut()
        .query::<(Entity, &ConnInspectButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == 0)
        .unwrap()
        .0;
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
    app.update();
}

#[test]
fn connection_drawer_close_escape_navigation_and_stale_disconnect_are_safe() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Connections);
    let original = app
        .world()
        .resource::<LastConnectionsProjection>()
        .0
        .clone();
    open(&mut app);
    assert_eq!(
        app.world().resource::<ConnectionsDrawerState>().selected,
        Some(0)
    );
    let layer = app
        .world_mut()
        .query_filtered::<Entity, With<ConnectionDrawerLayer>>()
        .single(app.world())
        .unwrap();
    assert_eq!(
        app.world().get::<Node>(layer).unwrap().display,
        Display::Flex
    );
    assert!(
        matches!(timing_availability(), Availability::Unsupported { reason } if reason.contains("DNS/TCP/TLS/TTFB"))
    );
    let close = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: close });
    app.update();
    app.update();
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    assert_eq!(
        app.world().get::<Node>(layer).unwrap().display,
        Display::None
    );
    let disconnect = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseConnectionButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert!(sink.submitted().is_empty());
    open(&mut app);
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.world_mut().insert_resource(keys);
    app.update();
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    open(&mut app);
    navigate_to(&mut app, Route::Logs);
    app.update();
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    assert_eq!(
        app.world().resource::<LastConnectionsProjection>().0,
        original
    );
    assert!(sink.submitted().is_empty());
}

#[test]
fn connection_drawer_unsupported_clipboard_does_not_report_success() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Connections);
    open(&mut app);
    let copy = app
        .world_mut()
        .query_filtered::<Entity, With<CopyConnectionHostButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: copy });
    app.world_mut().flush();
    let messages: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<ShellToast>>()
        .drain()
        .collect();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].kind, ToastKind::Warning);
    assert!(sink.submitted().is_empty());
    let disconnect = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseConnectionButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert_eq!(
        sink.submitted()
            .iter()
            .filter_map(UiCommand::to_intent)
            .collect::<Vec<_>>(),
        vec![CommandIntent::CloseConnection { id: "c-1".into() }]
    );
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert_eq!(sink.submitted().len(), 1);
}

#[test]
fn connection_drawer_refresh_follows_identity_through_reorder() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Connections);
    open(&mut app);
    let mut projection = app
        .world()
        .resource::<LastConnectionsProjection>()
        .0
        .clone()
        .unwrap();
    projection.connections.swap(0, 1);
    projection.connections[1].host = "updated.example.test:443".into();
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(projection));
    app.update();
    app.update();
    let state = app.world().resource::<ConnectionsDrawerState>();
    assert_eq!(state.selected_id.as_deref(), Some("c-1"));
    assert_eq!(state.selected, Some(1));
    let host = app
        .world_mut()
        .query::<(&Text, &ConnDrawerField)>()
        .iter(app.world())
        .find(|(_, field)| field.0 == ConnDrawerFieldKind::Host)
        .unwrap()
        .0;
    assert_eq!(host.0, "updated.example.test:443");
    let disconnect = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseConnectionButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert_eq!(
        sink.submitted()
            .iter()
            .filter_map(UiCommand::to_intent)
            .collect::<Vec<_>>(),
        vec![CommandIntent::CloseConnection { id: "c-1".into() }]
    );
}

#[test]
fn connection_drawer_refresh_dismisses_removed_identity_and_rejects_replay() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Connections);
    open(&mut app);
    let mut projection = app
        .world()
        .resource::<LastConnectionsProjection>()
        .0
        .clone()
        .unwrap();
    projection.connections.retain(|item| item.id != "c-1");
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(projection));
    app.update();
    app.update();
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    let disconnect = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseConnectionButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert!(sink.submitted().is_empty());
}

// ---- BANDROID-008: typed system-clipboard host seam ------------------------

/// A scripted host clipboard adapter: reads return `read`, writes record the
/// text and return `write`, so a false success cannot pass unnoticed.
struct FakeClipboard {
    read: ClipboardRead,
    write: ClipboardWrite,
    writes: Mutex<Vec<String>>,
}

impl FakeClipboard {
    fn written() -> Self {
        Self {
            read: ClipboardRead::Empty,
            write: ClipboardWrite::Written,
            writes: Mutex::new(Vec::new()),
        }
    }

    fn reading(read: ClipboardRead) -> Self {
        Self {
            read,
            write: ClipboardWrite::Written,
            writes: Mutex::new(Vec::new()),
        }
    }

    fn failing(write: ClipboardWrite) -> Self {
        Self {
            read: ClipboardRead::Empty,
            write,
            writes: Mutex::new(Vec::new()),
        }
    }

    fn writes(&self) -> Vec<String> {
        self.writes.lock().expect("clipboard writes").clone()
    }
}

impl ClipboardPort for FakeClipboard {
    fn read_text(&self) -> ClipboardRead {
        self.read.clone()
    }

    fn write_text(&self, text: &str) -> ClipboardWrite {
        self.writes
            .lock()
            .expect("clipboard writes")
            .push(text.to_owned());
        self.write.clone()
    }
}

fn install_clipboard(app: &mut App, fake: &Arc<FakeClipboard>) {
    let port: Arc<dyn ClipboardPort> = fake.clone();
    app.insert_resource(ClipboardHost::with_port(port));
}

fn copy_toasts(app: &mut App) -> Vec<ShellToast> {
    app.world_mut()
        .resource_mut::<Messages<ShellToast>>()
        .drain()
        .collect()
}

fn trigger_copy(app: &mut App) {
    let copy = app
        .world_mut()
        .query_filtered::<Entity, With<CopyConnectionHostButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: copy });
    app.world_mut().flush();
}

#[test]
fn connection_copy_reports_success_only_after_a_real_host_write() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let fake = Arc::new(FakeClipboard::written());
    install_clipboard(&mut app, &fake);
    navigate_to(&mut app, Route::Connections);
    open(&mut app);

    trigger_copy(&mut app);
    let toasts = copy_toasts(&mut app);
    assert_eq!(toasts.len(), 1);
    assert_eq!(
        toasts[0].kind,
        ToastKind::Info,
        "a confirmed write succeeds"
    );
    assert_eq!(
        fake.writes(),
        vec!["api.github.com".to_owned()],
        "the real host adapter performed the write"
    );
    assert!(sink.submitted().is_empty());
}

#[test]
fn connection_copy_surfaces_denied_write_without_success() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let fake = Arc::new(FakeClipboard::failing(ClipboardWrite::Denied));
    install_clipboard(&mut app, &fake);
    navigate_to(&mut app, Route::Connections);
    open(&mut app);

    trigger_copy(&mut app);
    let toasts = copy_toasts(&mut app);
    assert_eq!(toasts.len(), 1);
    assert_eq!(
        toasts[0].kind,
        ToastKind::Danger,
        "a denied write is surfaced, never a success"
    );
    assert!(sink.submitted().is_empty());
}

#[test]
fn connection_copy_surfaces_write_failure_without_success() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let fake = Arc::new(FakeClipboard::failing(ClipboardWrite::Failed(
        "clipboard busy".to_owned(),
    )));
    install_clipboard(&mut app, &fake);
    navigate_to(&mut app, Route::Connections);
    open(&mut app);

    trigger_copy(&mut app);
    let toasts = copy_toasts(&mut app);
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0].kind, ToastKind::Danger);
    assert!(
        toasts[0].text.contains("clipboard busy"),
        "the typed failure reason is surfaced: {}",
        toasts[0].text
    );
    assert!(sink.submitted().is_empty());
}

#[test]
fn clipboard_read_results_are_typed_with_zero_false_success() {
    let empty = ClipboardHost::with_port(Arc::new(FakeClipboard::reading(ClipboardRead::Empty)));
    assert_eq!(empty.read_text(), ClipboardRead::Empty);
    let denied = ClipboardHost::with_port(Arc::new(FakeClipboard::reading(ClipboardRead::Denied)));
    assert_eq!(denied.read_text(), ClipboardRead::Denied);
    let invalid = ClipboardHost::with_port(Arc::new(FakeClipboard::reading(
        ClipboardRead::Invalid("not a subscription"),
    )));
    assert_eq!(
        invalid.read_text(),
        ClipboardRead::Invalid("not a subscription")
    );
    // No installed port is a typed unsupported, never an empty success.
    assert_eq!(
        ClipboardHost::default().read_text(),
        ClipboardRead::Unsupported("no native clipboard host")
    );
}

#[test]
fn clipboard_paste_inserts_only_real_host_text() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let fake = Arc::new(FakeClipboard::reading(ClipboardRead::Value(
        "https://example.com/sub\u{200B}".to_owned(),
    )));
    install_clipboard(&mut app, &fake);
    let field = app
        .world_mut()
        .spawn((TextField(TextFieldState::new("")), TextFieldFocused(true)))
        .id();

    app.world_mut().trigger(ClipboardPasteIntent);
    app.update();

    assert_eq!(
        app.world().resource::<ClipboardPasteReport>().outcome,
        ClipboardPasteOutcome::Pasted("https://example.com/sub".chars().count()),
        "the sanitized host text was inserted"
    );
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "https://example.com/sub",
        "the zero-width character was stripped before insertion"
    );

    // An empty clipboard is a typed empty outcome, never a fake paste.
    let empty = Arc::new(FakeClipboard::reading(ClipboardRead::Empty));
    install_clipboard(&mut app, &empty);
    app.world_mut().trigger(ClipboardPasteIntent);
    app.update();
    assert_eq!(
        app.world().resource::<ClipboardPasteReport>().outcome,
        ClipboardPasteOutcome::Empty
    );
}

#[test]
fn clipboard_paste_denied_and_invalid_are_surfaced() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());

    let denied = Arc::new(FakeClipboard::reading(ClipboardRead::Denied));
    install_clipboard(&mut app, &denied);
    app.world_mut().trigger(ClipboardPasteIntent);
    app.update();
    assert_eq!(
        app.world().resource::<ClipboardPasteReport>().outcome,
        ClipboardPasteOutcome::Denied,
        "a denied read is surfaced, never a silent paste"
    );

    let invalid = Arc::new(FakeClipboard::reading(ClipboardRead::Invalid(
        "not a subscription",
    )));
    install_clipboard(&mut app, &invalid);
    app.world_mut().trigger(ClipboardPasteIntent);
    app.update();
    assert_eq!(
        app.world().resource::<ClipboardPasteReport>().outcome,
        ClipboardPasteOutcome::Invalid("not a subscription"),
        "invalid host content is surfaced with its reason"
    );
}
