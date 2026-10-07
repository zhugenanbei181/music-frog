//! The Overview page's speedtest sub-feature: the one-click button, the
//! target-URL and concurrency controls, the per-node detail modal and the
//! in-place restamp of every measured value from the shared engine snapshot.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_field_scene;
use crate::pages::overview::{LastOverviewProjection, OverviewLine};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::ResMut;
use bevy::ecs::system::{Commands, Query, Res, SystemParam};
use bevy::input::{ButtonInput, keyboard::KeyCode};
use bevy::scene::{Scene, bsn};
use bevy::text::{LineBreak, TextColor, TextLayout};
use bevy::ui::prelude::{
    AlignItems, AlignSelf, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent,
    Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::speedtest_detail_projection::{listing, project_details};
use infiltrator_application::speedtest_summary_projection::{
    dead_archive, egress_label, history_caption, metrics_label,
};
use infiltrator_bevy_widgets::adaptive_modal::ModalState;
use infiltrator_bevy_widgets::adaptive_modal::{OpenModal, adaptive_modal_scene};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::stat_chip::StatChipValue;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_shared::locales::{Lang, Localizer, get_system_language};
use std::env;

/// Marker naming which proxy mode a mode pill stands for; the refresh
/// observer restamps its `ControlVisual` selected bit (the widget layer's
/// shared repaint system re-derives the token fill from it). Mounted by
/// the shell's sidebar segment control.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SpeedtestDetailScrollArea;

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
                width: percent(100),
                max_width: percent(100),
                min_width: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
                align_items: AlignItems::Stretch,
                flex_shrink: 1.0,
            }
            Children [
                Node {
                    align_self: AlignSelf::End,
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
                    @{ icon_scene(IconId::Zap, 14.0, palette.accent) }
                    --
                    LocalizedText::plain("speedtest_start_action") OverviewSpeedtestText TextRole(Role::BodyStrong) TextColor({ palette.accent })
                ]
                --
                Node {
                    width: percent(100),
                    min_width: px(0.0),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(space::S6),
                    row_gap: Val::Px(space::S6),
                }
                Children [
                    Node {
                        flex_basis: px(240.0),
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        min_width: px(0.0),
                        max_width: percent(100),
                        align_items: AlignItems::Center,
                    }
                    OverviewSpeedtestUrlField
                    Children [
                        @{ localized_field_scene(String::new(), LocalizedText::plain("field_speedtest_url"),
                                palette,
                        ) } NativeTextField(1)
                    ]
                    --
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S6),
                        flex_shrink: 0.0,
                    }
                    Children [
                    LocalizedText::plain("speedtest_concurrency_label") TextRole(Role::Caption)
                    --
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
                    Children [
                        Text({ "-".to_owned() }) TextRole(Role::Body)
                    ]
                    --
                    Text({ "30".to_owned() })
                    OverviewSpeedtestConcurrencyText
                    TextRole(Role::Caption)
                    --
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
                    column_gap: Val::Px(space::S6),
                    row_gap: Val::Px(space::S6),
                }
                Children [
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
                    Children [
                        LocalizedText::plain("common_details") TextRole(Role::Caption)
                    ]
                    --
                    Text({ "—".to_owned() })
                    OverviewSpeedtestEgressText
                Node { width: percent(100), min_width: px(0.0), max_width: percent(100) }
                TextLayout { linebreak: LineBreak::WordOrCharacter, ..TextLayout::default() }
                    TextRole(Role::Caption)
                ]
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
    let language = env::var("INFILTRATOR_LANG").unwrap_or_else(|_| get_system_language());
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
    latest: Res<LatestSurfaceSnapshot>,
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

/// DUAL-06-12/13: restamp the egress caption and the detail-modal body from the
/// one shared snapshot. Neither surface owns a metric of its own.
pub fn sync_overview_speedtest_detail(
    last: Res<LastOverviewProjection>,
    mut egress: Query<&mut Text, SpeedtestEgressFilter>,
    mut detail: Query<&mut Text, SpeedtestDetailBodyFilter>,
    locale: Res<UiLocale>,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let snapshot = &projection.speedtest;

    let egress_label = egress_label(snapshot.fastest_node(), locale.code());
    for mut text in &mut egress {
        if text.0 != egress_label {
            text.0 = egress_label.clone();
        }
    }

    let body_label = listing(&project_details(snapshot), &|key| {
        Lang(locale.code()).tr(key).into_owned()
    });
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

/// The button is baked `testing: false` at mount; this system reflects the
/// live phase and progress so both surfaces read the same read model instead
/// of a static label. Runs on every projection update and each frame the
/// projection resource changes.
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
    let copy = if running {
        let done = snapshot.progress.completed_nodes;
        let total = snapshot.progress.total_nodes;
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
    let metrics_label = metrics_label(snapshot.fastest_node(), locale.code());
    for mut button in &mut buttons {
        if button.testing != running {
            button.testing = running;
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
    // DUAL-06-01: the live concurrency bound is read from the shared snapshot,
    // never a Bevy-local constant.
    let concurrency_label = snapshot.config.concurrency.to_string();
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
