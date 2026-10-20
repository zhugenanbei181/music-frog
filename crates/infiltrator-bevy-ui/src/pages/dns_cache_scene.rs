//! A full-window declarative modal keeps confirmation, progress and results visible.
use crate::localized_widgets::localized_button_scene;
use crate::pages::dns_cache::{CacheAction, CacheLine, CacheModalCard, CacheModalRoot};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{CommandsSceneExt, Scene, bsn, on};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BorderRadius, Display, FlexDirection, GlobalZIndex, JustifyContent, Node,
    PositionType, UiRect, percent,
};
use bevy::ui_widgets::{Activate, ModalDialog, ModalDialogBarrier, RequestClose};
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// One-off layout value (not a ladder rung): the widest the DNS cache modal
/// card may grow on a wide viewport.
const DNS_CACHE_MODAL_MAX_WIDTH_PX: f32 = 560.0;

pub fn spawn(mut commands: Commands, palette: Res<UiPalette>) {
    commands.spawn_scene(modal(&palette));
}
fn modal(palette: &UiPalette) -> impl Scene + use<> {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100), height: percent(100),
            display: Display::None,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        CacheModalRoot GlobalZIndex(115)
        // The cache modal is a modal dialog on Bevy's official behaviour
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
        on(|close: On<RequestClose>, cancels: Query<(Entity, &CacheAction)>, mut commands: Commands| {
            // A close request that names the modal's own cancel control is
            // that control's business (it already asked to close); every other
            // close request — the barrier's click-outside and the Escape
            // interception — runs the modal's own close path.
            for (entity, action) in &cancels {
                if matches!(action, CacheAction::Cancel) && entity != close.original_event_target() {
                    commands.trigger(Activate { entity });
                }
            }
        })
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim ThemeBackgroundColor({ Token::Scrim })
            --
            Node {
                width: percent(90), max_width: rem(DNS_CACHE_MODAL_MAX_WIDTH_PX), max_height: percent(90),
                padding: UiRect::all(rem(space::S20)),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S12),
                // The centered dialog takes the contract's modal rung.
                border_radius: BorderRadius::all(rem(palette.modal_radius_px)),
            }
            ModalDialogCard CacheModalCard ThemeBackgroundColor({ Token::Surface })
            ModalDialog
            AccessibilityNode(semantic) LocalizedLabel::plain("dns_cache_title")
            Children [
                LocalizedText::plain("dns_cache_title") TextRole(Role::Heading)
                --
                Text(String::new()) CacheLine::Status TextRole(Role::Body)
                --
                Text(String::new()) CacheLine::ReportLabel TextRole(Role::Caption)
                --
                Text(String::new()) CacheLine::FakeIp TextRole(Role::Caption)
                --
                Text(String::new()) CacheLine::System TextRole(Role::Caption)
                --
                Node { width: percent(100), column_gap: rem(space::S12), justify_content: JustifyContent::FlexEnd }
                Children [
                    @{ (localized_button_scene(LocalizedText::plain("dns_cache_cancel"), ButtonVariant::Default, palette), bsn! {
                        CacheAction::Cancel
                        // The cancel control asks the official layer to close;
                        // the barrier observer runs the modal's own close path.
                        on(|activate: On<Activate>, mut commands: Commands| {
                            commands.trigger(RequestClose { source: activate.entity });
                        })
                    }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_cache_confirm"), ButtonVariant::Danger, palette), bsn! { CacheAction::Confirm }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_cache_retry"), ButtonVariant::Primary, palette), bsn! { CacheAction::Retry }) }
                ]
            ]
        ]
    }
}
