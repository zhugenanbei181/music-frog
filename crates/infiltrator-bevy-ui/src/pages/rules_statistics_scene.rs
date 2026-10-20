//! Declarative inspector and a separate cleanup review overlay.
use crate::localized_widgets::localized_button_scene;
use crate::pages::rules_statistics::{
    StatisticsCard, StatisticsConfirmation, StatisticsConfirmationCard, StatisticsControl,
    StatisticsLine, StatisticsRows,
};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::Without;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::scene::{CommandsSceneExt, Scene, bsn, on};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, Display, FlexDirection, FlexWrap, GlobalZIndex, JustifyContent, Node, Overflow,
    PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::{Activate, ModalDialog, ModalDialogBarrier, RequestClose, ScrollArea};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_presentation::rule_statistics_workbench::StatisticsTab;

/// The metric column's minimum width in baseline pixels.
const STATISTICS_METRIC_MIN_WIDTH_PX: f32 = 120.0;

/// The statistics card's fixed height in baseline pixels.
const STATISTICS_CARD_HEIGHT_PX: f32 = 320.0;

/// The cleanup confirmation dialog's maximum width in baseline pixels.
const STATISTICS_CLEANUP_DIALOG_MAX_WIDTH_PX: f32 = 560.0;

fn control(
    key: &'static str,
    action: StatisticsControl,
    palette: &UiPalette,
) -> impl Scene + use<> {
    (
        localized_button_scene(LocalizedText::plain(key), ButtonVariant::Default, palette),
        bsn! { StatisticsControl { .. { action } } ButtonDisabled(true) },
    )
}
/// The cleanup review's dismiss control: it asks the official dialog layer to
/// close instead of closing the review itself.
fn dismiss_control(
    key: &'static str,
    action: StatisticsControl,
    palette: &UiPalette,
) -> impl Scene + use<> {
    (
        localized_button_scene(LocalizedText::plain(key), ButtonVariant::Default, palette),
        bsn! {
            StatisticsControl { .. { action } }
            ButtonDisabled(true)
            TabIndex(0)
            // The dismiss control asks the official layer to close; the page
            // never closes the cleanup review on its own.
            on(|activate: On<Activate>, mut commands: Commands| {
                commands.trigger(RequestClose { source: activate.entity });
            })
        },
    )
}
fn metric(key: &'static str, line: StatisticsLine) -> impl Scene + use<> {
    bsn! {
        Node { min_width: rem(STATISTICS_METRIC_MIN_WIDTH_PX), flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: rem(space::S4) }
        Children [ LocalizedText::plain(key) TextRole(Role::Caption) -- Text(String::new()) StatisticsLine { .. { line } } TextRole(Role::BodyStrong) ]
    }
}
pub fn scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { width: percent(100), height: rem(STATISTICS_CARD_HEIGHT_PX), min_height: rem(STATISTICS_CARD_HEIGHT_PX), flex_shrink: 0.0,
            padding: UiRect::all(rem(space::S12)), flex_direction: FlexDirection::Column, row_gap: rem(space::S8) }
        StatisticsCard ThemeBackgroundColor({Token::SurfaceElevated})
        Children [
            LocalizedText::plain("rule_hit_title") TextRole(Role::BodyStrong)
            --
            Text(String::new()) StatisticsLine::Status TextRole(Role::Caption)
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S8), row_gap: rem(space::S4), flex_shrink: 0.0 }
            Children [
                @{control("rules_stats_summary_tab", StatisticsControl::Tab(StatisticsTab::Summary), palette)}
                --
                @{control("rules_stats_top_tab", StatisticsControl::Tab(StatisticsTab::TopHits), palette)}
                --
                @{control("rules_stats_inactive_tab", StatisticsControl::Tab(StatisticsTab::Inactive), palette)}
            ]
            --
            Node { width: percent(100), min_height: px(0.0), flex_grow: 1.0, flex_shrink: 1.0,
                flex_direction: FlexDirection::Column, row_gap: rem(space::S8), overflow: Overflow::scroll_y() }
            ScrollArea
            Children [
                LocalizedText::plain("rule_hit_desc") TextRole(Role::Caption)
                --
                Text(String::new()) StatisticsLine::Source TextRole(Role::Caption)
                --
                Node { width: percent(100), flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S8), row_gap: rem(space::S8) }
                Children [ @{metric("rule_hit_total_hits", StatisticsLine::Total)} -- @{metric("rule_hit_dead_count", StatisticsLine::Dead)} -- @{metric("rule_hit_cidr_conflicts", StatisticsLine::Cidr)} -- @{metric("rule_hit_match_latency", StatisticsLine::Latency)} ]
                --
                Text(String::new()) StatisticsLine::Last TextRole(Role::Body)
                --
                Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: rem(space::S8) }
                StatisticsRows
                --
                Text(String::new()) StatisticsLine::Empty TextRole(Role::Caption)
            ]
            --
            Text(String::new()) StatisticsLine::Page TextRole(Role::Caption)
            --
            Text(String::new()) StatisticsLine::Feedback TextRole(Role::Caption)
            --
            Node { width: percent(100), flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S8), row_gap: rem(space::S4), flex_shrink: 0.0 }
            Children [
                @{control("common_previous_page", StatisticsControl::Previous, palette)}
                --
                @{control("common_next_page", StatisticsControl::Next, palette)}
                --
                @{control("rule_hit_btn_audit", StatisticsControl::Inspect, palette)}
                --
                @{control("rule_hit_btn_clean", StatisticsControl::PrepareCleanup, palette)}
                --
                @{control("rule_hit_btn_clear", StatisticsControl::Reset, palette)}
                --
                @{control("modal_close", StatisticsControl::DismissFailure, palette)}
            ]
        ]
    }
}
pub fn spawn_confirmation(mut commands: Commands, palette: Res<UiPalette>) {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    commands.spawn_scene(bsn! {
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), display: Display::None,
            align_items: AlignItems::Center, justify_content: JustifyContent::Center }
        StatisticsConfirmation GlobalZIndex(121)
        // The barrier is the modal layer's root: it owns the click-outside
        // close and the Escape interception, and the card is its child, so the
        // official behaviour layer reaches the focused control.
        ModalScrim ThemeBackgroundColor({Token::Scrim}) ModalDialogBarrier
        // The close request travels as the official `RequestClose` and the
        // page's own close path runs from it.
        on(|close: On<RequestClose>, closes: Query<(Entity, &StatisticsControl), Without<ModalScrim>>, mut commands: Commands| {
            for (entity, control) in &closes {
                if matches!(control, StatisticsControl::CancelCleanup)
                    && entity != close.original_event_target()
                {
                    commands.trigger(Activate { entity });
                }
            }
        })
        Children [
            Node { width: percent(90), max_width: rem(STATISTICS_CLEANUP_DIALOG_MAX_WIDTH_PX), max_height: percent(85), padding: UiRect::all(rem(space::S16)),
                flex_direction: FlexDirection::Column, row_gap: rem(space::S12) }
            StatisticsConfirmationCard ModalDialogCard ModalDialog AccessibilityNode(semantic) LocalizedLabel::plain("rules_stats_cleanup_title") ThemeBackgroundColor({Token::Surface})
            Children [
                LocalizedText::plain("rules_stats_cleanup_title") TextRole(Role::Heading)
                --
                LocalizedText::plain("rules_stats_cleanup_description") TextRole(Role::Caption)
                --
                Node { width: percent(100), min_height: px(0.0), flex_shrink: 1.0, overflow: Overflow::scroll_y() }
                ScrollArea
                Children [ Text(String::new()) StatisticsLine::Confirmation TextRole(Role::Body) ]
                --
                Node { flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S12), row_gap: rem(space::S8), flex_shrink: 0.0 }
                Children [ @{dismiss_control("modal_cancel", StatisticsControl::CancelCleanup, &palette)} -- @{control("rules_stats_cleanup_confirm", StatisticsControl::ConfirmCleanup, &palette)} ]
            ]
        ]
    });
}
