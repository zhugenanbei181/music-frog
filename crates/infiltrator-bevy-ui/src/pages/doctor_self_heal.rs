//! BEVY-033: the Doctor page's context-aware self-heal card.
//!
//! The card is the product surface for the widget layer's
//! [`SelfHealSignalBoard`](infiltrator_bevy_widgets::auto_heal::SelfHealSignalBoard):
//! the best proxy node under the weighted heuristic and the typed anomaly
//! signals (unobserved / excessive latency / excessive loss / unhealthy
//! history). Every fact is derived from the live proxy catalogue; a node whose
//! latency or loss was never observed surfaces as a typed
//! `NodeUnobserved` anomaly and is never scored as if the missing metric were
//! zero. The single actionable control submits a typed
//! [`UiCommand::SelectProxyNode`] through the shared command sink — the page
//! never calls the controller directly.
//!
//! The card is a fixed `bsn!` structure restamped in place: the best-node text
//! and anomaly rows are updated by [`refresh_self_heal`], and the anomaly list
//! replaces its bounded subtree when the signal set changes. No page rebuild.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_button_scene;
use crate::surface::LatestSurfaceSnapshot;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{AlignItems, FlexDirection, FlexWrap, JustifyContent, Node, percent, px};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::auto_heal::{
    AnomalySeverity, ContextAwareHealDetector, NodeCandidate, NodeObservation, SelfHealAnomaly,
    SelfHealSignalBoard,
};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::surface_snapshot::ProxiesPageSnapshot;
use std::collections::{HashMap, HashSet};

/// Marker on the self-heal card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoctorSelfHealCard;

/// Marker on the best-node recommendation text node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoctorBestNodeText;

/// Marker on the container whose bounded subtree holds one row per anomaly.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoctorSelfHealAnomalyRows;

/// Stable identity of one rendered anomaly row (`"<node>:<kind>"`).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct DoctorSelfHealAnomalyRow(pub String);

/// Marker for the "apply best node" control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Button, ButtonDisabled)]
pub struct ApplyBestNodeButton;

/// The latest self-heal projection, refreshed from the live proxy catalogue.
///
/// The board is a read-only projection for the UI: it is never re-derived by
/// the scene, and `best_node` stays `None` until a candidate actually had the
/// observed facts required to be scored.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct DoctorSelfHealBoard {
    pub signals: SelfHealSignalBoard,
    /// Policy group owning the recommended node, if one was scored.
    pub best_group: Option<String>,
    /// Number of distinct proxy nodes observed in the catalogue.
    pub observed_nodes: usize,
}

impl DoctorSelfHealBoard {
    /// The `(group, node)` the apply intent should carry, if any.
    pub fn apply_target(&self) -> Option<(String, String)> {
        match (&self.best_group, &self.signals.best_node) {
            (Some(group), Some(node)) => Some((group.clone(), node.clone())),
            _ => None,
        }
    }
}

/// One anomaly row the card should display.
#[derive(Clone, Debug, PartialEq)]
struct SelfHealRow {
    id: String,
    text: String,
    color: Color,
}

/// Build the self-heal card structure. Dynamic text and rows are restamped in
/// place by [`refresh_self_heal`]; the tree is never rebuilt.
pub fn self_heal_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    let unknown = LocalizedText::plain("dns_self_heal_unknown").render(&UiLocale::default());
    surface_scene(
        vec![
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: px(space::S8),
                    }
                    Children [
                        LocalizedText::plain("proxies_title") TextRole(Role::BodyStrong)
                        --
                        Text(unknown) DoctorBestNodeText TextRole(Role::BodyStrong)
                    ]
            }),
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(space::S4),
                    }
                    DoctorSelfHealAnomalyRows
            }),
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: px(space::S8),
                        row_gap: px(space::S4),
                        flex_wrap: FlexWrap::Wrap,
                    }
                    Children [
                        LocalizedText::plain("auto_heal_loss_description") TextRole(Role::Caption)
                        --
                        @{ (
                            localized_button_scene(LocalizedText::plain("btn_switch"), ButtonVariant::Primary, palette),
                            bsn! { ApplyBestNodeButton ButtonDisabled(true) },
                        ) }
                    ]
            }),
        ],
        palette,
    )
}

