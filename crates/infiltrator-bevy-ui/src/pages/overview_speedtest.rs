//! The Overview page's speedtest sub-feature: the one-click button,
//! concurrency controls, the per-node detail modal and the
//! in-place restamp of every measured value from the Core-owned snapshot.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::overview::{LastOverviewProjection, OverviewLine};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::ThemeTextColor;
use bevy::input::{ButtonInput, keyboard::KeyCode};
use bevy::scene::{Scene, bsn};
use bevy::text::{LineBreak, TextLayout};
use bevy::ui::prelude::{
    AlignItems, AlignSelf, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::speedtest_detail_projection::{listing, project_details};
use infiltrator_application::speedtest_summary_projection::{
    dead_archive, history_caption, metrics_label,
};
use infiltrator_bevy_widgets::adaptive_modal::ModalState;
use infiltrator_bevy_widgets::adaptive_modal::{OpenModal, adaptive_modal_scene};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::stat_chip::StatChipValue;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::outbound::inventory::OutboundInventoryState;
use infiltrator_contract::outbound::probe::OutboundProbeBatchScope;
use infiltrator_contract::speedtest::{
    SpeedtestConcurrency, SpeedtestPhase, SpeedtestRounds, SpeedtestTarget,
};
use infiltrator_shared::locales::{Lang, Localizer};

/// Marker on the speedtest detail's scroll area.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SpeedtestDetailScrollArea;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpeedtestButtonPhase {
    #[default]
    Ready,
    Running,
    CancelRequested,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestButton {
    pub phase: SpeedtestButtonPhase,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestText;

/// Marker for the fastest measured outbound's latency and jitter/loss metrics.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestMetricsText;

/// Marker for the timed-out / unreachable node archive caption.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestDeadText;

/// Marker for the persisted speedtest run-history caption, restamped from the
/// Core-owned snapshot's `recent_history`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestHistoryText;

/// Caption showing the bounded concurrency value from the shared snapshot.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestConcurrencyText;

/// A signed step applied to the shared concurrency bound.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestConcurrencyStep(pub i64);

/// Button that opens the typed outbound probe detail modal.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestDetailButton;

/// Modal body caption restamped from the shared typed result snapshot.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewSpeedtestDetailBodyText;

pub(crate) fn speedtest_button_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                max_width: percent(100),
                min_width: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S8),
                align_items: AlignItems::Stretch,
                flex_shrink: 1.0,
            }
            Children [
                Node {
                    align_self: AlignSelf::End,
                    min_height: rem(palette.control_height_px),
                    padding: UiRect::horizontal(rem(space::S12)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: rem(space::S6),
                    flex_shrink: 0.0,
                    border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                }
                ThemeBackgroundColor({ Token::AccentContainer })
                OverviewSpeedtestButton { phase: SpeedtestButtonPhase::Ready }
                Button
                Children [
                    @{ icon_scene(IconId::Zap, 14.0, palette.accent) }
                    --
                    LocalizedText::plain("speedtest_start_action") OverviewSpeedtestText TextRole(Role::BodyStrong) ThemeTextColor({ Token::Accent })
                ]
                --
                Node {
                    width: percent(100),
                    min_width: px(0.0),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: rem(space::S6),
                    row_gap: rem(space::S6),
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S6),
                        flex_shrink: 0.0,
                    }
                    Children [
                    LocalizedText::plain("speedtest_concurrency_label") TextRole(Role::Caption)
                    --
                    Node {
                        min_width: rem(26.0),
                        min_height: rem(24.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        flex_shrink: 0.0,
                    }
                    ThemeBackgroundColor({ Token::Border })
                    Button
                    OverviewSpeedtestConcurrencyStep(-5)
                    Children [
                        Text({ "-".to_owned() }) TextRole(Role::Body)
                    ]
                    --
                    Text({ "30".to_owned() })
                    OverviewSpeedtestConcurrencyText
                    TextRole(Role::Caption)
                    --
                    Node {
                        min_width: rem(26.0),
                        min_height: rem(24.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        flex_shrink: 0.0,
                    }
                    ThemeBackgroundColor({ Token::Border })
                    Button
                    OverviewSpeedtestConcurrencyStep(5)
                    Children [
                        Text({ "+".to_owned() }) TextRole(Role::Body)
                    ]
                    ]
                ]
                --
                Text({ "—".to_owned() })
                OverviewSpeedtestMetricsText
                Node { width: percent(100), min_width: px(0.0), max_width: percent(100) }
                TextLayout { linebreak: LineBreak::WordOrCharacter, ..TextLayout::default() }
                TextRole(Role::Caption)
                --
                Text({ "—".to_owned() })
                OverviewSpeedtestDeadText
                Node { width: percent(100), min_width: px(0.0), max_width: percent(100) }
                TextLayout { linebreak: LineBreak::WordOrCharacter, ..TextLayout::default() }
                TextRole(Role::Caption)
                --
                Text({ "—".to_owned() })
                OverviewSpeedtestHistoryText
                Node { width: percent(100), min_width: px(0.0), max_width: percent(100) }
                TextLayout { linebreak: LineBreak::WordOrCharacter, ..TextLayout::default() }
                TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    min_width: px(0.0),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: rem(space::S6),
                    row_gap: rem(space::S6),
                }
                Children [
                    Node {
                        min_height: rem(24.0),
                        padding: UiRect::horizontal(rem(space::S8)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        flex_shrink: 0.0,
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                    }
                    ThemeBackgroundColor({ Token::Border })
                    Button
                    OverviewSpeedtestDetailButton
                    Children [
                        LocalizedText::plain("common_details") TextRole(Role::Caption)
                    ]
                ]
            ]
    }
}

