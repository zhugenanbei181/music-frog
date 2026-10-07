//! One declarative native overlay retains review, failure, progress and actual receipt.
use crate::localized_widgets::localized_button_scene;
use crate::pages::logs_export::{LogsExportAction, LogsExportCard, LogsExportLine, LogsExportRoot};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::{LineBreak, TextLayout};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderColor, Display, FlexDirection, FlexWrap, GlobalZIndex,
    JustifyContent, Node, Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
pub fn spawn(mut commands: Commands, palette: Res<UiPalette>) {
    commands.spawn_scene(scene(&palette));
}
fn scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100),
            display: Display::None, align_items: AlignItems::Center, justify_content: JustifyContent::Center }
        LogsExportRoot GlobalZIndex(115)
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim BackgroundColor({palette.scrim}) LogsExportAction::Cancel
            --
            Node { width: percent(90), max_width: px(560.0), max_height: percent(85), padding: UiRect::all(px(16.0)),
                flex_direction: FlexDirection::Column, row_gap: px(12.0), border: UiRect::all(px(1.0)) }
            ModalDialogCard LogsExportCard BackgroundColor({palette.surface}) BorderColor::all(edge)
            AccessibilityNode(semantic) LocalizedLabel::plain("logs_export_title")
            Children [
                LocalizedText::plain("logs_export_title") TextRole(Role::Heading)
                --
                Node { width: percent(100), min_height: px(0.0), flex_shrink: 1.0,
                    flex_direction: FlexDirection::Column, row_gap: px(12.0), overflow: Overflow::scroll_y() }
                ScrollArea
                Children [
                    Text(String::new()) LogsExportLine::Status TextRole(Role::Body)
                    --
                    Text(String::new()) LogsExportLine::Details TextRole(Role::Caption)
                    --
                    Text(String::new()) LogsExportLine::Path TextRole(Role::Caption)
                    TextLayout::linebreak(LineBreak::WordOrCharacter)
                ]
                --
                Node { width: percent(100), column_gap: px(12.0), row_gap: px(8.0),
                    flex_wrap: FlexWrap::Wrap, flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd }
                Children [
                    @{(localized_button_scene(LocalizedText::plain("logs_export_cancel"), ButtonVariant::Default, palette), bsn! { LogsExportAction::Cancel })}
                    --
                    @{(localized_button_scene(LocalizedText::plain("logs_export_confirm"), ButtonVariant::Primary, palette), bsn! { LogsExportAction::Confirm })}
                    --
                    @{(localized_button_scene(LocalizedText::plain("logs_export_retry"), ButtonVariant::Primary, palette), bsn! { LogsExportAction::Retry })}
                ]
            ]
        ]
    }
}
