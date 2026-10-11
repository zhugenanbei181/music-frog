//! BEVY-033: the Doctor page's context-aware self-heal card.
//!
//! The card is the product surface for the widget layer's
//! [`SelfHealSignalBoard`](infiltrator_bevy_widgets::auto_heal::SelfHealSignalBoard):
//! the best proxy node under the weighted heuristic and the typed anomaly
//! signals (unobserved / excessive latency / excessive loss / unhealthy
//! history). Every fact is derived from the live proxy catalogue; a node whose
//! latency or loss was never observed surfaces as a typed
//! `NodeUnobserved` anomaly and is never scored as if the missing metric were
//! zero. The single actionable control resolves the recommendation against
//! the latest accepted neutral outbound inventory and submits a revision-fenced
//! [`UiCommand::SelectOutbound`] through the shared command sink.
//!
//! The card is a fixed `bsn!` structure restamped in place: the best-node text
//! and anomaly rows are updated by [`refresh_self_heal`], and the anomaly list
//! replaces its bounded subtree when the signal set changes. No page rebuild.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_button_scene;
use crate::pages::proxies::outbound_selection_request;
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
use bevy::ui::prelude::{AlignItems, FlexDirection, FlexWrap, JustifyContent, Node, percent};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::auto_heal::{
    AnomalySeverity, ContextAwareHealDetector, NodeCandidate, NodeObservation, SelfHealAnomaly,
    SelfHealSignalBoard,
};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::outbound::{identity::OutboundId, inventory::OutboundInventoryState};
use infiltrator_contract::speedtest::SpeedtestOutboundResult;
use infiltrator_contract::surface_snapshot::{PageStatus, SurfaceSnapshot};
use std::collections::HashMap;

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

/// The latest self-heal projection, refreshed from the accepted outbound
/// inventory and matching speedtest results.
///
/// The board is a read-only projection for the UI: it is never re-derived by
/// the scene, and `best_node` stays `None` until a candidate actually had the
/// observed facts required to be scored.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct DoctorSelfHealBoard {
    pub signals: SelfHealSignalBoard,
    /// Policy group owning the recommended node, if one was scored.
    pub best_target: Option<(OutboundId, OutboundId)>,
    /// Number of distinct proxy nodes observed in the catalogue.
    pub observed_nodes: usize,
}

impl DoctorSelfHealBoard {
    /// The `(group, node)` the apply intent should carry, if any.
    pub fn apply_target(&self) -> Option<(OutboundId, OutboundId)> {
        self.best_target.clone()
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
                        column_gap: rem(space::S8),
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
                        row_gap: rem(space::S4),
                    }
                    DoctorSelfHealAnomalyRows
            }),
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S8),
                        row_gap: rem(space::S4),
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

/// Derive candidates from the accepted neutral inventory and exact matching
/// speedtest results. Missing probe facts remain unobserved, and duplicate
/// display labels stay non-actionable because they cannot identify one opaque
/// outbound safely.
fn candidates_from_surface(
    snapshot: &SurfaceSnapshot,
) -> (
    Vec<NodeCandidate>,
    HashMap<String, (OutboundId, OutboundId)>,
) {
    if !matches!(
        &snapshot.outbound_inventory.status,
        PageStatus::Ready | PageStatus::Empty
    ) {
        return (Vec::new(), HashMap::new());
    }
    let Some(inventory) = snapshot.outbound_inventory.data.as_ref() else {
        return (Vec::new(), HashMap::new());
    };
    if snapshot.core.scope().as_ref() != Some(inventory.scope()) {
        return (Vec::new(), HashMap::new());
    }
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        return (Vec::new(), HashMap::new());
    };

    let current_speedtest = snapshot
        .speedtest
        .state
        .target()
        .is_some_and(|target| target.core_scope() == inventory.scope())
        .then_some(&snapshot.speedtest);
    let mut candidates = Vec::new();
    let mut groups = HashMap::new();
    for outbound in entries {
        let name = outbound
            .display_name
            .clone()
            .unwrap_or_else(|| outbound.id.value().to_owned());
        let speedtest = current_speedtest.and_then(|speedtest| speedtest.result(&outbound.id));
        let mut candidate = NodeCandidate::new(name.clone());
        candidate.observation = NodeObservation {
            latency_ms: speedtest.and_then(|result| result.delay_ms().map(|delay| delay as f32)),
            loss_ratio: speedtest.and_then(observed_loss_ratio),
        };
        candidates.push(candidate);
        if let Some(collection) = collections
            .iter()
            .find(|collection| collection.member_ids.contains(&outbound.id))
        {
            groups.insert(name, (collection.id.clone(), outbound.id.clone()));
        }
    }
    (candidates, groups)
}

