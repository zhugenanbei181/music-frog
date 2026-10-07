//! An independent native review overlay with full-content scrolling and explicit actions.
use crate::localized_widgets::localized_button_scene;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, FlexWrap, GlobalZIndex, JustifyContent,
    Node, Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::{Button, ScrollArea};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
#[derive(Component, Clone, Copy)]
#[require(Button, ButtonDisabled)]
pub enum RestoreControl {
    Confirm,
    Cancel,
    Retry,
}
#[derive(Component, Clone, Default)]
pub struct RestoreOverlay;
#[derive(Component, Clone, Default)]
pub struct RestoreCard;
#[derive(Component, Clone, Copy, Default)]
pub enum RestoreLine {
    #[default]
    Status,
    Details,
    Content,
}
#[derive(Component, Clone, Default)]
pub struct ReviewControl;
fn control(key: &'static str, action: RestoreControl, palette: &UiPalette) -> impl Scene + use<> {
    bsn! { @{localized_button_scene(LocalizedText::plain(key), ButtonVariant::Secondary, palette)} action ReviewControl }
}
pub fn spawn_review(mut commands: Commands, palette: Res<UiPalette>) {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    let cancel = RestoreControl::Cancel;
    let status = RestoreLine::Status;
    let details = RestoreLine::Details;
    let content = RestoreLine::Content;
    commands.spawn_scene(bsn! {
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), display: Display::None, align_items: AlignItems::Center, justify_content: JustifyContent::Center }
        RestoreOverlay GlobalZIndex(150)
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim cancel ReviewControl BackgroundColor({palette.scrim})
            --
            Node { width: percent(90), max_width: px(700), max_height: percent(85), padding: UiRect::all(px(16)), flex_direction: FlexDirection::Column, row_gap: px(12) }
            ModalDialogCard RestoreCard AccessibilityNode(semantic) LocalizedLabel::plain("snapshot_restore_title") BackgroundColor({palette.surface})
            Children [
                LocalizedText::plain("snapshot_restore_title") TextRole(Role::Heading)
                --
                Node { width: percent(100), min_height: px(0), flex_shrink: 1.0, overflow: Overflow::scroll_y() } ScrollArea
                Children [ Node { width: percent(100), min_width: px(0), flex_direction: FlexDirection::Column, row_gap: px(8) }
                    Children [ Text(String::new()) status TextRole(Role::Body)
                        -- Text(String::new()) details TextRole(Role::Mono)
                        -- Text(String::new()) content TextRole(Role::Mono) ] ]
                -- Node { flex_wrap: FlexWrap::Wrap, column_gap: px(8), row_gap: px(8), flex_shrink: 0.0 }
                Children [ @{control("snapshot_restore_cancel", RestoreControl::Cancel, &palette)}
                    -- @{control("snapshot_restore_confirm", RestoreControl::Confirm, &palette)}
                    -- @{control("snapshot_restore_retry", RestoreControl::Retry, &palette)} ]
            ]
        ]
    });
}
