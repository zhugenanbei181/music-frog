//! Engine-neutral profile write-protection read model.
//!
//! A profile downloaded from a subscription URL is owned by the provider: a
//! direct content edit would be silently overwritten by the next refresh and
//! is almost never what the user meant. The classification lives in a neutral
//! vocabulary so both surfaces render the same state and the application can
//! enforce the same rule, while Mixin overrides stay allowed by design.

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

/// Explicit authority carried by a profile document write request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileWriteAuthorization {
    /// Direct writes are accepted only for profiles already classified editable.
    #[default]
    RequireUnlock,
    /// The user explicitly unlocked writing to a protected subscription profile.
    ExplicitUnlock,
}

impl ProfileWriteAuthorization {
    pub const fn allows_protected_write(self) -> bool {
        matches!(self, Self::ExplicitUnlock)
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::RequireUnlock => Self::ExplicitUnlock,
            Self::ExplicitUnlock => Self::RequireUnlock,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{ProfileWriteAuthorization, ProfileWriteProtection};

    #[test]
    fn protection_classifies_by_subscription_url() {
        assert_eq!(
            ProfileWriteProtection::from_subscription_url("  "),
            ProfileWriteProtection::Editable
        );
        assert_eq!(
            ProfileWriteProtection::from_subscription_url("https://example.test/sub"),
            ProfileWriteProtection::RemoteSubscription
        );
        assert!(!ProfileWriteProtection::default().is_protected());
    }

    #[test]
    fn authorization_toggles_and_reports_protected_write() {
        assert!(!ProfileWriteAuthorization::default().allows_protected_write());
        assert!(
            ProfileWriteAuthorization::RequireUnlock
                .toggled()
                .allows_protected_write()
        );
        assert_eq!(
            ProfileWriteAuthorization::ExplicitUnlock.toggled(),
            ProfileWriteAuthorization::RequireUnlock
        );
    }

    #[test]
    fn protection_serde_names_are_snake_case() {
        assert_eq!(
            serde_json::to_string(&ProfileWriteProtection::RemoteSubscription).unwrap(),
            "\"remote_subscription\""
        );
        assert_eq!(
            serde_json::to_string(&ProfileWriteAuthorization::RequireUnlock).unwrap(),
            "\"require_unlock\""
        );
    }
}