fn observed_loss_ratio(result: &SpeedtestOutboundResult) -> Option<f32> {
    result
        .calculation()
        .map(|calculation| (calculation.loss_percent / 100.0) as f32)
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

/// Refresh the self-heal board from the latest neutral outbound inventory and
/// restamp the mounted card. Structure changes only when anomaly identities
/// change.
pub fn refresh_self_heal(
    snapshot: Option<Res<LatestSurfaceSnapshot>>,
    mut board: ResMut<DoctorSelfHealBoard>,
    locale: Option<Res<UiLocale>>,
    palette: Res<UiPalette>,
    mut surface: SelfHealSurface,
) {
    let fallback = UiLocale::default();
    let locale_ref = locale.as_deref().unwrap_or(&fallback);

    let (candidates, targets) = snapshot
        .as_deref()
        .map(|snapshot| candidates_from_surface(&snapshot.0))
        .unwrap_or_default();

    let detector = ContextAwareHealDetector::new();
    let mut signals = SelfHealSignalBoard::new();
    signals.refresh(&detector, &candidates);
    let best_target = signals
        .best_node
        .as_ref()
        .and_then(|node| targets.get(node).cloned());
    let next = DoctorSelfHealBoard {
        signals,
        best_target,
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

    let disabled = board.apply_target().is_none_or(|(group, node)| {
        snapshot
            .as_deref()
            .and_then(|snapshot| outbound_selection_request(&snapshot.0, &group, &node))
            .is_none()
    });
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
    latest: Option<Res<LatestSurfaceSnapshot>>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    let Some(sink) = sink else { return };
    let Some((group, node)) = board.apply_target() else {
        return;
    };
    let Some(request) = latest
        .as_deref()
        .and_then(|snapshot| outbound_selection_request(&snapshot.0, &group, &node))
    else {
        return;
    };
    sink.submit(UiCommand::SelectOutbound { request });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::{DemoSurfaceSource, SurfaceSource};
    use infiltrator_contract::outbound::probe::OutboundProbeBatchScope;
    use infiltrator_contract::outbound::{
        details::OutboundSnapshot,
        identity::{OutboundId, SnapshotRevision},
        inventory::{
            OutboundInventorySnapshot, OutboundSelectionSet, OutboundSelectionState,
            SelectionAvailability,
        },
    };
    use infiltrator_contract::speedtest::{
        SpeedtestOutboundResult, SpeedtestProgress, SpeedtestResultState, SpeedtestRounds,
        SpeedtestRunId, SpeedtestRunState, SpeedtestSampleOutcome, SpeedtestTarget,
    };
    use infiltrator_contract::surface_snapshot::PageData;

    fn observed_surface() -> SurfaceSnapshot {
        let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
        let scope = snapshot
            .core
            .scope()
            .expect("demo surface has an accepted core scope");
        let engine = scope.identity.engine.clone();
        let id = |name: &str| OutboundId::new(engine.clone(), name).expect("valid ID");
        let names = ["HK-Fast", "US-Lossy", "JP-Unknown"];
        let entries = names
            .iter()
            .map(|name| OutboundSnapshot {
                id: id(name),
                display_name: Some((*name).to_owned()),
            })
            .collect();
        let inventory = OutboundInventorySnapshot::new(
            scope.clone(),
            SnapshotRevision::new(5),
            OutboundInventoryState::Ready {
                entries,
                collections: vec![OutboundSelectionSet {
                    id: id("GLOBAL"),
                    member_ids: names.iter().map(|name| id(name)).collect(),
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Unknown,
                }],
            },
        )
        .expect("valid outbound fixture");
        snapshot.outbound_inventory = PageData::ready(inventory);
        snapshot.speedtest.state = SpeedtestRunState::Completed {
            run_id: SpeedtestRunId::new(1).expect("valid fixture run ID"),
            target: SpeedtestTarget::new(
                scope,
                SnapshotRevision::new(5),
                OutboundProbeBatchScope::All,
                SpeedtestRounds::DEFAULT,
            )
            .expect("valid speedtest target"),
            progress: SpeedtestProgress {
                completed_targets: 2,
                total_targets: 2,
            },
        };
        snapshot.speedtest.results = vec![
            SpeedtestOutboundResult {
                outbound_id: id("HK-Fast"),
                display_name: Some("HK-Fast".to_owned()),
                protocol_display_name: Some("test".to_owned()),
                samples: vec![SpeedtestSampleOutcome::Measured { delay_ms: 40 }],
                state: SpeedtestResultState::Completed,
                tested_at_epoch_ms: 1,
            },
            SpeedtestOutboundResult {
                outbound_id: id("US-Lossy"),
                display_name: Some("US-Lossy".to_owned()),
                protocol_display_name: Some("test".to_owned()),
                samples: vec![
                    SpeedtestSampleOutcome::Measured { delay_ms: 120 },
                    SpeedtestSampleOutcome::TimedOut,
                    SpeedtestSampleOutcome::TimedOut,
                ],
                state: SpeedtestResultState::Completed,
                tested_at_epoch_ms: 1,
            },
        ];
        snapshot
    }

    #[test]
    fn missing_observations_stay_typed_unknown() {
        let snapshot = observed_surface();
        let (candidates, groups) = candidates_from_surface(&snapshot);
        assert_eq!(candidates.len(), 3);
        assert_eq!(
            groups.get("JP-Unknown").map(|(group, _)| group.value()),
            Some("GLOBAL")
        );
        assert!(snapshot.outbound_inventory.data.is_some());

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
