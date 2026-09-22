//! DUAL-09-14: shared per-profile option sidecar read model for the editors.
//!
//! The Mixin overlay and the subscription filter are stored in one sidecar
//! (`<config-dir>/options/<profile>.yaml`). Both surfaces edit those two
//! documents, and both must start from the *stored* fact: this module carries
//! the sidecar across the surface boundary in the same surface-editable shape
//! the Iced panes use (a Mixin YAML buffer and the shared
//! [`crate::subscription_import::SubscriptionFilterDraft`]).
//!
//! Like the profile-document read model, the snapshot is published by the
//! application use-case and read by the surface projection, so a Bevy pane
//! never reads the file itself.

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

/// The stored options of one profile, in surface-editable form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileOptionsSnapshot {
    pub profile: String,
    /// The stored Mixin overlay serialized as YAML — the document the Mixin
    /// editor pane opens (the same bytes `serde_yaml_ng` produces for the
    /// stored `MixinConfig`, byte-stable across both surfaces).
    pub mixin_yaml: String,
    /// The stored subscription filter as the shared surface draft.
    pub filter: crate::subscription_import::SubscriptionFilterDraft,
}

impl ProfileOptionsSnapshot {
    pub fn new(
        profile: impl Into<String>,
        mixin_yaml: impl Into<String>,
        filter: crate::subscription_import::SubscriptionFilterDraft,
    ) -> Self {
        Self {
            profile: profile.into(),
            mixin_yaml: mixin_yaml.into(),
            filter,
        }
    }

    /// True when neither half would change the composed profile document, so a
    /// surface can state "无覆盖" instead of rendering an empty editor.
    pub fn is_empty(&self) -> bool {
        let mixin_empty = self
            .mixin_yaml
            .lines()
            .map(str::trim)
            .all(|line| line.is_empty() || line == "{}" || line == "---");
        mixin_empty && self.filter.is_empty()
    }
}

fn options_cache() -> &'static Mutex<Option<ProfileOptionsSnapshot>> {
    static OPTIONS: OnceLock<Mutex<Option<ProfileOptionsSnapshot>>> = OnceLock::new();
    OPTIONS.get_or_init(|| Mutex::new(None))
}

/// Publish the sidecar a surface just loaded (or saved).
pub fn publish_profile_options(options: ProfileOptionsSnapshot) {
    if let Ok(mut cache) = options_cache().lock() {
        *cache = Some(options);
    }
}

/// The last published sidecar, if any.
pub fn last_profile_options() -> Option<ProfileOptionsSnapshot> {
    options_cache().lock().ok().and_then(|cache| cache.clone())
}

/// Drop the cached sidecar (profile switch, delete, restore).
pub fn clear_profile_options() {
    if let Ok(mut cache) = options_cache().lock() {
        *cache = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subscription_import::SubscriptionFilterDraft;

    #[test]
    fn empty_snapshot_is_recognized_from_the_stored_bytes() {
        let bare = ProfileOptionsSnapshot::new("main", "{}\n", SubscriptionFilterDraft::default());
        assert!(bare.is_empty());

        let filtered = ProfileOptionsSnapshot::new(
            "main",
            "{}\n",
            SubscriptionFilterDraft {
                include: "香港".to_owned(),
                ..SubscriptionFilterDraft::default()
            },
        );
        assert!(!filtered.is_empty());

        let mixed = ProfileOptionsSnapshot::new("main", "mode: rule\n", Default::default());
        assert!(!mixed.is_empty());
    }

    #[test]
    fn publish_read_and_clear_round_trip() {
        let snapshot = ProfileOptionsSnapshot::new(
            "main",
            "mode: rule\n",
            SubscriptionFilterDraft {
                dedup_index: 2,
                ..SubscriptionFilterDraft::default()
            },
        );
        publish_profile_options(snapshot.clone());
        assert_eq!(last_profile_options(), Some(snapshot));
        clear_profile_options();
        assert_eq!(last_profile_options(), None);
    }
}
