//! One declarative native overlay retains review, failure, progress and actual receipt.
use crate::localized_widgets::localized_button_scene;
use crate::pages::logs_export::{LogsExportAction, LogsExportCard, LogsExportLine, LogsExportRoot};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{CommandsSceneExt, Scene, bsn, on};
use bevy::text::{LineBreak, TextLayout};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BorderColor, Display, FlexDirection, FlexWrap, GlobalZIndex, JustifyContent, Node,
    Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::ScrollArea;
use bevy::ui_widgets::{Activate, ModalDialog, ModalDialogBarrier, RequestClose};
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// One-off layout value (not a ladder rung): the widest the logs export modal
/// card may grow on a wide viewport.
const LOGS_EXPORT_MODAL_MAX_WIDTH_PX: f32 = 560.0;

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
        // The export modal is a modal dialog on Bevy's official behaviour
        // layer: the barrier owns the click-outside close and the Escape
        // interception, the card owns the focus containment, and every close
        // request travels as the official `RequestClose`.
        //
        // The barrier is the modal root — the only common ancestor of the
        // scrim and the card — because the official Escape interception
        // reaches the barrier through the focused entity and the official
        // `RequestClose` reaches it through the closing control, both of
        // which live under the card.
        ModalDialogBarrier
        on(|close: On<RequestClose>, cancels: Query<(Entity, &LogsExportAction)>, mut commands: Commands| {
            // A close request that names the modal's own cancel control is
            // that control's business (it already asked to close); every other
            // close request — the barrier's click-outside and the Escape
            // interception — runs the modal's own close path.
            for (entity, action) in &cancels {
                if matches!(action, LogsExportAction::Cancel) && entity != close.original_event_target() {
                    commands.trigger(Activate { entity });
                }
            }
        })
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim ThemeBackgroundColor({Token::Scrim})
            --
            Node { width: percent(90), max_width: rem(LOGS_EXPORT_MODAL_MAX_WIDTH_PX), max_height: percent(85), padding: UiRect::all(rem(space::S16)),
                flex_direction: FlexDirection::Column, row_gap: rem(space::S12), border: UiRect::all(px(palette.hairline_px)) }
            ModalDialogCard LogsExportCard ThemeBackgroundColor({Token::Surface}) BorderColor::all(edge)
            ModalDialog
            AccessibilityNode(semantic) LocalizedLabel::plain("logs_export_title")
            Children [
                LocalizedText::plain("logs_export_title") TextRole(Role::Heading)
                --
                Node { width: percent(100), min_height: px(0.0), flex_shrink: 1.0,
                    flex_direction: FlexDirection::Column, row_gap: rem(space::S12), overflow: Overflow::scroll_y() }
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
                Node { width: percent(100), column_gap: rem(space::S12), row_gap: rem(space::S8),
                    flex_wrap: FlexWrap::Wrap, flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd }
                Children [
                    @{(localized_button_scene(LocalizedText::plain("logs_export_cancel"), ButtonVariant::Default, palette), bsn! {
                        LogsExportAction::Cancel
                        // The cancel control asks the official layer to close;
                        // the barrier observer runs the modal's own close path.
                        on(|activate: On<Activate>, mut commands: Commands| {
                            commands.trigger(RequestClose { source: activate.entity });
                        })
                    })}
                    --
                    @{(localized_button_scene(LocalizedText::plain("logs_export_confirm"), ButtonVariant::Primary, palette), bsn! { LogsExportAction::Confirm })}
                    --
                    @{(localized_button_scene(LocalizedText::plain("logs_export_retry"), ButtonVariant::Primary, palette), bsn! { LogsExportAction::Retry })}
                ]
            ]
        ]
    }
}
