//! A full-window declarative modal keeps confirmation, progress and results visible.
use crate::localized_widgets::localized_button_scene;
use crate::pages::dns_cache::{CacheAction, CacheLine, CacheModalCard, CacheModalRoot};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, GlobalZIndex, JustifyContent, Node,
    PositionType, UiRect, percent, px,
};
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
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
            ModalScrim BackgroundColor({ palette.scrim }) CacheAction::Cancel
            --
            Node {
                width: percent(90), max_width: px(560.0), max_height: percent(90),
                padding: UiRect::all(px(20.0)),
                flex_direction: FlexDirection::Column,
                row_gap: px(12.0),
            }
            ModalDialogCard CacheModalCard BackgroundColor({ palette.surface })
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
                Node { width: percent(100), column_gap: px(12.0), justify_content: JustifyContent::FlexEnd }
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
