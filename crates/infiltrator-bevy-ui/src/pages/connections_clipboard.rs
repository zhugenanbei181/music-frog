//! Native system-clipboard host seam (BANDROID-008).
//!
//! Connection copy, subscription import and SDK text paste all reach the same
//! system clipboard. On Android/iOS `bevy::clipboard::Clipboard` is an
//! in-process buffer that reports `Ok(())` without touching the OS, so a
//! success toast driven by it would be a false success. This module owns the
//! typed [`ClipboardHost`] port a native `ClipboardManager` adapter installs;
//! the desktop `Clipboard` resource stays the fallback only where it is a real
//! OS clipboard. Subscription import still routes through the shared
//! application's `SubscriptionImportPort`, never a second read here.

use crate::pages::connections::LastConnectionsProjection;
use crate::pages::connections_drawer::ConnectionsDrawerState;
use crate::toast::ShellToast;
use bevy::app::{App, Plugin};
use bevy::clipboard::Clipboard;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input_focus::InputFocus;
use bevy::text::{EditableText, TextEdit};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::clipboard_sanitizer::sanitize_pasted_text;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_shared::locales::{Lang, Localizer};
use std::fmt;
use std::sync::{Arc, OnceLock};

/// The typed result of reading the system clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardRead {
    /// The clipboard held this text.
    Value(String),
    /// The clipboard held no usable text.
    Empty,
    /// The host denied clipboard access (locked, restricted, no permission).
    Denied,
    /// The host returned content this surface rejects before use.
    Invalid(&'static str),
    /// No clipboard backend exists (unsupported platform or absent host).
    Unsupported(&'static str),
}

/// The typed result of writing the system clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardWrite {
    /// The backend confirmed the write actually reached the OS clipboard.
    Written,
    /// The host denied clipboard access.
    Denied,
    /// The backend accepted the request but the write failed.
    Failed(String),
    /// No clipboard backend exists (unsupported platform or absent host).
    Unsupported(&'static str),
}

/// A platform system-clipboard adapter (Android `ClipboardManager`, iOS
/// `UIPasteboard`, desktop `arboard`). The UI never talks to the OS directly.
pub trait ClipboardPort: Send + Sync {
    /// Read the current clipboard text as a typed result.
    fn read_text(&self) -> ClipboardRead;

    /// Write `text`; success only when the platform confirms the write.
    fn write_text(&self, text: &str) -> ClipboardWrite;
}

/// The shell's typed clipboard host (BANDROID-008).
///
/// Presence of a port is the capability; without one the shell reports
/// [`ClipboardWrite::Unsupported`] instead of a fabricated success. A native
/// host installs its port with [`ClipboardHost::with_port`] (or
/// [`attach_clipboard`] before launch).
#[derive(Resource, Clone, Default)]
pub struct ClipboardHost {
    port: Option<Arc<dyn ClipboardPort>>,
}

/// `lib.rs` installs the shell default with `insert_resource(ClipboardHost)`;
/// a braced struct has no value-namespace constructor, so this const shares the
/// type's name and supplies that default without a unit-struct marker.
#[allow(non_upper_case_globals)]
pub const ClipboardHost: ClipboardHost = ClipboardHost { port: None };

impl fmt::Debug for ClipboardHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClipboardHost")
            .field("has_port", &self.port.is_some())
            .finish()
    }
}

impl ClipboardHost {
    /// Install a platform clipboard adapter.
    pub fn with_port(port: Arc<dyn ClipboardPort>) -> Self {
        Self { port: Some(port) }
    }

    /// Whether a real platform adapter is installed.
    pub const fn has_port(&self) -> bool {
        self.port.is_some()
    }

    /// Read through the installed adapter, or a typed unsupported result.
    pub fn read_text(&self) -> ClipboardRead {
        match &self.port {
            Some(port) => port.read_text(),
            None => ClipboardRead::Unsupported("no native clipboard host"),
        }
    }

    /// Write through the installed adapter, or a typed unsupported result.
    pub fn write_text(&self, text: &str) -> ClipboardWrite {
        match &self.port {
            Some(port) => port.write_text(text),
            None => ClipboardWrite::Unsupported("no native clipboard host"),
        }
    }
}

