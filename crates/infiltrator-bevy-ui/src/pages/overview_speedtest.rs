//! The Overview page's speedtest sub-feature: the one-click button, the
//! target-URL and concurrency controls, the per-node detail modal and the
//! in-place restamp of every measured value from the shared engine snapshot.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::adaptive_modal::{OpenModal, adaptive_modal_scene};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::stat_chip::StatChipValue;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::speedtest::{
    EgressCountryMatch, HistoricalSpeedtestRecord, NodeSpeedtestResult, SpeedtestScope,
    SpeedtestSnapshot,
};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::overview::{LastOverviewProjection, OverviewLine};

/// Marker naming which proxy mode a mode pill stands for; the refresh
/// observer restamps its `ControlVisual` selected bit (the widget layer's
/// shared repaint system re-derives the token fill from it). Mounted by
/// the shell's sidebar segment control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestButton {
    pub testing: bool,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestText;

/// Marker for the live speedtest metrics caption (jitter / loss / stars /
/// bandwidth of the fastest measured node), restamped from the shared engine.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestMetricsText;

/// Marker for the timed-out / unreachable node archive caption.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestDeadText;

/// Marker for the persisted speedtest run-history caption, restamped from the
/// shared engine snapshot's `recent_history`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestHistoryText;

/// DUAL-06-03: parent node of the Overview speedtest target-URL text field.
/// The typed value is read into `UiCommand::TestAllProxyGroupsWithUrl`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestUrlField;

/// DUAL-06-01: caption showing the live concurrency bound from the shared
/// speedtest snapshot (`snapshot.config.concurrency`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestConcurrencyText;

/// DUAL-06-01: a signed step applied to the shared concurrency bound. The
/// observer reads the live bound and submits `SetSpeedtestConcurrency`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestConcurrencyStep(pub i64);

/// DUAL-06-12: caption showing the fastest node's reported egress endpoint and
/// the honest label-vs-egress country comparison.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestEgressText;

/// DUAL-06-13: button that opens the per-node speedtest detail modal.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestDetailButton;

/// DUAL-06-13: the modal body caption restamped with every measured node's
/// metrics from the shared snapshot (honest empty / failed states).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestDetailBodyText;

pub(crate) fn speedtest_button_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
            align_items: AlignItems::End,
            flex_shrink: 0.0,
        }
        Children [
            (
                Node {
                    min_height: px(palette.control_height_px),
                    padding: UiRect::horizontal(Val::Px(space::S12)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(space::S6),
                    flex_shrink: 0.0,
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.accent_container })
                OverviewSpeedtestButton { testing: false }
                Button
                Children [
                    ( { icon_scene(IconId::Zap, 14.0, palette.accent) } ),
                    ( Text({ "一键测速".to_owned() }) OverviewSpeedtestText TextRole(Role::BodyStrong) TextColor({ palette.accent }) ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S6),
                    flex_shrink: 0.0,
                }
                Children [
                    (
                        Node {
                            width: px(240.0),
                            align_items: AlignItems::Center,
                            flex_shrink: 0.0,
                        }
                        OverviewSpeedtestUrlField
                        Children [
                            ( { text_field_with_placeholder_scene(
                                String::new(),
                                "测速目标 URL (留空用默认)".to_owned(),
                                palette,
                            ) } ),
                        ]
                    ),
                    ( Text({ "并发".to_owned() }) TextRole(Role::Caption) ),
                    (
                        Node {
                            min_width: px(26.0),
                            min_height: px(24.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            flex_shrink: 0.0,
                        }
                        BackgroundColor({ palette.border })
                        Button
                        OverviewSpeedtestConcurrencyStep(-5)
                        Children [ ( Text({ "-".to_owned() }) TextRole(Role::Body) ) ]
                    ),
                    (
                        Text({ "30".to_owned() })
                        OverviewSpeedtestConcurrencyText
                        TextRole(Role::Caption)
                    ),
                    (
                        Node {
                            min_width: px(26.0),
                            min_height: px(24.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            flex_shrink: 0.0,
                        }
                        BackgroundColor({ palette.border })
                        Button
                        OverviewSpeedtestConcurrencyStep(5)
                        Children [ ( Text({ "+".to_owned() }) TextRole(Role::Body) ) ]
                    ),
                ]
            ),
            (
                Text({ "—".to_owned() })
                OverviewSpeedtestMetricsText
                TextRole(Role::Caption)
            ),
            (
                Text({ "—".to_owned() })
                OverviewSpeedtestDeadText
                TextRole(Role::Caption)
            ),
            (
                Text({ "—".to_owned() })
                OverviewSpeedtestHistoryText
                TextRole(Role::Caption)
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S6),
                    flex_shrink: 0.0,
                }
                Children [
                    (
                        Node {
                            min_height: px(24.0),
                            padding: UiRect::horizontal(Val::Px(space::S8)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            flex_shrink: 0.0,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.border })
                        Button
                        OverviewSpeedtestDetailButton
                        Children [ ( Text({ "详情".to_owned() }) TextRole(Role::Caption) ) ]
                    ),
                    (
                        Text({ "—".to_owned() })
                        OverviewSpeedtestEgressText
                        TextRole(Role::Caption)
                    ),
                ]
            ),
        ]
    }
}

