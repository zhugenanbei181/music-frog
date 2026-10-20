//! Proxy node card rendering with feature chips, favorite pin, trend waveforms, and toolbar controls.
//!
//! Subtree scenes composing the individual proxy node card (BEVY-GAP-031),
//! favorite pinning toggle and trend wave (BEVY-GAP-033), protocol & UDP chips (BEVY-GAP-035),
//! proxies toolbar controls ("只看可用", 4-sort pills, "测试地址"), and group cards.
//!
//! The scenes are split along business seams:
//!
//! * `toolbar` — page chrome: the search/toolbar card and the header summary card.
//! * `node_card` — the single proxy node card vocabulary (chips, favorite pin, latency trend).
//! * `group_card` — the strategy group card and its header-only variant.
//!
//! Each seam is a public module, so callers import from the defining module's
//! canonical path (the repository bans re-export facades):
//! `infiltrator_bevy_ui::pages::proxies_card::node_card::proxy_node_scene`.

pub mod group_card;
pub mod node_card;
pub mod toolbar;