/// Mutable surface access for the self-heal refresh system.
#[derive(SystemParam)]
pub struct SelfHealSurface<'w, 's> {
    commands: Commands<'w, 's>,
    texts: Query<'w, 's, SelfHealText>,
    roots: Query<'w, 's, (Entity, Option<&'static Children>), With<DoctorSelfHealAnomalyRows>>,
    buttons: Query<'w, 's, &'static mut ButtonDisabled, With<ApplyBestNodeButton>>,
}

/// Text nodes the refresh system restamps (best-node line and anomaly rows).
#[derive(QueryData)]
#[query_data(mutable)]
pub struct SelfHealText {
    text: &'static mut Text,
    color: Option<&'static mut TextColor>,
    best: Option<&'static DoctorBestNodeText>,
    row: Option<&'static DoctorSelfHealAnomalyRow>,
}

/// Derive node candidates from the observed proxy catalogue.
///
/// `delay_ms` is the observed latency. The controller-reported `alive` flag is
/// the only loss fact the catalogue carries (a reported dead node is 100%
/// loss); a node without the flag keeps loss unobserved, so it is never
/// silently scored. Returns the candidates and the first policy group owning
/// each node.
fn candidates_from_proxies(
    proxies: &ProxiesPageSnapshot,
) -> (Vec<NodeCandidate>, HashMap<String, String>) {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    let mut groups = HashMap::new();
    for group in &proxies.groups {
        for node in &group.proxies {
            if !seen.insert(node.name.clone()) {
                continue;
            }
            groups.insert(node.name.clone(), group.name.clone());
            let mut candidate = NodeCandidate::new(node.name.clone());
            candidate.observation = NodeObservation {
                latency_ms: node.delay_ms.map(|delay| delay as f32),
                loss_ratio: node.alive.map(|alive| if alive { 0.0 } else { 1.0 }),
            };
            candidates.push(candidate);
        }
    }
    (candidates, groups)
}

/// Stable row identity: node plus anomaly kind. A changing measurement updates
/// the existing row in place instead of churning the subtree.
fn anomaly_key(anomaly: &SelfHealAnomaly) -> String {
    let kind = match anomaly {
        SelfHealAnomaly::NodeUnobserved { .. } => "unobserved",
        SelfHealAnomaly::ExcessiveLatency { .. } => "latency",
        SelfHealAnomaly::ExcessiveLoss { .. } => "loss",
        SelfHealAnomaly::UnhealthyHistory { .. } => "history",
    };
    format!("{}:{kind}", anomaly.node_name())
}

fn severity_color(severity: AnomalySeverity, palette: &UiPalette) -> Color {
    match severity {
        AnomalySeverity::Info => palette.ink_dim,
        AnomalySeverity::Warning => palette.warning,
        AnomalySeverity::Critical => palette.danger,
    }
}

fn anomaly_label(anomaly: &SelfHealAnomaly, locale: &UiLocale) -> String {
    let key = match anomaly {
        SelfHealAnomaly::NodeUnobserved { .. } => "dns_self_heal_unknown",
        SelfHealAnomaly::ExcessiveLatency { .. } => "proxies_inspect_latency",
        SelfHealAnomaly::ExcessiveLoss { .. } => "speedtest_packet_loss",
        SelfHealAnomaly::UnhealthyHistory { .. } => "dns_self_heal_warning",
    };
    LocalizedText::plain(key).render(locale)
}

fn anomaly_value(anomaly: &SelfHealAnomaly) -> String {
    match anomaly {
        SelfHealAnomaly::NodeUnobserved { .. } => String::new(),
        SelfHealAnomaly::ExcessiveLatency { latency_ms, .. } => format!("{latency_ms:.0} ms"),
        SelfHealAnomaly::ExcessiveLoss { loss_ratio, .. } => format!("{:.0}%", loss_ratio * 100.0),
        SelfHealAnomaly::UnhealthyHistory { success_ratio, .. } => {
            format!("{:.0}%", success_ratio * 100.0)
        }
    }
}

fn desired_rows(
    anomalies: &[SelfHealAnomaly],
    locale: &UiLocale,
    palette: &UiPalette,
) -> Vec<SelfHealRow> {
    anomalies
        .iter()
        .map(|anomaly| {
            let label = anomaly_label(anomaly, locale);
            let value = anomaly_value(anomaly);
            let node = anomaly.node_name();
            let text = if value.is_empty() {
                format!("{node} · {label}")
            } else {
                format!("{node} · {label} {value}")
            };
            SelfHealRow {
                id: anomaly_key(anomaly),
                text,
                color: severity_color(anomaly.severity(), palette),
            }
        })
        .collect()
}

fn anomaly_row_scene(id: String, text: String, color: Color) -> impl Scene + use<> {
    bsn! {
            Text(text) DoctorSelfHealAnomalyRow(id) TextRole(Role::Caption) TextColor(color)
    }
}

/// Refresh the self-heal board from the latest proxy catalogue and restamp the
/// mounted card. Structure changes only when the anomaly identity set changes.
pub fn refresh_self_heal(
    snapshot: Option<Res<LatestSurfaceSnapshot>>,
    mut board: ResMut<DoctorSelfHealBoard>,
    locale: Option<Res<UiLocale>>,
    palette: Res<UiPalette>,
    mut surface: SelfHealSurface,
) {
    let fallback = UiLocale::default();
    let locale_ref = locale.as_deref().unwrap_or(&fallback);

    let (candidates, groups) = snapshot
        .as_deref()
        .and_then(|snapshot| snapshot.0.pages.proxies.data.as_ref())
        .map(candidates_from_proxies)
        .unwrap_or_default();

    let detector = ContextAwareHealDetector::new();
    let mut signals = SelfHealSignalBoard::new();
    signals.refresh(&detector, &candidates);
    let best_group = signals
        .best_node
        .as_ref()
        .and_then(|node| groups.get(node).cloned());
    let next = DoctorSelfHealBoard {
        signals,
        best_group,
        observed_nodes: candidates.len(),
    };
    if board.as_ref() != &next {
        *board = next;
    }

    let best_value = board
        .signals
        .best_node
        .clone()
        .unwrap_or_else(|| LocalizedText::plain("dns_self_heal_unknown").render(locale_ref));
    let rows = desired_rows(&board.signals.anomalies, locale_ref, &palette);

    for (root, children) in &surface.roots {
        let existing: HashMap<String, Entity> = children
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .filter_map(|entity| {
                surface
                    .texts
                    .get(entity)
                    .ok()
                    .and_then(|item| item.row.map(|row| (row.0.clone(), entity)))
            })
            .collect();
        for row in &rows {
            if !existing.contains_key(&row.id) {
                surface
                    .commands
                    .spawn_scene(anomaly_row_scene(
                        row.id.clone(),
                        row.text.clone(),
                        row.color,
                    ))
                    .insert(ChildOf(root));
            }
        }
        for (id, entity) in existing {
            if !rows.iter().any(|row| row.id == id) {
                surface.commands.entity(entity).despawn();
            }
        }
    }

    for mut item in &mut surface.texts {
        if item.best.is_some() {
            if item.text.0 != best_value {
                item.text.0 = best_value.clone();
            }
            continue;
        }
        let Some(row) = item.row else { continue };
        let Some(desired) = rows.iter().find(|desired| desired.id == row.0) else {
            continue;
        };
        if item.text.0 != desired.text {
            item.text.0 = desired.text.clone();
        }
        if let Some(color) = item.color.as_mut()
            && color.0 != desired.color
        {
            color.0 = desired.color;
        }
    }

    let disabled = board.apply_target().is_none();
    for mut button in &mut surface.buttons {
        if button.0 != disabled {
            button.0 = disabled;
        }
    }
}

/// Apply the recommended node through the shared command sink. The observer
/// submits a typed intent; it never calls a controller directly.
pub fn on_apply_best_node(
    activate: On<Activate>,
    buttons: Query<(), With<ApplyBestNodeButton>>,
    board: Res<DoctorSelfHealBoard>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    let Some(sink) = sink else { return };
    let Some((group, node)) = board.apply_target() else {
        return;
    };
    sink.submit(UiCommand::SelectProxyNode { group, node });
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::surface_snapshot::{ProxyGroupSnapshot, ProxyNodeSnapshot};

    fn proxies() -> ProxiesPageSnapshot {
        ProxiesPageSnapshot {
            name_runs: Default::default(),
            search_query: String::new(),
            node_details: Vec::new(),
            groups: vec![ProxyGroupSnapshot {
                name: "GLOBAL".to_owned(),
                group_type: "select".to_owned(),
                classification: None,
                current: "HK-Fast".to_owned(),
                expanded: true,
                proxies: vec![
                    ProxyNodeSnapshot {
                        name: "HK-Fast".to_owned(),
                        node_type: "ss".to_owned(),
                        delay_ms: Some(40),
                        alive: Some(true),
                        selected: true,
                        favorite: false,
                        features: Vec::new(),
                    },
                    ProxyNodeSnapshot {
                        name: "US-Lossy".to_owned(),
                        node_type: "ss".to_owned(),
                        delay_ms: Some(120),
                        alive: Some(false),
                        selected: false,
                        favorite: false,
                        features: Vec::new(),
                    },
                    ProxyNodeSnapshot {
                        name: "JP-Unknown".to_owned(),
                        node_type: "ss".to_owned(),
                        delay_ms: None,
                        alive: None,
                        selected: false,
                        favorite: false,
                        features: Vec::new(),
                    },
                ],
            }],
            testing: false,
            active_exit: String::new(),
            filter_alive: Default::default(),
            sort_order: Default::default(),
            compact_view: false,
            custom_node: Default::default(),
        }
    }

    #[test]
    fn missing_observations_stay_typed_unknown() {
        let (candidates, groups) = candidates_from_proxies(&proxies());
        assert_eq!(candidates.len(), 3);
        assert_eq!(groups.get("JP-Unknown").map(String::as_str), Some("GLOBAL"));

        let unknown = candidates
            .iter()
            .find(|candidate| candidate.name == "JP-Unknown")
            .expect("unknown node present");
        assert_eq!(unknown.observation.latency_ms, None);
        assert_eq!(unknown.observation.loss_ratio, None);

        let detector = ContextAwareHealDetector::new();
        let mut board = SelfHealSignalBoard::new();
        board.refresh(&detector, &candidates);
        assert_eq!(board.best_node.as_deref(), Some("HK-Fast"));
        assert!(board.anomalies.iter().any(|anomaly| matches!(
            anomaly,
            SelfHealAnomaly::NodeUnobserved { node, .. } if node == "JP-Unknown"
        )));
        assert!(board.anomalies.iter().any(|anomaly| matches!(
            anomaly,
            SelfHealAnomaly::ExcessiveLoss { node, .. } if node == "US-Lossy"
        )));
    }

    #[test]
    fn apply_target_is_absent_without_a_scored_node() {
        let board = DoctorSelfHealBoard::default();
        assert_eq!(board.apply_target(), None);
    }
}
