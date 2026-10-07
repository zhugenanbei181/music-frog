//! Rules & DNS/advanced-config tests: render cache + filtering, pagination
//! bounds, lazy JSON editors, form drafts, validation and heavy-sample smoke.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior
#[path = "rules_dns_tests/rule_list.rs"]
mod rule_list;

use crate::state::AppState;
use crate::types::dns::{AdvancedConfigsBundle, AdvancedEditMode, DnsTab};
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::rules::RuleBadgeKind;
use crate::types::runtime::RebuildFlowState;
use crate::view::components::BadgeKind;
use crate::view::rules::{display_rule_type, semantic_badge_kind};
use infiltrator_domain::rules::RuleEntry;

// ---- LEFT-05 L1 / DUAL-09-01: editor saves keep comments -------------------

#[path = "rules_dns_tests/advanced.rs"]
mod advanced;
#[path = "rules_dns_tests/dns.rs"]
mod dns;
#[path = "rules_dns_tests/mixin.rs"]
mod mixin;
#[path = "rules_dns_tests/rule.rs"]
mod rule;
#[path = "rules_dns_tests/rules_dns.rs"]
mod rules_dns;
#[path = "rules_dns_tests/rules_edit.rs"]
mod rules_edit;
#[path = "rules_dns_tests/rules_filter.rs"]
mod rules_filter;
#[path = "rules_dns_tests/rules_game.rs"]
mod rules_game;
#[path = "rules_dns_tests/rules_pagination.rs"]
mod rules_pagination;
#[path = "rules_dns_tests/rules_provider.rs"]
mod rules_provider;
#[path = "rules_dns_tests/rules_render.rs"]
mod rules_render;
#[path = "rules_dns_tests/rules_rule.rs"]
mod rules_rule;
#[path = "rules_dns_tests/rules_tracer.rs"]
pub(super) mod rules_tracer;
#[path = "rules_dns_tests/rules_type.rs"]
mod rules_type;
#[path = "rules_dns_tests/rules_virtual.rs"]
mod rules_virtual;
#[path = "rules_dns_tests/rules_workspace.rs"]
mod rules_workspace;
#[path = "rules_dns_tests/set.rs"]
mod set;
#[path = "rules_dns_tests/tun.rs"]
mod tun;

#[path = "rules_dns_tests/statistics.rs"]
mod statistics;
