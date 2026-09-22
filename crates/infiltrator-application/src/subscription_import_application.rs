//! DUAL-07-01: shared multi-channel subscription import orchestration.
//!
//! The three import channels (remote URL, local file, clipboard) used to live
//! in each surface: Iced read files/clipboard directly, Bevy had only a static
//! card. This application owns the channel dispatch behind the host
//! [`SubscriptionImportPort`] so both surfaces produce the same
//! [`SubscriptionImportReport`] and a host without clipboard integration
//! surfaces a typed unsupported error instead of a fake success.

use std::sync::Arc;

use infiltrator_contract::error::Failure;
use infiltrator_contract::subscription_import::{
    SubscriptionImportChannel, SubscriptionImportReport,
};
use infiltrator_domain::subscription::extract_subscription_url;
use infiltrator_ports::subscription_import::SubscriptionImportPort;
use infiltrator_ports::subscription_source::SubscriptionSource;

use crate::profile_application::ProfileApplication;

/// Shared channel dispatch for subscription imports.
#[derive(Clone)]
pub struct SubscriptionImportApplication {
    profile: ProfileApplication,
    import_port: Arc<dyn SubscriptionImportPort>,
}

impl SubscriptionImportApplication {
    pub fn new(profile: ProfileApplication, import_port: Arc<dyn SubscriptionImportPort>) -> Self {
        Self {
            profile,
            import_port,
        }
    }

    pub fn profile(&self) -> &ProfileApplication {
        &self.profile
    }

    /// Import `name` from the given channel.
    ///
    /// * [`SubscriptionImportChannel::Url`] fetches `source_text` through the
    ///   shared subscription source;
    /// * [`SubscriptionImportChannel::LocalFile`] reads `source_text` as a
    ///   filesystem path through the host port;
    /// * [`SubscriptionImportChannel::Clipboard`] reads the clipboard and, if
    ///   it contains a subscription URL, fetches it, otherwise imports the raw
    ///   text as a local document.
    pub async fn import<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
        channel: SubscriptionImportChannel,
        source_text: &str,
    ) -> Result<SubscriptionImportReport, Failure> {
        match channel {
            SubscriptionImportChannel::Url => {
                self.profile
                    .import_subscription_report(source, name, source_text)
                    .await
            }
            SubscriptionImportChannel::LocalFile => {
                let content = self
                    .import_port
                    .read_local_file(source_text)
                    .await
                    .map_err(Failure::from)?;
                self.profile.import_document(name, &content, channel).await
            }
            SubscriptionImportChannel::Clipboard => {
                let text = self
                    .import_port
                    .read_clipboard()
                    .await
                    .map_err(Failure::from)?;
                if let Some(url) = extract_subscription_url(&text) {
                    self.profile
                        .import_subscription_report(source, name, &url)
                        .await
                } else {
                    self.profile.import_document(name, &text, channel).await
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "subscription_import_application_test.rs"]
mod tests;
