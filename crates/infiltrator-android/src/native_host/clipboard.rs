//! BANDROID-008: the Android `ClipboardManager` behind the typed clipboard
//! port.
//!
//! The Kotlin host owns the real `ClipboardManager`; this module defines the
//! toolkit-neutral channel and the [`SubscriptionImportPort`] adapter the
//! shared application already consumes for clipboard imports. A host without
//! a registered channel answers a typed unsupported result instead of a
//! fabricated empty document, and a write reports its real outcome so the UI
//! never shows "copied" before the platform accepted the text.

use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_ports::error::PortError;
use infiltrator_ports::subscription_import::SubscriptionImportPort;
use std::sync::{Arc, OnceLock, RwLock};

/// Native clipboard pull/push channel implemented by the JNI bridge. JNI and
/// `Context` types stay behind this boundary.
pub trait NativeClipboardChannel: Send + Sync {
    /// The current clipboard text, or `None` when the clipboard is empty.
    /// A host that denies access returns a typed error, never `Ok(None)`.
    fn read_clipboard(&self) -> Result<Option<String>, PortError>;
    /// Write `text` to the system clipboard.
    fn write_clipboard(&self, text: &str) -> Result<(), PortError>;
}

fn clipboard_slot() -> &'static RwLock<Option<Arc<dyn NativeClipboardChannel>>> {
    static SLOT: OnceLock<RwLock<Option<Arc<dyn NativeClipboardChannel>>>> = OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(None))
}

/// Register the process's native clipboard channel. Re-registering (Activity
/// recreation) replaces and drops the retired channel.
pub fn set_native_clipboard_channel(channel: Arc<dyn NativeClipboardChannel>) {
    let mut guard = clipboard_slot()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = Some(channel);
}

/// Release the registered channel (Activity destroyed).
pub fn clear_native_clipboard_channel() {
    let mut guard = clipboard_slot()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = None;
}

/// The registered channel, if the native host installed one.
pub fn native_clipboard_channel() -> Option<Arc<dyn NativeClipboardChannel>> {
    clipboard_slot()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// The typed result of one clipboard write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardWriteOutcome {
    /// The platform accepted the text.
    Written,
    /// There was nothing to copy.
    Empty,
    /// The platform denied clipboard access.
    Restricted,
    /// The host has no clipboard integration.
    Unavailable,
    /// The write failed for another reason.
    Failed(String),
}

impl ClipboardWriteOutcome {
    /// Whether the text was actually written.
    pub const fn is_written(&self) -> bool {
        matches!(self, Self::Written)
    }
}

/// The Android clipboard adapter implementing the shared import port.
///
/// With an explicit channel it serves that channel; with none it resolves the
/// process-registered channel on every call, so a composition built before the
/// Activity registers still reaches the clipboard once it appears.
#[derive(Clone, Default)]
pub struct AndroidClipboardPort {
    channel: Option<Arc<dyn NativeClipboardChannel>>,
}

impl AndroidClipboardPort {
    /// Bind to an explicit channel (tests, direct composition).
    pub fn new(channel: Arc<dyn NativeClipboardChannel>) -> Self {
        Self {
            channel: Some(channel),
        }
    }

    /// Resolve the process-registered native channel on each call. Safe to
    /// build before the Activity has registered one; calls answer a typed
    /// unsupported result until it does.
    pub fn from_registry() -> Self {
        Self { channel: None }
    }

    fn channel(&self) -> Option<Arc<dyn NativeClipboardChannel>> {
        self.channel.clone().or_else(native_clipboard_channel)
    }

    /// Write text to the system clipboard with a typed outcome. Empty input
    /// is rejected before touching the platform so the UI cannot claim a
    /// successful copy of nothing.
    pub fn write(&self, text: &str) -> ClipboardWriteOutcome {
        let normalized = normalize_clipboard_text(text);
        if normalized.is_empty() {
            return ClipboardWriteOutcome::Empty;
        }
        let Some(channel) = self.channel() else {
            return ClipboardWriteOutcome::Unavailable;
        };
        match channel.write_clipboard(&normalized) {
            Ok(()) => ClipboardWriteOutcome::Written,
            Err(PortError::PermissionDenied(_)) => ClipboardWriteOutcome::Restricted,
            Err(PortError::Unsupported { .. }) => ClipboardWriteOutcome::Unavailable,
            Err(other) => ClipboardWriteOutcome::Failed(other.to_string()),
        }
    }
}

#[async_trait]
impl SubscriptionImportPort for AndroidClipboardPort {
    async fn read_local_file(&self, _path: &str) -> Result<String, PortError> {
        // Android imports arrive through the system clipboard or a content
        // URI; a raw filesystem path is not a supported channel.
        Err(PortError::unsupported(
            Capability::Profiles,
            "Android imports use the system clipboard or a content URI, not a raw path",
        ))
    }

    async fn read_clipboard(&self) -> Result<String, PortError> {
        let Some(channel) = self.channel() else {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "no Android clipboard host is registered",
            ));
        };
        let text = channel.read_clipboard()?.unwrap_or_default();
        let normalized = normalize_clipboard_text(&text);
        if normalized.is_empty() {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "clipboard is empty",
            ));
        }
        Ok(normalized)
    }
}