/// DUAL-06-13: the Overview speedtest detail modal scene. The body caption is
/// restamped from the shared snapshot by `sync_overview_speedtest_detail`; the
/// modal widget layer owns open/close visibility and responsive morphology.
pub fn overview_speedtest_detail_modal_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let body = Box::new(bsn! {
        Node {
            width: percent(100),
            max_height: px(360.0),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::scroll_y(),
        }
        Children [
            (
                Text({ "—".to_owned() })
                OverviewSpeedtestDetailBodyText
                TextRole(Role::Caption)
            ),
        ]
    });
    adaptive_modal_scene(
        "测速结果明细".to_owned(),
        body,
        Vec::<Box<dyn Scene>>::new(),
        palette,
    )
}

pub(crate) fn on_overview_speedtest_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewSpeedtestButton>,
    url_fields: Query<&Children, With<OverviewSpeedtestUrlField>>,
    text_fields: Query<&TextField>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    // The same control toggles: idle starts a batch, in-flight cancels the
    // shared engine run (no dead "testing" state that can never be stopped).
    if button.testing {
        handle.submit(UiCommand::CancelSpeedtest);
        return;
    }
    // DUAL-06-03: read the typed target URL from the card's text field and
    // carry it into the shared intent; blank keeps the engine's own default.
    let url = url_fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text())
        .unwrap_or_default();
    let url = url.trim().to_owned();
    if url.is_empty() {
        handle.submit(UiCommand::TestAllProxyGroups);
    } else {
        handle.submit(UiCommand::TestAllProxyGroupsWithUrl { url });
    }
}

/// DUAL-06-01: apply a signed step to the live concurrency bound read from the
/// shared speedtest snapshot and submit the shared intent. The UI never owns
/// the effective bound.
pub(crate) fn on_overview_speedtest_concurrency_stepped(
    activate: On<Activate>,
    steps: Query<&OverviewSpeedtestConcurrencyStep>,
    latest: Res<crate::surface::LatestSurfaceSnapshot>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(step) = steps.get(activate.entity) else {
        return;
    };
    let current = latest.0.speedtest.config.concurrency as i64;
    let next = (current + step.0).clamp(1, 64) as usize;
    handle.submit(UiCommand::SetSpeedtestConcurrency { limit: next });
}

/// DUAL-06-13: open the detail modal from the shared snapshot. Pure view state
/// in the modal widget layer; the observer never probes or fabricates a node.
pub(crate) fn on_overview_speedtest_detail_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewSpeedtestDetailButton>,
    mut commands: Commands,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    commands.trigger(OpenModal);
}

