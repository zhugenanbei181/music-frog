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

use crate::profile_source::ProfileSourceIdentity;
use crate::subscription_import::SubscriptionFilterDraft;
use serde::{Deserialize, Serialize};

/// The stored options of one profile, in surface-editable form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileOptionsSnapshot {
    pub source: ProfileSourceIdentity,
    /// The stored Mixin overlay serialized as YAML — the document the Mixin
    /// editor pane opens (the same bytes `serde_yaml_ng` produces for the
    /// stored `MixinConfig`, byte-stable across both surfaces).
    pub mixin_yaml: String,
    /// The stored subscription filter as the shared surface draft.
    pub filter: SubscriptionFilterDraft,
}

impl ProfileOptionsSnapshot {
    pub fn new(
        source: ProfileSourceIdentity,
        mixin_yaml: impl Into<String>,
        filter: SubscriptionFilterDraft,
    ) -> Self {
        Self {
            source,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subscription_import::SubscriptionFilterDraft;
    fn source() -> ProfileSourceIdentity {
        ProfileSourceIdentity {
            profile: "main".into(),
            document_hash: "contract-fixture".into(),
            options_hash: None,
        }
    }

    #[test]
    fn empty_snapshot_is_recognized_from_the_stored_bytes() {
        let bare =
            ProfileOptionsSnapshot::new(source(), "{}\n", SubscriptionFilterDraft::default());
        assert!(bare.is_empty());

        let filtered = ProfileOptionsSnapshot::new(
            source(),
            "{}\n",
            SubscriptionFilterDraft {
                include: "香港".to_owned(),
                ..SubscriptionFilterDraft::default()
            },
        );
        assert!(!filtered.is_empty());

        let mixed = ProfileOptionsSnapshot::new(source(), "mode: rule\n", Default::default());
        assert!(!mixed.is_empty());
    }

    #[test]
    fn serialized_options_preserve_source_and_raw_filter_data() {
        let snapshot = ProfileOptionsSnapshot::new(
            source(),
            "mode: rule\n",
            SubscriptionFilterDraft {
                include: "用户 {source}".into(),
                ..Default::default()
            },
        );
        let bytes = serde_json::to_string(&snapshot).unwrap();
        let decoded: ProfileOptionsSnapshot = serde_json::from_str(&bytes).unwrap();
        assert_eq!(decoded, snapshot);
        assert_eq!(decoded.filter.include, "用户 {source}");
    }
}
