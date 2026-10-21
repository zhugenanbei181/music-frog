//! Isolated subscription-import port fixture.
//!
//! The fixture publishes one local file and one clipboard entry. A host
//! without clipboard integration answers `read_clipboard` with a typed
//! `Unsupported` failure instead of an empty document, and an unknown path
//! answers `NotFound`.

use async_trait::async_trait;
use infiltrator_ports::subscription_import::SubscriptionImportPort;
use lurker_core_contract::capability::Capability;
use lurker_core_ports::error::PortError;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The local file the fixture can read.
pub const CAPTURE_FILE_PATH: &str = "/isolated-import/subscription.yaml";
/// The document the local file carries.
pub const CAPTURE_FILE_DOCUMENT: &str =
    "mixed-port: 7890\nproxies: []\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n";
/// The clipboard entry the fixture publishes.
pub const CAPTURE_CLIPBOARD_TEXT: &str = "https://subscription.example.com/profile.yaml";

/// An isolated subscription-import host that reads files and the clipboard.
pub struct SubscriptionImportCapture {
    files: Mutex<BTreeMap<String, String>>,
    clipboard: Mutex<Option<String>>,
}

impl Default for SubscriptionImportCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionImportCapture {
    /// A host with one local file and one clipboard entry.
    pub fn new() -> Self {
        let mut files = BTreeMap::new();
        files.insert(
            CAPTURE_FILE_PATH.to_owned(),
            CAPTURE_FILE_DOCUMENT.to_owned(),
        );
        Self {
            files: Mutex::new(files),
            clipboard: Mutex::new(Some(CAPTURE_CLIPBOARD_TEXT.to_owned())),
        }
    }

    /// A host without any clipboard integration.
    pub fn without_clipboard() -> Self {
        let host = Self::new();
        *host
            .clipboard
            .lock()
            .expect("subscription import clipboard") = None;
        host
    }

    /// The clipboard text the host publishes, when it has one.
    pub fn clipboard_text(&self) -> Option<String> {
        self.clipboard
            .lock()
            .expect("subscription import clipboard")
            .clone()
    }
}

#[async_trait]
impl SubscriptionImportPort for SubscriptionImportCapture {
    async fn read_local_file(&self, path: &str) -> Result<String, PortError> {
        self.files
            .lock()
            .expect("subscription import files")
            .get(path)
            .cloned()
            .ok_or_else(|| PortError::NotFound(format!("no import document at {path}")))
    }

    async fn read_clipboard(&self) -> Result<String, PortError> {
        self.clipboard
            .lock()
            .expect("subscription import clipboard")
            .clone()
            .ok_or_else(|| {
                PortError::unsupported(
                    Capability::Profiles,
                    "this host has no clipboard integration",
                )
            })
    }
}
