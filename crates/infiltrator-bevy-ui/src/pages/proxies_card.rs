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
//! The re-exports below keep the public paths stable:
//! `infiltrator_bevy_ui::pages::proxies_card::*`.

mod group_card;
mod node_card;
mod toolbar;

pub use group_card::{group_card_scene, group_header_scene};
pub use node_card::{ProxyNodeSceneContext, proxy_node_scene, proxy_node_scene_sized};
pub use toolbar::{header_card_scene, search_bar_card_scene};