/// Disjoint filter for the reported egress caption.
type SpeedtestEgressFilter = (
    With<OverviewSpeedtestEgressText>,
    Without<OverviewSpeedtestDetailBodyText>,
    Without<OverviewSpeedtestText>,
    Without<OverviewSpeedtestMetricsText>,
    Without<OverviewSpeedtestDeadText>,
    Without<OverviewSpeedtestHistoryText>,
    Without<OverviewSpeedtestConcurrencyText>,
);

/// Disjoint filter for the detail-modal body caption.
type SpeedtestDetailBodyFilter = (
    With<OverviewSpeedtestDetailBodyText>,
    Without<OverviewSpeedtestEgressText>,
    Without<OverviewSpeedtestText>,
    Without<OverviewSpeedtestMetricsText>,
    Without<OverviewSpeedtestDeadText>,
    Without<OverviewSpeedtestHistoryText>,
    Without<OverviewSpeedtestConcurrencyText>,
);

/// One honest line for a node in the detail modal.
fn overview_detail_line(node: &NodeSpeedtestResult) -> String {
    let delay = node
        .delay_ms
        .map(|ms| format!("{ms}ms"))
        .unwrap_or_else(|| "—".to_owned());
    let jitter = node
        .jitter
        .as_ref()
        .map(|j| format!("{:.1}ms", j.jitter_ms))
        .unwrap_or_else(|| "—".to_owned());
    let loss = node
        .jitter
        .as_ref()
        .map(|j| format!("{:.1}%", j.loss_percent))
        .unwrap_or_else(|| "—".to_owned());
    let bandwidth = node
        .bandwidth_mbps
        .map(|mbps| format!("{mbps:.1}Mbps"))
        .unwrap_or_else(|| "—".to_owned());
    let stars = "★".repeat(node.star_rating.min(5) as usize);
    let match_label = match node.egress_country_match() {
        EgressCountryMatch::Match => "归属一致",
        EgressCountryMatch::Mismatch => "归属不一致",
        EgressCountryMatch::Unlabelled => "无标签国家",
        EgressCountryMatch::Unknown => "出口未探测",
    };
    format!(
        "{} · 延迟 {delay} · 抖动 {jitter} · 丢包 {loss} · 带宽 {bandwidth} · {stars} · 出口 {} · {match_label}",
        node.node_name,
        node.egress_endpoint_label(),
    )
}

/// Honest modal body: empty / failed states are literal, never fabricated.
fn overview_detail_body(snapshot: &SpeedtestSnapshot) -> String {
    if snapshot.node_results.is_empty() {
        return match &snapshot.failure {
            Some(failure) => format!("测速失败: {failure}"),
            None => "暂无测速结果".to_owned(),
        };
    }
    let mut lines: Vec<String> = vec![format!("出口状态: {}", snapshot.egress_summary())];
    for node in snapshot.sorted_by_latency() {
        lines.push(overview_detail_line(node));
    }
    lines.join("\n")
}

/// DUAL-06-12/13: restamp the egress caption and the detail-modal body from the
/// one shared snapshot. Neither surface owns a metric of its own.
pub fn sync_overview_speedtest_detail(
    last: Res<LastOverviewProjection>,
    mut egress: Query<&mut Text, SpeedtestEgressFilter>,
    mut detail: Query<&mut Text, SpeedtestDetailBodyFilter>,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let snapshot = &projection.speedtest;

    let egress_label = match snapshot.fastest_node() {
        Some(node) => format!(
            "出口 {} · {}",
            node.egress_endpoint_label(),
            match node.egress_country_match() {
                EgressCountryMatch::Match => "归属一致",
                EgressCountryMatch::Mismatch => "归属不一致",
                EgressCountryMatch::Unlabelled => "无标签国家",
                EgressCountryMatch::Unknown => "出口未探测",
            }
        ),
        None => "—".to_owned(),
    };
    for mut text in &mut egress {
        if text.0 != egress_label {
            text.0 = egress_label.clone();
        }
    }

    let body_label = overview_detail_body(snapshot);
    for mut text in &mut detail {
        if text.0 != body_label {
            text.0 = body_label.clone();
        }
    }
}

