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
    ///
    /// The default implementation has no transport of its own: it performs a
    /// plain [`Self::fetch`] (always "modified", no validators). An adapter
    /// that cannot actually honor `insecure_skip_verify` must not pretend to,
    /// so the flag is rejected as a typed unsupported capability here and
    /// must be overridden by an adapter that can disable TLS verification.
    async fn fetch_conditional(
        &self,
        profile: &str,
        url: &CheckedSubscriptionUrl,
        headers: &ConditionalFetchHeaders,
    ) -> Result<ConditionalDocumentResult, PortError> {
        if headers.insecure_skip_verify {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "this subscription source cannot skip TLS certificate verification",
            ));
        }
        let document = self.fetch(profile, url).await?;
        Ok(ConditionalDocumentResult::Modified {
            document,
            etag: None,
            last_modified: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::capability::Capability;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    struct PlainSource;

    #[async_trait]
    impl SubscriptionSource for PlainSource {
        async fn fetch(
            &self,
            _profile: &str,
            _url: &CheckedSubscriptionUrl,
        ) -> Result<SubscriptionDocument, PortError> {
            Ok(SubscriptionDocument {
                content: "proxies: []".to_string(),
                userinfo: None,
            })
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn default_conditional_fetch_is_a_plain_fetch() {
        let source = PlainSource;
        let url = CheckedSubscriptionUrl::parse("https://example.com/sub.yaml").expect("url");
        let result =
            block_on(source.fetch_conditional("p", &url, &ConditionalFetchHeaders::default()))
                .expect("plain fetch");
        assert!(matches!(result, ConditionalDocumentResult::Modified { .. }));
    }

    #[test]
    fn default_conditional_fetch_rejects_unhonored_insecure_flag() {
        let source = PlainSource;
        let url = CheckedSubscriptionUrl::parse("https://example.com/sub.yaml").expect("url");
        let headers = ConditionalFetchHeaders {
            insecure_skip_verify: true,
            ..ConditionalFetchHeaders::default()
        };
        let error = block_on(source.fetch_conditional("p", &url, &headers))
            .expect_err("must be typed unsupported");
        match error {
            PortError::Unsupported { capability, .. } => {
                assert_eq!(capability, Capability::Profiles);
            }
            other => panic!("expected typed unsupported, got {other:?}"),
        }
    }
}
