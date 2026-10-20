//! A full-window declarative modal keeps row editing separate from the scrolled DNS page.
use crate::localized_widgets::{localized_button_scene, localized_field_scene};
use crate::pages::dns_hosts::{
    DnsHostsApplyButton, DnsHostsDomainField, DnsHostsEditorField, DnsHostsStatusLine, HostAction,
    HostsEmptyHint, HostsLegacyHint, HostsModalCard, HostsModalRoot, HostsRows,
};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::{
    AlignItems, Display, FlexDirection, GlobalZIndex, JustifyContent, Node, Overflow, PositionType,
    UiRect, percent, px,
};
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::theme::space;

/// The hosts modal dialog's maximum width in baseline pixels.
const DNS_HOSTS_MODAL_MAX_WIDTH_PX: f32 = 600.0;

/// The hosts rows area's fixed height in baseline pixels.
const DNS_HOSTS_ROWS_HEIGHT_PX: f32 = 128.0;

/// The hosts rows area's minimum height in baseline pixels.
const DNS_HOSTS_ROWS_MIN_HEIGHT_PX: f32 = 40.0;

pub fn spawn_modal(mut commands: Commands, palette: Res<UiPalette>) {
    commands.spawn_scene(modal(&palette));
}
pub fn modal(palette: &UiPalette) -> impl Scene + use<> {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), display: Display::None, align_items: AlignItems::Center, justify_content: JustifyContent::Center }
        HostsModalRoot GlobalZIndex(114)
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim ThemeBackgroundColor({ Token::Scrim }) HostAction::Cancel
            --
            Node { width: percent(90), max_width: rem(DNS_HOSTS_MODAL_MAX_WIDTH_PX), max_height: percent(90), padding: UiRect::all(rem(space::S16)), flex_direction: FlexDirection::Column, row_gap: rem(space::S8) }
            ModalDialogCard HostsModalCard ThemeBackgroundColor({ Token::Surface }) AccessibilityNode(semantic) LocalizedLabel::plain("dns_hosts_title")
            Children [
                LocalizedText::plain("dns_hosts_title") TextRole(Role::Heading)
                --
                LocalizedText::plain("dns_hosts_desc") TextRole(Role::Caption)
                --
                Node { width: percent(100), column_gap: rem(space::S8) }
                Children [
                    Node { width: percent(48), min_width: px(0.0), flex_direction: FlexDirection::Column, row_gap: rem(space::S4) }
                    DnsHostsEditorField
                    Children [ LocalizedText::plain("dns_hosts_address") TextRole(Role::Caption) --
                        @{ (localized_field_scene(String::new(), LocalizedText::plain("dns_hosts_address"), palette), bsn! { NativeTextField(0) }) } ]
                    --
                    Node { width: percent(48), min_width: px(0.0), flex_direction: FlexDirection::Column, row_gap: rem(space::S4) }
                    DnsHostsDomainField
                    Children [ LocalizedText::plain("dns_hosts_domain") TextRole(Role::Caption) --
                        @{ (localized_field_scene(String::new(), LocalizedText::plain("dns_hosts_domain"), palette), bsn! { NativeTextField(1) }) } ]
                ]
                --
                Node { width: percent(100), column_gap: rem(space::S8), justify_content: JustifyContent::SpaceBetween }
                Children [
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_add"), ButtonVariant::Default, palette), bsn! { HostAction::CommitRow }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_cancel_row"), ButtonVariant::Default, palette), bsn! { HostAction::CancelRow }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_import_legacy"), ButtonVariant::Default, palette), bsn! { HostAction::ImportLegacy }) }
                ]
                --
                Node { width: percent(100), height: rem(DNS_HOSTS_ROWS_HEIGHT_PX), min_height: rem(DNS_HOSTS_ROWS_MIN_HEIGHT_PX), flex_shrink: 1.0, flex_direction: FlexDirection::Column, row_gap: rem(space::S4), overflow: Overflow::scroll_y() }
                ScrollArea HostsRows
                --
                LocalizedText::plain("dns_hosts_empty") HostsEmptyHint TextRole(Role::Caption)
                --
                LocalizedText::plain("common_message") HostsLegacyHint TextRole(Role::Caption)
                --
                LocalizedText::plain("dns_hosts_unobserved") DnsHostsStatusLine TextRole(Role::Caption)
                --
                Node { width: percent(100), column_gap: rem(space::S8), justify_content: JustifyContent::SpaceBetween, flex_shrink: 0.0 }
                Children [
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_clear_draft"), ButtonVariant::Default, palette), bsn! { HostAction::Cancel }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_apply"), ButtonVariant::Primary, palette), bsn! { HostAction::Apply DnsHostsApplyButton }) }
                ]
            ]
        ]
    }
}