/// Restamp the Overview speedtest button from the shared engine snapshot.
///
/// Query filter for the speedtest caption text, kept disjoint from the marked
/// Overview line texts and stat chip values.
type SpeedtestTextFilter = (
    With<OverviewSpeedtestText>,
    Without<OverviewLine>,
    Without<StatChipValue>,
    Without<OverviewSpeedtestMetricsText>,
    Without<OverviewSpeedtestDeadText>,
    Without<OverviewSpeedtestHistoryText>,
);

/// Disjoint filter for the live speedtest metrics caption.
type SpeedtestMetricsFilter = (
    With<OverviewSpeedtestMetricsText>,
    Without<OverviewLine>,
    Without<StatChipValue>,
    Without<OverviewSpeedtestText>,
    Without<OverviewSpeedtestDeadText>,
    Without<OverviewSpeedtestHistoryText>,
);

/// Disjoint filter for the timed-out / unreachable node archive caption.
type SpeedtestDeadFilter = (
    With<OverviewSpeedtestDeadText>,
    Without<OverviewLine>,
    Without<StatChipValue>,
    Without<OverviewSpeedtestText>,
    Without<OverviewSpeedtestMetricsText>,
    Without<OverviewSpeedtestHistoryText>,
);

/// Disjoint filter for the persisted run-history caption.
type SpeedtestHistoryFilter = (
    With<OverviewSpeedtestHistoryText>,
    Without<OverviewLine>,
    Without<StatChipValue>,
    Without<OverviewSpeedtestText>,
    Without<OverviewSpeedtestMetricsText>,
    Without<OverviewSpeedtestDeadText>,
);

/// Disjoint filter for the live concurrency-bound caption.
type SpeedtestConcurrencyFilter = (
    With<OverviewSpeedtestConcurrencyText>,
    Without<OverviewLine>,
    Without<StatChipValue>,
    Without<OverviewSpeedtestText>,
    Without<OverviewSpeedtestMetricsText>,
    Without<OverviewSpeedtestDeadText>,
    Without<OverviewSpeedtestHistoryText>,
);

/// Format an epoch-millisecond timestamp as UTC `MM-DD HH:MM` (no date crate).
fn overview_run_time(epoch_ms: u64) -> String {
    let secs = (epoch_ms / 1000) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{month:02}-{day:02} {hour:02}:{minute:02}")
}

fn overview_scope_label(scope: &SpeedtestScope) -> String {
    match scope {
        SpeedtestScope::AllGroups => "全部节点".to_owned(),
        SpeedtestScope::SingleGroup(group) => format!("分组 {group}"),
        SpeedtestScope::SingleNode(node) => format!("节点 {node}"),
    }
}

/// One honest, compact line per persisted run (shared `recent_history`).
fn overview_history_line(record: &HistoricalSpeedtestRecord) -> String {
    let bandwidth = record
        .avg_bandwidth_mbps
        .map(|mbps| format!("{mbps:.1}Mbps"))
        .unwrap_or_else(|| "—".to_owned());
    let latency = record
        .avg_latency_ms
        .map(|ms| format!("{ms:.1}ms"))
        .unwrap_or_else(|| "—".to_owned());
    let jitter = record
        .avg_jitter_ms
        .map(|ms| format!("{ms:.1}ms"))
        .unwrap_or_else(|| "—".to_owned());
    format!(
        "{} · {} · 存活 {}/{} · 平均延迟 {latency} · 平均抖动 {jitter} · 平均带宽 {bandwidth} · ★{}",
        overview_run_time(record.timestamp_epoch_ms),
        overview_scope_label(&record.scope),
        record.alive_nodes,
        record.total_nodes,
        record.overall_star_rating.min(5),
    )
}