static ATTACHED_CLIPBOARD: OnceLock<ClipboardHost> = OnceLock::new();

/// Attach the host's clipboard adapter before launch (mirrors
/// `attach_application` / `attach_haptics`). The default
/// [`ClipboardHostPlugin`] picks it up.
pub fn attach_clipboard(host: ClipboardHost) {
    let _ = ATTACHED_CLIPBOARD.set(host);
}

/// Retrieve the attached clipboard host, if any.
pub fn attached_clipboard() -> Option<ClipboardHost> {
    ATTACHED_CLIPBOARD.get().cloned()
}

/// Read the system clipboard through the host port, falling back to the real
/// desktop `Clipboard` resource only when the shell advertises the
/// [`ClipboardHost`] seam. Mobile never falls back: the Bevy resource is an
/// in-process buffer there and would fake a cross-app paste.
pub fn read_clipboard(
    host: Option<&ClipboardHost>,
    clipboard: Option<&mut Clipboard>,
) -> ClipboardRead {
    let Some(host) = host else {
        return ClipboardRead::Unsupported("shell has no clipboard seam");
    };
    if host.has_port() {
        return host.read_text();
    }
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return ClipboardRead::Unsupported("mobile host has no clipboard port");
    }
    let Some(clipboard) = clipboard else {
        return ClipboardRead::Unsupported("no clipboard backend");
    };
    let mut read = clipboard.fetch_text();
    match read.poll_result() {
        Some(Ok(text)) if text.is_empty() => ClipboardRead::Empty,
        Some(Ok(text)) => ClipboardRead::Value(text),
        Some(Err(_)) => ClipboardRead::Denied,
        None => ClipboardRead::Unsupported("clipboard read is still pending"),
    }
}

/// Write the system clipboard through the host port, falling back to the real
/// desktop `Clipboard` resource only when the shell advertises the
/// [`ClipboardHost`] seam. Success is reported only after the chosen backend
/// confirms the write reached the OS.
pub fn write_clipboard(
    host: Option<&ClipboardHost>,
    clipboard: Option<&mut Clipboard>,
    text: &str,
) -> ClipboardWrite {
    let Some(host) = host else {
        return ClipboardWrite::Unsupported("shell has no clipboard seam");
    };
    if host.has_port() {
        return host.write_text(text);
    }
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return ClipboardWrite::Unsupported("mobile host has no clipboard port");
    }
    let Some(clipboard) = clipboard else {
        return ClipboardWrite::Unsupported("no clipboard backend");
    };
    match clipboard.set_text(text) {
        Ok(()) => ClipboardWrite::Written,
        Err(error) => ClipboardWrite::Failed(error.to_string()),
    }
}

/// Explicit host composition, separate from the SDK editor's clipboard resource.
/// Presence permits an OS operation; only its actual result acknowledges success.
#[derive(Component, Default, Clone, Copy)]
pub struct CopyConnectionHostButton;

pub(crate) fn copy_connection_host(
    activation: On<Activate>,
    buttons: Query<(), With<CopyConnectionHostButton>>,
    state: Res<ConnectionsDrawerState>,
    projection: Res<LastConnectionsProjection>,
    host: Option<Res<ClipboardHost>>,
    mut clipboard: Option<ResMut<Clipboard>>,
    mut toasts: MessageWriter<ShellToast>,
) {
    if !buttons.contains(activation.entity) || !state.open {
        return;
    }
    let Some(item) = state.selected_id.as_ref().and_then(|id| {
        projection
            .0
            .as_ref()
            .and_then(|projection| projection.connections.iter().find(|item| &item.id == id))
    }) else {
        return;
    };
    let language = UiLocale::default().code().to_string();
    let lang = Lang(&language);
    if item.destination_host.is_empty() {
        toasts.write(ShellToast::warning(
            lang.tr("clipboard_unsupported").into_owned(),
        ));
        return;
    }
    match write_clipboard(
        host.as_deref(),
        clipboard.as_deref_mut(),
        &item.destination_host,
    ) {
        ClipboardWrite::Written => {
            toasts.write(ShellToast::info(lang.tr("clipboard_copied").into_owned()));
        }
        ClipboardWrite::Denied => {
            toasts.write(ShellToast::danger(format!(
                "{}: denied",
                lang.tr("clipboard_failed")
            )));
        }
        ClipboardWrite::Failed(error) => {
            toasts.write(ShellToast::danger(format!(
                "{}: {error}",
                lang.tr("clipboard_failed")
            )));
        }
        ClipboardWrite::Unsupported(reason) => {
            toasts.write(ShellToast::warning(format!(
                "{}: {reason}",
                lang.tr("clipboard_unsupported")
            )));
        }
    }
}