/// Strip invisible zero-width characters, normalize line endings and trim the
/// outer whitespace. Format classification (subscription URL / YAML / base64)
/// stays downstream in the shared application; this is only the host-boundary
/// sanitation the desktop host performs too.
pub fn normalize_clipboard_text(raw: &str) -> String {
    raw.chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
            )
        })
        .filter(|&c| c != '\r')
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::pin;
    use std::sync::Mutex;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;
    use std::thread::yield_now;

    #[derive(Default)]
    struct MockClipboard {
        text: Mutex<Option<String>>,
        fail: bool,
    }

    impl MockClipboard {
        fn with_text(text: &str) -> Self {
            Self {
                text: Mutex::new(Some(text.to_string())),
                fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                text: Mutex::new(None),
                fail: true,
            }
        }
    }

    impl NativeClipboardChannel for MockClipboard {
        fn read_clipboard(&self) -> Result<Option<String>, PortError> {
            if self.fail {
                return Err(PortError::PermissionDenied(
                    "clipboard read denied".to_string(),
                ));
            }
            Ok(self.text.lock().expect("clipboard").clone())
        }

        fn write_clipboard(&self, text: &str) -> Result<(), PortError> {
            if self.fail {
                return Err(PortError::PermissionDenied(
                    "clipboard write denied".to_string(),
                ));
            }
            *self.text.lock().expect("clipboard") = Some(text.to_string());
            Ok(())
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => yield_now(),
            }
        }
    }

    #[test]
    fn clipboard_sanitizes_invisible_characters() {
        assert_eq!(
            normalize_clipboard_text("\u{FEFF}https://example.com/sub\u{200B}\r\n"),
            "https://example.com/sub"
        );
        assert_eq!(normalize_clipboard_text("  spaced  "), "spaced");
    }

    #[test]
    fn the_registered_channel_drives_the_import_port() {
        let mock = Arc::new(MockClipboard::with_text(
            "\u{FEFF}https://example.com/sub\r\n",
        ));
        let port = AndroidClipboardPort::new(mock.clone());
        let read = block_on(port.read_clipboard()).expect("clipboard text");
        assert_eq!(read, "https://example.com/sub");

        assert!(matches!(
            block_on(port.read_local_file("a.yaml")),
            Err(PortError::Unsupported { .. })
        ));
    }

    #[test]
    fn an_empty_clipboard_is_a_typed_unsupported_not_an_empty_document() {
        let port = AndroidClipboardPort::new(Arc::new(MockClipboard::default()));
        match block_on(port.read_clipboard()).expect_err("empty clipboard") {
            PortError::Unsupported { capability, .. } => {
                assert_eq!(capability, Capability::Profiles)
            }
            other => panic!("expected typed unsupported, got {other:?}"),
        }
    }

    #[test]
    fn a_denied_clipboard_read_keeps_its_permission_error() {
        let port = AndroidClipboardPort::new(Arc::new(MockClipboard::failing()));
        assert!(matches!(
            block_on(port.read_clipboard()),
            Err(PortError::PermissionDenied(_))
        ));
    }

    #[test]
    fn a_write_reports_the_real_platform_outcome() {
        let mock = Arc::new(MockClipboard::default());
        let port = AndroidClipboardPort::new(mock.clone());
        assert_eq!(port.write("hello"), ClipboardWriteOutcome::Written);
        assert_eq!(
            mock.text.lock().expect("clipboard").as_deref(),
            Some("hello")
        );
        assert!(port.write("hello").is_written());

        assert_eq!(port.write("   "), ClipboardWriteOutcome::Empty);
        assert_eq!(
            port.write("\u{200B}\u{FEFF}"),
            ClipboardWriteOutcome::Empty,
            "invisible-only text is not a successful copy"
        );

        let denied = AndroidClipboardPort::new(Arc::new(MockClipboard::failing()));
        assert_eq!(denied.write("x"), ClipboardWriteOutcome::Restricted);
    }

    #[test]
    fn the_registry_backed_port_resolves_the_channel_lazily() {
        clear_native_clipboard_channel();
        assert!(native_clipboard_channel().is_none());

        // Built before the host registers: the port answers a typed
        // unsupported result instead of an empty document.
        let port = AndroidClipboardPort::from_registry();
        match block_on(port.read_clipboard()).expect_err("no host") {
            PortError::Unsupported { capability, .. } => {
                assert_eq!(capability, Capability::Profiles)
            }
            other => panic!("expected typed unsupported, got {other:?}"),
        }
        assert_eq!(port.write("x"), ClipboardWriteOutcome::Unavailable);

        // Once the Activity registers, the same port reaches the clipboard.
        set_native_clipboard_channel(Arc::new(MockClipboard::with_text("registered")));
        assert_eq!(block_on(port.read_clipboard()).expect("read"), "registered");
        assert_eq!(port.write("out"), ClipboardWriteOutcome::Written);

        clear_native_clipboard_channel();
        assert!(native_clipboard_channel().is_none());
        assert_eq!(port.write("x"), ClipboardWriteOutcome::Unavailable);
    }
}
