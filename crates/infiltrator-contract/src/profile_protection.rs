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

    /// Surface badge label (both surfaces render the identical string).
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Editable => "本地配置 · 可直接编辑",
            Self::RemoteSubscription => "远程订阅 · 只读保护",
        }
    }

    /// Why direct edits are discouraged, and what to do instead.
    pub const fn hint_zh(self) -> &'static str {
        match self {
            Self::Editable => "本地配置由你完全掌控，可直接保存修改。",
            Self::RemoteSubscription => {
                "远程订阅内容会在下次更新时被覆盖；请优先使用 Mixin 覆写，或显式解锁后直接编辑。"
            }
        }
    }
}