/// The Overview outbound-probe detail modal scene. The body caption is
/// restamped from the shared snapshot by `sync_overview_speedtest_detail`; the
/// modal widget layer owns open/close visibility and responsive morphology.
pub fn overview_speedtest_detail_modal_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let body = Box::new(bsn! {
            Node {
                width: percent(100),
                max_height: Val::Vh(62.0),
                min_height: px(0.0), flex_shrink: 1.0,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
            }
            ScrollArea SpeedtestDetailScrollArea
            Children [
                Text({ "—".to_owned() })
                OverviewSpeedtestDetailBodyText
                TextRole(Role::Caption)
            ]
    });
    let language = UiLocale::default().code().to_string();
    adaptive_modal_scene(
        Lang(&language).tr("speedtest_detail_title").into_owned(),
        Lang(&language).tr("modal_close").into_owned(),
        body,
        Vec::<Box<dyn Scene>>::new(),
        palette,
    )
}

pub(crate) fn on_overview_speedtest_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewSpeedtestButton>,
    latest: Option<Res<LatestSurfaceSnapshot>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    match button.phase {
        SpeedtestButtonPhase::Running => {
            let Some(latest) = latest.as_deref() else {
                return;
            };
            let Some(run_id) = latest.0.speedtest.state.run_id() else {
                return;
            };
            handle.submit(UiCommand::CancelSpeedtestRun { run_id });
            return;
        }
        SpeedtestButtonPhase::CancelRequested => return,
        SpeedtestButtonPhase::Ready => {}
    }
    let Some(latest) = latest.as_deref() else {
        return;
    };
    let Some(target) = speedtest_target(&latest.0) else {
        return;
    };
    handle.submit(UiCommand::StartSpeedtest { target });
}

fn speedtest_target(
    snapshot: &infiltrator_contract::surface_snapshot::SurfaceSnapshot,
) -> Option<SpeedtestTarget> {
    let scope = snapshot.core.scope()?;
    let inventory = snapshot.outbound_inventory.data.as_ref()?;
    if inventory.scope() != &scope {
        return None;
    }
    match inventory.inventory() {
        OutboundInventoryState::Ready { .. } => SpeedtestTarget::new(
            scope,
            inventory.revision(),
            OutboundProbeBatchScope::All,
            SpeedtestRounds::DEFAULT,
        )
        .ok(),
        OutboundInventoryState::Unknown
        | OutboundInventoryState::Empty
        | OutboundInventoryState::Unsupported
        | OutboundInventoryState::Failed { .. } => None,
    }
}

/// Apply a signed step to the live concurrency bound read from the shared
/// speedtest snapshot and submit the typed shared intent.
pub(crate) fn on_overview_speedtest_concurrency_stepped(
    activate: On<Activate>,
    steps: Query<&OverviewSpeedtestConcurrencyStep>,
    latest: Res<LatestSurfaceSnapshot>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(step) = steps.get(activate.entity) else {
        return;
    };
    let current = i64::from(latest.0.speedtest.concurrency.get());
    let next = (current + step.0).clamp(1, 30) as u8;
    let Ok(concurrency) = SpeedtestConcurrency::new(next) else {
        return;
    };
    handle.submit(UiCommand::SetSpeedtestConcurrency { concurrency });
}

/// Open the detail modal from the shared snapshot. Pure view state
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

