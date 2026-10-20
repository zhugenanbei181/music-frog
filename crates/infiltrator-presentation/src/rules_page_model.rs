//! The one typed page model of the rules page, shared by every surface.
//!
//! The rules workspace already shared its tabs, its rows and its JSON
//! projection. What was still written twice were the eight operation phases
//! of the page: Iced kept one boolean-ish phase per operation, Bevy kept its
//! own resources, and neither surface could answer "is the rules page busy
//! and with what" the same way.
//!
//! This module owns those phases. Each operation is one typed phase machine
//! with its own token, so a late answer to one operation can never move
//! another operation's phase. The JSON document buffers are typed drafts with
//! their own baseline, so the page can no longer carry a `Vec<bool>` of
//! dirty flags beside the documents it edits.

use crate::editable_draft::EditableDraft;
use crate::operation_phase::OperationPhase;
use infiltrator_contract::rules_workspace::{
    RulesJsonDocumentSnapshot, RulesJsonSection, RulesTab,
};

/// The rules page operations the shared model tracks.
///
/// The eight operations are the ones the rules workspace owns; every surface
/// answers "what is the rules page doing" from this one list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulesOperation {
    /// Load the rule list.
    Load,
    /// Save the rule list.
    Save,
    /// Save the `rule-providers` JSON document.
    SaveRuleProvidersJson,
    /// Save the `proxy-providers` JSON document.
    SaveProxyProvidersJson,
    /// Save the `sniffer` JSON document.
    SaveSnifferJson,
    /// Update the GeoIP and GeoSite databases.
    UpdateGeoDatabases,
    /// Add one rule.
    AddRule,
    /// Load a rule-provider diff.
    LoadProviderDiff,
}

impl RulesOperation {
    /// Every operation, in presentation order.
    pub const ALL: [Self; 8] = [
        Self::Load,
        Self::Save,
        Self::SaveRuleProvidersJson,
        Self::SaveProxyProvidersJson,
        Self::SaveSnifferJson,
        Self::UpdateGeoDatabases,
        Self::AddRule,
        Self::LoadProviderDiff,
    ];

    /// Position of the operation in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::Load => 0,
            Self::Save => 1,
            Self::SaveRuleProvidersJson => 2,
            Self::SaveProxyProvidersJson => 3,
            Self::SaveSnifferJson => 4,
            Self::UpdateGeoDatabases => 5,
            Self::AddRule => 6,
            Self::LoadProviderDiff => 7,
        }
    }

    /// The JSON section an operation saves, when it saves one.
    pub const fn json_section(self) -> Option<RulesJsonSection> {
        match self {
            Self::SaveRuleProvidersJson => Some(RulesJsonSection::RuleProviders),
            Self::SaveProxyProvidersJson => Some(RulesJsonSection::ProxyProviders),
            Self::SaveSnifferJson => Some(RulesJsonSection::Sniffer),
            Self::Load
            | Self::Save
            | Self::UpdateGeoDatabases
            | Self::AddRule
            | Self::LoadProviderDiff => None,
        }
    }
}

/// The one page model of the rules page.
///
/// A surface renders the rules page from this model alone: the active tab,
/// the active JSON section, the JSON document buffers and every operation
/// phase come from here, and the surface only dispatches typed intents back.
#[derive(Clone, Debug, Default)]
pub struct RulesPageModel {
    tab: RulesTab,
    json_section: RulesJsonSection,
    rule_providers_json: EditableDraft<String>,
    proxy_providers_json: EditableDraft<String>,
    sniffer_json: EditableDraft<String>,
    load: OperationPhase,
    save: OperationPhase,
    save_rule_providers_json: OperationPhase,
    save_proxy_providers_json: OperationPhase,
    save_sniffer_json: OperationPhase,
    update_geo_databases: OperationPhase,
    add_rule: OperationPhase,
    load_provider_diff: OperationPhase,
}

impl RulesPageModel {
    /// The active tab of the rules workspace.
    pub fn tab(&self) -> RulesTab {
        self.tab
    }

    /// Select the active tab of the rules workspace.
    pub fn select_tab(&mut self, tab: RulesTab) {
        self.tab = tab;
    }

    /// Select the tab a control position names.
    pub fn select_tab_at(&mut self, index: usize) {
        self.tab = RulesTab::from_index(index);
    }