/// Host intent to paste the system clipboard into the focused text owner
/// (SDK multiline editor or widget text field). The host triggers this instead
/// of letting the SDK read its in-process clipboard buffer directly.
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClipboardPasteIntent;

/// What the most recent paste intent did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ClipboardPasteOutcome {
    /// No paste has been attempted yet.
    #[default]
    Idle,
    /// `n` characters were inserted into the focused owner.
    Pasted(usize),
    /// The clipboard held no usable text.
    Empty,
    /// The host denied clipboard access.
    Denied,
    /// The host returned content this surface rejects.
    Invalid(&'static str),
    /// No clipboard backend or no focused text owner.
    Unsupported(&'static str),
}

/// The typed outcome of the last [`ClipboardPasteIntent`].
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipboardPasteReport {
    pub outcome: ClipboardPasteOutcome,
}

/// Paste the host clipboard into the focused text owner through its own edit
/// path, surfacing a typed outcome instead of a silent success.
pub fn on_clipboard_paste(
    _intent: On<ClipboardPasteIntent>,
    host: Option<Res<ClipboardHost>>,
    mut clipboard: Option<ResMut<Clipboard>>,
    focus: Option<Res<InputFocus>>,
    mut fields: Query<(&TextFieldFocused, &mut TextField)>,
    mut editors: Query<&mut EditableText>,
    mut report: ResMut<ClipboardPasteReport>,
) {
    let read = read_clipboard(host.as_deref(), clipboard.as_deref_mut());
    let text = match read {
        ClipboardRead::Value(raw) => sanitize_pasted_text(&raw),
        ClipboardRead::Empty => {
            report.outcome = ClipboardPasteOutcome::Empty;
            return;
        }
        ClipboardRead::Denied => {
            report.outcome = ClipboardPasteOutcome::Denied;
            return;
        }
        ClipboardRead::Invalid(reason) => {
            report.outcome = ClipboardPasteOutcome::Invalid(reason);
            return;
        }
        ClipboardRead::Unsupported(reason) => {
            report.outcome = ClipboardPasteOutcome::Unsupported(reason);
            return;
        }
    };
    if text.is_empty() {
        report.outcome = ClipboardPasteOutcome::Empty;
        return;
    }
    let count = text.chars().count();
    if let Some(entity) = focus.as_deref().and_then(InputFocus::get)
        && let Ok(mut editor) = editors.get_mut(entity)
    {
        editor.queue_edit(TextEdit::Insert(text.into()));
        report.outcome = ClipboardPasteOutcome::Pasted(count);
        return;
    }
    for (focused, mut field) in &mut fields {
        if focused.0 {
            field.0.apply(TextFieldInput::Insert(text));
            report.outcome = ClipboardPasteOutcome::Pasted(count);
            return;
        }
    }
    report.outcome = ClipboardPasteOutcome::Unsupported("no focused text owner");
}

/// Installs the clipboard paste seam and, when attached, the native host port.
pub struct ClipboardHostPlugin {
    host: Option<ClipboardHost>,
}

impl Default for ClipboardHostPlugin {
    fn default() -> Self {
        Self {
            host: attached_clipboard(),
        }
    }
}

impl ClipboardHostPlugin {
    /// Install with an explicit host clipboard adapter.
    pub fn new(host: ClipboardHost) -> Self {
        Self { host: Some(host) }
    }
}

impl Plugin for ClipboardHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClipboardPasteReport>()
            .add_observer(on_clipboard_paste);
        if let Some(host) = &self.host {
            app.insert_resource(host.clone());
        }
    }
}
