//! Declarative inspector and a separate cleanup review overlay.
use crate::localized_widgets::localized_button_scene;
use crate::pages::rules_statistics::{
    StatisticsCard, StatisticsConfirmation, StatisticsConfirmationCard, StatisticsControl,
    StatisticsLine, StatisticsRows,
};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, FlexWrap, GlobalZIndex, JustifyContent,
    Node, Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::ScrollArea;
use infiltrator_application::rule_statistics_workbench::StatisticsTab;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};

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
fn metric(key: &'static str, line: StatisticsLine) -> impl Scene + use<> {
    bsn! {
        Node { min_width: px(120.0), flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: px(4.0) }
        Children [ LocalizedText::plain(key) TextRole(Role::Caption) -- Text(String::new()) StatisticsLine { .. { line } } TextRole(Role::BodyStrong) ]
    }
}
pub fn scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { width: percent(100), height: px(320.0), min_height: px(320.0), flex_shrink: 0.0,
            padding: UiRect::all(px(12.0)), flex_direction: FlexDirection::Column, row_gap: px(8.0) }
        StatisticsCard BackgroundColor({palette.surface_elevated})
        Children [
            LocalizedText::plain("rule_hit_title") TextRole(Role::BodyStrong)
            --
            Text(String::new()) StatisticsLine::Status TextRole(Role::Caption)
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: px(8.0), row_gap: px(4.0), flex_shrink: 0.0 }
            Children [
                @{control("rules_stats_summary_tab", StatisticsControl::Tab(StatisticsTab::Summary), palette)}
                --
                @{control("rules_stats_top_tab", StatisticsControl::Tab(StatisticsTab::TopHits), palette)}
                --
                @{control("rules_stats_inactive_tab", StatisticsControl::Tab(StatisticsTab::Inactive), palette)}
            ]
            --
            Node { width: percent(100), min_height: px(0.0), flex_grow: 1.0, flex_shrink: 1.0,
                flex_direction: FlexDirection::Column, row_gap: px(8.0), overflow: Overflow::scroll_y() }
            ScrollArea
            Children [
                LocalizedText::plain("rule_hit_desc") TextRole(Role::Caption)
                --
                Text(String::new()) StatisticsLine::Source TextRole(Role::Caption)
                --
                Node { width: percent(100), flex_wrap: FlexWrap::Wrap, column_gap: px(8.0), row_gap: px(8.0) }
                Children [ @{metric("rule_hit_total_hits", StatisticsLine::Total)} -- @{metric("rule_hit_dead_count", StatisticsLine::Dead)} -- @{metric("rule_hit_cidr_conflicts", StatisticsLine::Cidr)} -- @{metric("rule_hit_match_latency", StatisticsLine::Latency)} ]
                --
                Text(String::new()) StatisticsLine::Last TextRole(Role::Body)
                --
                Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(8.0) }
                StatisticsRows
                --
                Text(String::new()) StatisticsLine::Empty TextRole(Role::Caption)
            ]
            --
            Text(String::new()) StatisticsLine::Page TextRole(Role::Caption)
            --
            Text(String::new()) StatisticsLine::Feedback TextRole(Role::Caption)
            --
            Node { width: percent(100), flex_wrap: FlexWrap::Wrap, column_gap: px(8.0), row_gap: px(4.0), flex_shrink: 0.0 }
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
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim StatisticsControl::CancelCleanup BackgroundColor({palette.scrim})
            --
            Node { width: percent(90), max_width: px(560.0), max_height: percent(85), padding: UiRect::all(px(16.0)),
                flex_direction: FlexDirection::Column, row_gap: px(12.0) }
            StatisticsConfirmationCard ModalDialogCard AccessibilityNode(semantic) LocalizedLabel::plain("rules_stats_cleanup_title") BackgroundColor({palette.surface})
            Children [
                LocalizedText::plain("rules_stats_cleanup_title") TextRole(Role::Heading)
                --
                LocalizedText::plain("rules_stats_cleanup_description") TextRole(Role::Caption)
                --
                Node { width: percent(100), min_height: px(0.0), flex_shrink: 1.0, overflow: Overflow::scroll_y() }
                ScrollArea
                Children [ Text(String::new()) StatisticsLine::Confirmation TextRole(Role::Body) ]
                --
                Node { flex_wrap: FlexWrap::Wrap, column_gap: px(12.0), row_gap: px(8.0), flex_shrink: 0.0 }
                Children [ @{control("modal_cancel", StatisticsControl::CancelCleanup, &palette)} -- @{control("rules_stats_cleanup_confirm", StatisticsControl::ConfirmCleanup, &palette)} ]
            ]
        ]
    });
}
