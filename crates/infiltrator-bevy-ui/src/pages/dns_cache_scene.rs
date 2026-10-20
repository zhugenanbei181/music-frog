//! A full-window declarative modal keeps confirmation, progress and results visible.
use crate::localized_widgets::localized_button_scene;
use crate::pages::dns_cache::{CacheAction, CacheLine, CacheModalCard, CacheModalRoot};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, Display, FlexDirection, GlobalZIndex, JustifyContent, Node, PositionType, UiRect,
    percent,
};
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
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim ThemeBackgroundColor({ Token::Scrim }) CacheAction::Cancel
            --
            Node {
                width: percent(90), max_width: rem(DNS_CACHE_MODAL_MAX_WIDTH_PX), max_height: percent(90),
                padding: UiRect::all(rem(space::S20)),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S12),
            }
            ModalDialogCard CacheModalCard ThemeBackgroundColor({ Token::Surface })
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
                    @{ (localized_button_scene(LocalizedText::plain("dns_cache_cancel"), ButtonVariant::Default, palette), bsn! { CacheAction::Cancel }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_cache_confirm"), ButtonVariant::Danger, palette), bsn! { CacheAction::Confirm }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_cache_retry"), ButtonVariant::Primary, palette), bsn! { CacheAction::Retry }) }
                ]
            ]
        ]
    }
}
