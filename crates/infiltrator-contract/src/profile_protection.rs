//! DUAL-09-12: remote-subscription write protection read model.
//!
//! A profile downloaded from a subscription URL is owned by the provider: a
//! direct content edit would be silently overwritten by the next refresh and
//! is almost never what the user meant. The shared classification lives here
//! so both surfaces render the same state and the application can enforce the
//! same rule, while Mixin overrides stay allowed by design.

use serde::{Deserialize, Serialize};

/// Whether a profile's content may be edited directly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileWriteProtection {
    /// Local/manual profile without a subscription source: direct editing is
    /// the normal path.
    #[default]
    Editable,
    /// Remote subscription: direct content edits must be explicitly unlocked;
    /// Mixin overrides remain the recommended route.
    RemoteSubscription,
}

impl ProfileWriteProtection {
    /// Classify from the profile's subscription URL. An empty/blank URL means
    /// the profile is local (imported or hand-written) and editable.
    pub fn from_subscription_url(url: &str) -> Self {
        if url.trim().is_empty() {
            Self::Editable
        } else {
            Self::RemoteSubscription
        }
    }

    pub const fn is_protected(self) -> bool {
        matches!(self, Self::RemoteSubscription)
    }
}
