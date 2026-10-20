//! Rules-domain types: the rules load bundle, page tabs, tracer state,
//! and the rendered rule rows consumed by the rules view.

use infiltrator_domain::rules::tracer::RuleTraceMatch;
use lurker_core_contract::error::Failure;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleBadgeKind {
    Domain,
    Ip,
    #[default]
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct RuleRenderItem {
    pub hit_count: Option<u64>,
    pub is_shadowed: bool,
    pub source_ip: bool,
    pub no_resolve: bool,
    pub failure: Option<Failure>,
    pub source_index: usize,
    pub rule_type: String,
    pub payload: String,
    pub target: String,
    pub badge: RuleBadgeKind,
}

/// State for the interactive Live Rule Tracer sandbox.
#[derive(Debug, Clone, Default)]
pub struct RuleTracerState {
    pub query: String,
    pub port_input: String,
    pub process_input: String,
    pub in_type_input: String,
    pub match_result: Option<RuleTraceMatch>,
    pub trace_performed: bool,
}

/// The single stage of the rule-provider maintenance card.
///
/// A provider read and a local cache purge are mutually exclusive stages of
/// one process, so the stage is one typed value. Two flags would encode the
/// same table implicitly and could both be set at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderMaintenanceStage {
    /// No provider read and no cache purge is in flight.
    #[default]
    Idle,
    /// DUAL-11-06: a provider read is in flight.
    Unpacking,
    /// The rule-provider cache purge is in flight.
    PurgingCache,
}

impl ProviderMaintenanceStage {
    /// True while a provider read or a cache purge is in flight.
    pub fn is_busy(self) -> bool {
        !matches!(self, ProviderMaintenanceStage::Idle)
    }
}

/// State for Rule-Provider unpacking and local cache purging.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderUnpackState {
    pub unpacked_rules_count: usize,
    /// The one stage the maintenance card is in.
    pub stage: ProviderMaintenanceStage,
    /// Last honest outcome ("provider · N rules · origin …" or the typed
    /// failure reason). Never a fabricated success string.
    pub status_message: Option<String>,
}

impl ProviderUnpackState {
    /// Begin a provider read. A read requested while the card is already busy
    /// is refused with the running stage instead of overwriting it.
    pub fn begin_unpack(&mut self) -> Result<(), ProviderMaintenanceStage> {
        match self.stage {
            ProviderMaintenanceStage::Idle => {
                self.stage = ProviderMaintenanceStage::Unpacking;
                Ok(())
            }
            busy => Err(busy),
        }
    }

    /// Begin a cache purge, refused the same way while the card is busy.
    pub fn begin_cache_purge(&mut self) -> Result<(), ProviderMaintenanceStage> {
        match self.stage {
            ProviderMaintenanceStage::Idle => {
                self.stage = ProviderMaintenanceStage::PurgingCache;
                Ok(())
            }
            busy => Err(busy),
        }
    }

    /// The in-flight stage finished; the card returns to idle.
    pub fn finish(&mut self) {
        self.stage = ProviderMaintenanceStage::Idle;
    }
}
