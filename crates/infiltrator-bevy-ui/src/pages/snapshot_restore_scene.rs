//! An independent native review overlay with full-content scrolling and explicit actions.
use crate::localized_widgets::localized_button_scene;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
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
    AlignItems, BorderRadius, Display, FlexDirection, FlexWrap, GlobalZIndex, JustifyContent,
    Node, Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::{
    Activate, Button, ModalDialog, ModalDialogBarrier, RequestClose, ScrollArea,
};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// One-off layout value (not a ladder rung): the widest the snapshot restore
/// review modal card may grow on a wide viewport.
const SNAPSHOT_RESTORE_MODAL_MAX_WIDTH_PX: f32 = 700.0;

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
/// The review overlay's dismiss control: it asks the official dialog layer to
/// close instead of closing the review itself.
fn dismiss_control(
    key: &'static str,
    action: RestoreControl,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        @{localized_button_scene(LocalizedText::plain(key), ButtonVariant::Secondary, palette)}
        action ReviewControl
        TabIndex(0)
        // The dismiss control asks the official layer to close; the page never
        // closes the restore review on its own.
        on(|activate: On<Activate>, mut commands: Commands| {
            commands.trigger(RequestClose { source: activate.entity });
        })
    }
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
        // The barrier is the modal layer's root: it owns the click-outside
        // close and the Escape interception, and the card is its child, so the
        // official behaviour layer reaches the focused control.
        ModalScrim cancel ReviewControl ThemeBackgroundColor({Token::Scrim}) ModalDialogBarrier
        // The close request travels as the official `RequestClose` and the
        // page's own close path runs from it.
        on(|close: On<RequestClose>, closes: Query<(Entity, &RestoreControl), Without<ModalScrim>>, mut commands: Commands| {
            for (entity, control) in &closes {
                if matches!(control, RestoreControl::Cancel)
                    && entity != close.original_event_target()
                {
                    commands.trigger(Activate { entity });
                }
            }
        })
        Children [
            Node { width: percent(90), max_width: rem(SNAPSHOT_RESTORE_MODAL_MAX_WIDTH_PX), max_height: percent(85), padding: UiRect::all(rem(space::S16)), flex_direction: FlexDirection::Column, row_gap: rem(space::S12), border_radius: BorderRadius::all(rem(palette.modal_radius_px)) }
            ModalDialogCard RestoreCard ModalDialog AccessibilityNode(semantic) LocalizedLabel::plain("snapshot_restore_title") ThemeBackgroundColor({Token::Surface})
            Children [
                LocalizedText::plain("snapshot_restore_title") TextRole(Role::Heading)
                --
                Node { width: percent(100), min_height: px(0), flex_shrink: 1.0, overflow: Overflow::scroll_y() } ScrollArea
                Children [ Node { width: percent(100), min_width: px(0), flex_direction: FlexDirection::Column, row_gap: rem(space::S8) }
                    Children [ Text(String::new()) status TextRole(Role::Body)
                        -- Text(String::new()) details TextRole(Role::Mono)
                        -- Text(String::new()) content TextRole(Role::Mono) ] ]
                -- Node { flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S8), row_gap: rem(space::S8), flex_shrink: 0.0 }
                Children [ @{dismiss_control("snapshot_restore_cancel", RestoreControl::Cancel, &palette)}
                    -- @{control("snapshot_restore_confirm", RestoreControl::Confirm, &palette)}
                    -- @{control("snapshot_restore_retry", RestoreControl::Retry, &palette)} ]
            ]
        ]
    });
}
