//! The one typed page model of the DNS page, shared by every surface.
//!
//! The DNS page had three per-surface pieces of state: the partition the user
//! looks at, whether the page shows the structured form or the raw JSON, and
//! the phase of each configuration write. Iced kept all three privately —
//! including its own `DnsTab` and `AdvancedEditMode` enums — while the
//! shared layer already owned the DNS form projections.
//!
//! This module owns that page state. The tab and the edit mode are one typed
//! value each, and every configuration write is one typed phase machine with
//! its own token, so the page can never show "saving" next to "failed".

use crate::operation_phase::OperationPhase;
use infiltrator_contract::dns::{DnsEnhancedMode, DnsFakeIpFilterMode};

/// The partitions of the DNS page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DnsPageTab {
    /// The DNS resolver configuration.
    #[default]
    Dns,
    /// The Fake-IP configuration.
    FakeIp,
    /// The TUN configuration.
    Tun,
}

impl DnsPageTab {
    /// Every partition, in presentation order.
    pub const ALL: [Self; 3] = [Self::Dns, Self::FakeIp, Self::Tun];

    /// Position of the partition in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::Dns => 0,
            Self::FakeIp => 1,
            Self::Tun => 2,
        }
    }

    /// The partition a control position names.
    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::FakeIp,
            2 => Self::Tun,
            _ => Self::Dns,
        }
    }
}

/// How the DNS page presents its configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DnsEditMode {
    /// The structured form.
    #[default]
    Form,
    /// The raw JSON document.
    Json,
}

/// The DNS page operations the shared model tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnsOperation {
    /// Save the DNS resolver configuration.
    SaveDns,
    /// Save the Fake-IP configuration.
    SaveFakeIp,
    /// Save the TUN configuration.
    SaveTun,
    /// Probe the latency of the configured resolvers.
    ProbeLatency,
}

impl DnsOperation {
    /// Every operation, in presentation order.
    pub const ALL: [Self; 4] = [
        Self::SaveDns,
        Self::SaveFakeIp,
        Self::SaveTun,
        Self::ProbeLatency,
    ];

    /// Position of the operation in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::SaveDns => 0,
            Self::SaveFakeIp => 1,
            Self::SaveTun => 2,
            Self::ProbeLatency => 3,
        }
    }
}

/// The one page model of the DNS page.
///
/// A surface renders the DNS page from this model alone: the active
/// partition, the edit mode and every operation phase come from here, and the
/// surface only dispatches typed intents back.
#[derive(Clone, Debug, Default)]
pub struct DnsPageModel {
    tab: DnsPageTab,
    edit_mode: DnsEditMode,
    enhanced_mode: DnsEnhancedMode,
    filter_mode: DnsFakeIpFilterMode,
    save_dns: OperationPhase,
    save_fake_ip: OperationPhase,
    save_tun: OperationPhase,
    probe_latency: OperationPhase,
}

impl DnsPageModel {
    /// The active partition of the page.
    pub fn tab(&self) -> DnsPageTab {
        self.tab
    }

    /// Select the active partition of the page.
    pub fn select_tab(&mut self, tab: DnsPageTab) {
        self.tab = tab;
    }

    /// Select the partition a control position names.
    pub fn select_tab_at(&mut self, index: usize) {
        self.tab = DnsPageTab::from_index(index);
    }

    /// The position of the active partition in the partition strip.
    pub fn tab_index(&self) -> usize {
        self.tab.index()
    }

    /// How the page presents its configuration.
    pub fn edit_mode(&self) -> DnsEditMode {
        self.edit_mode
    }

    /// Select how the page presents its configuration.
    pub fn select_edit_mode(&mut self, mode: DnsEditMode) {
        self.edit_mode = mode;
    }

    /// Fold the owner's DNS modes into the page model.
    pub fn observe_modes(
        &mut self,
        enhanced_mode: DnsEnhancedMode,
        filter_mode: DnsFakeIpFilterMode,
    ) {
        self.enhanced_mode = enhanced_mode;
        self.filter_mode = filter_mode;
    }

    /// The domain mapping mode the owner reported.
    pub fn enhanced_mode(&self) -> DnsEnhancedMode {
        self.enhanced_mode
    }

    /// The Fake-IP filter mode the owner reported.
    pub fn filter_mode(&self) -> DnsFakeIpFilterMode {
        self.filter_mode
    }

    /// The phase of one operation.
    pub fn operation(&self, operation: DnsOperation) -> &OperationPhase {
        match operation {
            DnsOperation::SaveDns => &self.save_dns,
            DnsOperation::SaveFakeIp => &self.save_fake_ip,
            DnsOperation::SaveTun => &self.save_tun,
            DnsOperation::ProbeLatency => &self.probe_latency,
        }
    }

    /// The phase of one operation, for the fold that changes it.
    pub fn operation_mut(&mut self, operation: DnsOperation) -> &mut OperationPhase {
        match operation {
            DnsOperation::SaveDns => &mut self.save_dns,
            DnsOperation::SaveFakeIp => &mut self.save_fake_ip,
            DnsOperation::SaveTun => &mut self.save_tun,
            DnsOperation::ProbeLatency => &mut self.probe_latency,
        }
    }

    /// Whether any operation of the page is still in flight.
    pub fn is_busy(&self) -> bool {
        DnsOperation::ALL
            .iter()
            .any(|operation| self.operation(*operation).is_running())
    }

    /// The operations of the page that are still in flight.
    pub fn running_operations(&self) -> Vec<DnsOperation> {
        DnsOperation::ALL
            .into_iter()
            .filter(|operation| self.operation(*operation).is_running())
            .collect()
    }
}

#[cfg(test)]
#[path = "dns_page_model_tests.rs"]
mod tests;