    /// The position of the active tab in the tab strip.
    pub fn tab_index(&self) -> usize {
        self.tab.index()
    }

    /// The active JSON section of the JSON editors partition.
    pub fn json_section(&self) -> RulesJsonSection {
        self.json_section
    }

    /// Select the active JSON section of the JSON editors partition.
    pub fn select_json_section(&mut self, section: RulesJsonSection) {
        self.json_section = section;
    }

    /// The buffer of one JSON section.
    pub fn json_buffer(&self, section: RulesJsonSection) -> &EditableDraft<String> {
        match section {
            RulesJsonSection::RuleProviders => &self.rule_providers_json,
            RulesJsonSection::ProxyProviders => &self.proxy_providers_json,
            RulesJsonSection::Sniffer => &self.sniffer_json,
        }
    }

    /// The buffer of one JSON section, for the fold that changes it.
    pub fn json_buffer_mut(&mut self, section: RulesJsonSection) -> &mut EditableDraft<String> {
        match section {
            RulesJsonSection::RuleProviders => &mut self.rule_providers_json,
            RulesJsonSection::ProxyProviders => &mut self.proxy_providers_json,
            RulesJsonSection::Sniffer => &mut self.sniffer_json,
        }
    }

    /// The text the buffer of one JSON section shows.
    pub fn json_text(&self, section: RulesJsonSection) -> &str {
        self.json_buffer(section).value()
    }

    /// Whether the buffer of one JSON section holds an unaccepted edit.
    pub fn json_modified(&self, section: RulesJsonSection) -> bool {
        self.json_buffer(section).is_modified()
    }

    /// Adopt the JSON documents the owner published.
    ///
    /// A buffer the user edited is never clobbered: the published text
    /// becomes the new baseline and the edit stays visible.
    pub fn observe_json_documents(&mut self, documents: &[RulesJsonDocumentSnapshot]) {
        for document in documents {
            self.observe_json_document(document.section, document.json.clone());
        }
    }

    /// Adopt the text the owner published for one JSON section.
    pub fn observe_json_document(&mut self, section: RulesJsonSection, text: String) {
        self.json_buffer_mut(section).adopt(text);
    }

    /// The phase of one operation.
    pub fn operation(&self, operation: RulesOperation) -> &OperationPhase {
        match operation {
            RulesOperation::Load => &self.load,
            RulesOperation::Save => &self.save,
            RulesOperation::SaveRuleProvidersJson => &self.save_rule_providers_json,
            RulesOperation::SaveProxyProvidersJson => &self.save_proxy_providers_json,
            RulesOperation::SaveSnifferJson => &self.save_sniffer_json,
            RulesOperation::UpdateGeoDatabases => &self.update_geo_databases,
            RulesOperation::AddRule => &self.add_rule,
            RulesOperation::LoadProviderDiff => &self.load_provider_diff,
        }
    }

    /// The phase of one operation, for the fold that changes it.
    pub fn operation_mut(&mut self, operation: RulesOperation) -> &mut OperationPhase {
        match operation {
            RulesOperation::Load => &mut self.load,
            RulesOperation::Save => &mut self.save,
            RulesOperation::SaveRuleProvidersJson => &mut self.save_rule_providers_json,
            RulesOperation::SaveProxyProvidersJson => &mut self.save_proxy_providers_json,
            RulesOperation::SaveSnifferJson => &mut self.save_sniffer_json,
            RulesOperation::UpdateGeoDatabases => &mut self.update_geo_databases,
            RulesOperation::AddRule => &mut self.add_rule,
            RulesOperation::LoadProviderDiff => &mut self.load_provider_diff,
        }
    }

    /// Whether any operation of the page is still in flight.
    pub fn is_busy(&self) -> bool {
        RulesOperation::ALL
            .iter()
            .any(|operation| self.operation(*operation).is_running())
    }

    /// The operations of the page that are still in flight.
    pub fn running_operations(&self) -> Vec<RulesOperation> {
        RulesOperation::ALL
            .into_iter()
            .filter(|operation| self.operation(*operation).is_running())
            .collect()
    }
}

#[cfg(test)]
#[path = "rules_page_model_tests.rs"]
mod tests;