/// The button is baked `testing: false` at mount; this system reflects the
/// live phase and progress so both surfaces read the same read model instead
/// of a static label. Runs on every projection update and each frame the
/// projection resource changes.
pub fn sync_overview_speedtest_button(
    last: Res<LastOverviewProjection>,
    mut buttons: Query<&mut OverviewSpeedtestButton>,
    mut texts: Query<&mut Text, SpeedtestTextFilter>,
    mut metrics: Query<&mut Text, SpeedtestMetricsFilter>,
    mut dead: Query<&mut Text, SpeedtestDeadFilter>,
    mut history: Query<&mut Text, SpeedtestHistoryFilter>,
    mut concurrency: Query<&mut Text, SpeedtestConcurrencyFilter>,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let snapshot = &projection.speedtest;
    let running = snapshot.is_running();
    let label = if running {
        let done = snapshot.progress.completed_nodes;
        let total = snapshot.progress.total_nodes;
        if total > 0 {
            format!("测速中 {done}/{total}")
        } else {
            "测速中…".to_owned()
        }
    } else {
        "一键测速".to_owned()
    };
    // Mirror the Iced speedtest card metrics from the same shared snapshot:
    // jitter, packet-loss rating, star rating and bandwidth of the fastest
    // measured node. Honest "—" until the engine has a result.
    let metrics_label = match snapshot.fastest_node() {
        Some(node) => {
            let jitter = node
                .jitter
                .as_ref()
                .map(|j| format!("{:.1}ms", j.jitter_ms))
                .unwrap_or_else(|| "—".to_owned());
            let bandwidth = node
                .bandwidth_mbps
                .map(|mbps| format!("{mbps:.1}Mbps"))
                .unwrap_or_else(|| "—".to_owned());
            let stars = "★".repeat(node.star_rating.min(5) as usize);
            format!(
                "{} · 抖动 {jitter} · 丢包 {} · {bandwidth} · {stars}",
                node.node_name,
                node.packet_loss.label_en()
            )
        }
        None => "—".to_owned(),
    };
    for mut button in &mut buttons {
        if button.testing != running {
            button.testing = running;
        }
    }
    for mut text in &mut texts {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
    for mut text in &mut metrics {
        if text.0 != metrics_label {
            text.0 = metrics_label.clone();
        }
    }
    // Timed-out / unreachable nodes are archived in one honest line; empty
    // means "—", never a fabricated node.
    let dead_label = {
        let dead_nodes = snapshot.dead_nodes();
        if dead_nodes.is_empty() {
            "—".to_owned()
        } else {
            let names: Vec<&str> = dead_nodes
                .iter()
                .take(4)
                .map(|node| node.node_name.as_str())
                .collect();
            let extra = dead_nodes.len().saturating_sub(names.len());
            let mut listed = names.join(" · ");
            if extra > 0 {
                listed.push_str(&format!(" (+{extra})"));
            }
            format!("超时归档 {} · {listed}", dead_nodes.len())
        }
    };
    for mut text in &mut dead {
        if text.0 != dead_label {
            text.0 = dead_label.clone();
        }
    }
    // Persisted run history from the same shared snapshot: one honest line per
    // run, newest first, with "—" when nothing has been recorded yet.
    let history_label = {
        let lines: Vec<String> = snapshot
            .recent_history
            .iter()
            .rev()
            .map(overview_history_line)
            .collect();
        if lines.is_empty() {
            "—".to_owned()
        } else {
            format!("最近测速: {}", lines.join(" | "))
        }
    };
    for mut text in &mut history {
        if text.0 != history_label {
            text.0 = history_label.clone();
        }
    }
    // DUAL-06-01: the live concurrency bound is read from the shared snapshot,
    // never a Bevy-local constant.
    let concurrency_label = snapshot.config.concurrency.to_string();
    for mut text in &mut concurrency {
        if text.0 != concurrency_label {
            text.0 = concurrency_label.clone();
        }
    }
}
