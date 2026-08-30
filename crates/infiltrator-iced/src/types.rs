//! Application types, grouped by business domain.
//!
//! Each submodule owns one domain's state/DTO types; every item is
//! re-exported here so existing `crate::types::X` paths stay stable.

pub use infiltrator_core::error::InfiltratorError;

mod app;
mod dns;
mod editor;
mod message;
mod perf;
mod rules;
mod runtime;

pub use app::{Route, ToastStatus, Transition};
pub use dns::{
    AdvancedConfigsBundle, AdvancedEditMode, AdvancedValidationState, DnsFormDraft, DnsTab,
    FakeIpFormDraft, TunFormDraft,
};
pub use editor::EditorLazyState;
pub use message::Message;
pub use rules::{RuleBadgeKind, RuleRenderItem, RulesJsonTab, RulesLoadBundle, RulesTab};
pub use perf::PerfSnapshot;
pub use runtime::{RebuildFlowState, RuntimeConfig, RuntimeStatus};
