//! Outbound port for fetching a profile's prepared subscription document.

use async_trait::async_trait;
use infiltrator_domain::subscription::{CheckedSubscriptionUrl, SubscriptionUserInfo};

use crate::error::PortError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubscriptionDocument {
    pub content: String,
    pub userinfo: Option<SubscriptionUserInfo>,
}

/// Request headers for conditional subscription fetch (ETag / If-Modified-Since).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConditionalFetchHeaders {
    pub etag: Option<String>,
    pub if_modified_since: Option<String>,
    pub custom_user_agent: Option<String>,
    pub insecure_skip_verify: bool,
}

/// Result of a conditional fetch attempt: either modified new content or 304 Not Modified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionalDocumentResult {
    /// 200 OK: New content received.
    Modified {
        document: SubscriptionDocument,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    /// 304 Not Modified: Server confirmed configuration is unchanged.
    NotModified {
        userinfo: Option<SubscriptionUserInfo>,
        etag: Option<String>,
        last_modified: Option<String>,
    },
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

    /// Conditional fetch with support for ETag, If-Modified-Since, custom User-Agent, and 304 Not Modified.
    async fn fetch_conditional(
        &self,
        profile: &str,
        url: &CheckedSubscriptionUrl,
        headers: &ConditionalFetchHeaders,
    ) -> Result<ConditionalDocumentResult, PortError> {
        let _ = headers;
        let document = self.fetch(profile, url).await?;
        Ok(ConditionalDocumentResult::Modified {
            document,
            etag: None,
            last_modified: None,
        })
    }
}
