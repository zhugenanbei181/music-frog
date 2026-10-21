//! Isolated subscription-source port fixture.
//!
//! The default fixture is a source that *fails*: the fetch really fails with a
//! typed network failure, so a capture can prove the refusal path instead of
//! rendering a fabricated document. A serving source publishes one prepared
//! document per profile and answers `NotFound` for a profile it never served.

use async_trait::async_trait;
use infiltrator_domain::subscription::CheckedSubscriptionUrl;
use infiltrator_ports::subscription_source::{SubscriptionDocument, SubscriptionSource};
use lurker_core_ports::error::PortError;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The profile the serving source answers for.
pub const CAPTURE_PROFILE: &str = "capture-subscription";
/// The document the serving source publishes.
pub const CAPTURE_DOCUMENT: &str =
    "mixed-port: 7890\nproxies: []\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n";

/// An isolated subscription source that either serves or refuses.
pub struct SubscriptionSourceCapture {
    documents: Mutex<BTreeMap<String, SubscriptionDocument>>,
    refusal: Mutex<Option<String>>,
    requested_profiles: Mutex<Vec<String>>,
}

impl Default for SubscriptionSourceCapture {
    fn default() -> Self {
        Self::refusing("capture subscription source is offline")
    }
}

impl SubscriptionSourceCapture {
    /// A source that refuses every fetch with a typed network failure.
    pub fn refusing(reason: impl Into<String>) -> Self {
        Self {
            documents: Mutex::new(BTreeMap::new()),
            refusal: Mutex::new(Some(reason.into())),
            requested_profiles: Mutex::new(Vec::new()),
        }
    }

    /// A source that serves one prepared document for `profile`.
    pub fn serving(profile: &str, content: &str) -> Self {
        let mut documents = BTreeMap::new();
        documents.insert(
            profile.to_owned(),
            SubscriptionDocument {
                content: content.to_owned(),
                userinfo: None,
            },
        );
        Self {
            documents: Mutex::new(documents),
            refusal: Mutex::new(None),
            requested_profiles: Mutex::new(Vec::new()),
        }
    }

    /// The profiles a surface asked the source for.
    pub fn requested_profiles(&self) -> Vec<String> {
        self.requested_profiles
            .lock()
            .expect("subscription capture requests")
            .clone()
    }
}

#[async_trait]
impl SubscriptionSource for SubscriptionSourceCapture {
    async fn fetch(
        &self,
        profile: &str,
        _url: &CheckedSubscriptionUrl,
    ) -> Result<SubscriptionDocument, PortError> {
        self.requested_profiles
            .lock()
            .expect("subscription capture requests")
            .push(profile.to_owned());
        if let Some(reason) = self
            .refusal
            .lock()
            .expect("subscription capture refusal")
            .clone()
        {
            return Err(PortError::Network(reason));
        }
        self.documents
            .lock()
            .expect("subscription capture documents")
            .get(profile)
            .cloned()
            .ok_or_else(|| {
                PortError::NotFound(format!(
                    "capture subscription source never served profile {profile}"
                ))
            })
    }
}