/// Disjoint filter for the detail-modal body caption.
type SpeedtestDetailBodyFilter = (
    With<OverviewSpeedtestDetailBodyText>,
    Without<OverviewSpeedtestText>,
    Without<OverviewSpeedtestMetricsText>,
    Without<OverviewSpeedtestDeadText>,
    Without<OverviewSpeedtestHistoryText>,
    Without<OverviewSpeedtestConcurrencyText>,
);

/// Restamp the typed per-outbound details from the one shared snapshot.
pub fn sync_overview_speedtest_detail(
    last: Res<LastOverviewProjection>,
    mut detail: Query<&mut Text, SpeedtestDetailBodyFilter>,
    locale: Res<UiLocale>,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let snapshot = &projection.speedtest;

    let body_label = listing(&project_details(snapshot), &|key| {
        Lang(locale.code()).tr(key).into_owned()
    });
    for mut text in &mut detail {
        if text.0 != body_label {
            text.0 = body_label.clone();
        }
    }
}

/// Restamp the Overview speedtest button from the Core-owned snapshot.
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

/// The button is baked in its ready phase at mount; this system reflects the
/// typed live phase and progress from the shared read model.
#[derive(SystemParam)]
pub struct SpeedtestControls<'w, 's> {
    buttons: Query<'w, 's, &'static mut OverviewSpeedtestButton>,
    texts: Query<'w, 's, (&'static mut Text, &'static mut LocalizedText), SpeedtestTextFilter>,
    locale: Res<'w, UiLocale>,
}

pub fn sync_overview_speedtest_button(
    last: Res<LastOverviewProjection>,
    controls: SpeedtestControls,
    mut metrics: Query<&mut Text, SpeedtestMetricsFilter>,
    mut dead: Query<&mut Text, SpeedtestDeadFilter>,
    mut history: Query<&mut Text, SpeedtestHistoryFilter>,
    mut concurrency: Query<&mut Text, SpeedtestConcurrencyFilter>,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let SpeedtestControls {
        mut buttons,
        mut texts,
        locale,
    } = controls;
    let snapshot = &projection.speedtest;
    let running = snapshot.is_running();
    let button_phase = match &snapshot.state.phase() {
        SpeedtestPhase::Probing => SpeedtestButtonPhase::Running,
        SpeedtestPhase::CancelRequested => SpeedtestButtonPhase::CancelRequested,
        SpeedtestPhase::Idle
        | SpeedtestPhase::Completed
        | SpeedtestPhase::Cancelled
        | SpeedtestPhase::Failed { .. }
        | SpeedtestPhase::Unavailable { .. } => SpeedtestButtonPhase::Ready,
    };
    let copy = if running {
        let progress = snapshot.state.progress();
        let done = progress.map_or(0, |progress| progress.completed_targets);
        let total = progress.map_or(0, |progress| progress.total_targets);
        if total > 0 {
            LocalizedText::new(
                "speedtest_cancel_progress",
                vec![("done", done.to_string()), ("total", total.to_string())],
            )
        } else {
            LocalizedText::plain("speedtest_cancel_running")
        }
    } else {
        LocalizedText::plain("speedtest_start_action")
    };
    let label = copy.render(&locale);
    let metrics_label = metrics_label(snapshot.fastest(), locale.code());
    for mut button in &mut buttons {
        if button.phase != button_phase {
            button.phase = button_phase;
        }
    }
    for (mut text, mut current) in &mut texts {
        if *current != copy {
            *current = copy.clone();
        }
        if text.0 != label {
            text.0 = label.clone();
        }
    }
    for mut text in &mut metrics {
        if text.0 != metrics_label {
            text.0 = metrics_label.clone();
        }
    }
    let dead_label = dead_archive(snapshot, locale.code()).caption;
    for mut text in &mut dead {
        if text.0 != dead_label {
            text.0 = dead_label.clone();
        }
    }
    let history_label = history_caption(snapshot, locale.code());
    for mut text in &mut history {
        if text.0 != history_label {
            text.0 = history_label.clone();
        }
    }
    // The live concurrency bound comes from the shared snapshot.
    let concurrency_label = snapshot.concurrency.get().to_string();
    for mut text in &mut concurrency {
        if text.0 != concurrency_label {
            text.0 = concurrency_label.clone();
        }
    }
}

/// A detail inspection never stays open over a different page or after Escape.
pub fn dismiss_speedtest_detail(
    route: Res<ActiveRoute>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<ModalState>,
) {
    if state.is_open
        && (route.0 != Some(Route::Overview)
            || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
    {
        state.close();
    }
}
