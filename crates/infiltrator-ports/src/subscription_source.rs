//! Outbound port for fetching a profile's prepared subscription document.

use async_trait::async_trait;
use infiltrator_domain::subscription::{CheckedSubscriptionUrl, SubscriptionUserInfo};

use crate::error::PortError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubscriptionDocument {
    pub content: String,
    pub userinfo: Option<SubscriptionUserInfo>,
}

/// Fetches and prepares subscription content for one profile.
///
/// The adapter may apply profile-specific options before returning the
/// document. Transport clients, headers, retries, and filesystem sidecars do
/// not cross this port.
#[async_trait]
pub trait SubscriptionSource: Send + Sync {
    async fn fetch(
        &self,
        profile: &str,
        url: &CheckedSubscriptionUrl,
    ) -> Result<SubscriptionDocument, PortError>;
}
