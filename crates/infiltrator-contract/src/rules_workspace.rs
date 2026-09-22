//! Shared rules-workspace partition vocabulary (DUAL-11-14).
//!
//! Both surfaces expose the same three rule-management capability groups and
//! the same JSON document sections. The *presentation* differs (Iced splits
//! the workspace into four page tabs, Bevy mounts the same four partitions in
//! one scrolling page and switches their visibility), so the identity and
//! order of every partition lives here once instead of being re-invented per
//! surface.

use serde::{Deserialize, Serialize};

/// The four rule-management partitions both surfaces expose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RulesTab {
    /// The ordered rule list: search, paging, virtual window, edits.
    #[default]
    List,
    /// Rule-provider and MRS lifecycle cards.
    Providers,
    /// The three JSON documents of the rule workspace.
    JsonEditors,
    /// The interactive rule tracer / hit audit.
    Tracer,
}

impl RulesTab {
    /// Every partition, in presentation order.
    pub const ALL: [Self; 4] = [Self::List, Self::Providers, Self::JsonEditors, Self::Tracer];

    /// Position of this partition in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::List => 0,
            Self::Providers => 1,
            Self::JsonEditors => 2,
            Self::Tracer => 3,
        }
    }

    /// Partition at `index`, falling back to the list partition.
    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Providers,
            2 => Self::JsonEditors,
            3 => Self::Tracer,
            _ => Self::List,
        }
    }

    /// I18n key of the partition label. The Iced surface resolves it through
    /// its language table; Bevy renders its own bare-Chinese label for the
    /// same partition.
    pub const fn i18n_key(self) -> &'static str {
        match self {
            Self::List => "rules_tab_list",
            Self::Providers => "rules_tab_providers",
            Self::JsonEditors => "rules_tab_json",
            Self::Tracer => "rules_tab_tracer",
        }
    }
}

/// The three JSON documents the rules JSON partition edits. Each maps onto one
/// shared `ConfigurationApplication::save_*` use-case, so the intent carries
/// only the section identity and the edited text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RulesJsonSection {
    /// `rule-providers` of the active profile.
    #[default]
    RuleProviders,
    /// `proxy-providers` of the active profile.
    ProxyProviders,
    /// `sniffer` of the active profile.
    Sniffer,
}

impl RulesJsonSection {
    /// Every JSON section, in presentation order.
    pub const ALL: [Self; 3] = [Self::RuleProviders, Self::ProxyProviders, Self::Sniffer];

    /// Position of this section in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::RuleProviders => 0,
            Self::ProxyProviders => 1,
            Self::Sniffer => 2,
        }
    }

    /// Section at `index`, falling back to the rule-providers document.
    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::ProxyProviders,
            2 => Self::Sniffer,
            _ => Self::RuleProviders,
        }
    }

    /// I18n key of the section label (Iced resolves it; Bevy has its own).
    pub const fn i18n_key(self) -> &'static str {
        match self {
            Self::RuleProviders => "rules_rule_providers_json",
            Self::ProxyProviders => "rules_proxy_providers_json",
            Self::Sniffer => "rules_sniffer_json",
        }
    }

    /// I18n key of the section's save action.
    pub const fn save_i18n_key(self) -> &'static str {
        match self {
            Self::RuleProviders => "rules_save_rule_providers_btn",
            Self::ProxyProviders => "rules_save_proxy_providers_btn",
            Self::Sniffer => "rules_save_sniffer_btn",
        }
    }
}

/// One published JSON document of the rules workspace. The shared reader
/// serialises the same document the Iced surface loads through its ports, so
/// both surfaces show the same text and hand the same text back to the shared
/// save use-cases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesJsonDocumentSnapshot {
    pub section: RulesJsonSection,
    /// Pretty-printed JSON of the document, exactly what the editor shows.
    pub json: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partitions_round_trip_through_their_index() {
        assert_eq!(RulesTab::ALL.len(), 4);
        assert_eq!(RulesJsonSection::ALL.len(), 3);
        for tab in RulesTab::ALL {
            assert_eq!(RulesTab::from_index(tab.index()), tab);
        }
        for section in RulesJsonSection::ALL {
            assert_eq!(RulesJsonSection::from_index(section.index()), section);
        }
        assert_eq!(RulesTab::from_index(99), RulesTab::List);
        assert_eq!(
            RulesJsonSection::from_index(99),
            RulesJsonSection::RuleProviders
        );
        assert_eq!(RulesTab::default(), RulesTab::List);
        assert_eq!(RulesJsonSection::default(), RulesJsonSection::RuleProviders);
    }

    #[test]
    fn every_partition_has_a_distinct_label_key() {
        let tab_keys: Vec<&str> = RulesTab::ALL.iter().map(|tab| tab.i18n_key()).collect();
        let mut dedup = tab_keys.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), tab_keys.len());
        assert!(tab_keys.iter().all(|key| key.starts_with("rules_")));

        let section_keys: Vec<&str> = RulesJsonSection::ALL
            .iter()
            .map(|section| section.i18n_key())
            .collect();
        let mut dedup = section_keys.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), section_keys.len());
        for section in RulesJsonSection::ALL {
            assert!(section.save_i18n_key().starts_with("rules_save_"));
        }
    }

    #[test]
    fn json_document_snapshot_round_trips() {
        let document = RulesJsonDocumentSnapshot {
            section: RulesJsonSection::Sniffer,
            json: "{\n  \"enable\": true\n}".to_owned(),
        };
        let encoded = serde_json::to_string(&document).unwrap();
        let decoded: RulesJsonDocumentSnapshot = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, document);
    }
}
