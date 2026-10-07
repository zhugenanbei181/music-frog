//! A full-window declarative modal keeps row editing separate from the scrolled DNS page.
use crate::localized_widgets::{localized_button_scene, localized_field_scene};
use crate::pages::dns_hosts::{
    DnsHostsApplyButton, DnsHostsDomainField, DnsHostsEditorField, DnsHostsStatusLine, HostAction,
    HostsEmptyHint, HostsLegacyHint, HostsModalCard, HostsModalRoot, HostsRows,
};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, GlobalZIndex, JustifyContent, Node,
    Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
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
            ModalScrim BackgroundColor({ palette.scrim }) HostAction::Cancel
            --
            Node { width: percent(90), max_width: px(600.0), max_height: percent(90), padding: UiRect::all(px(16.0)), flex_direction: FlexDirection::Column, row_gap: px(8.0) }
            ModalDialogCard HostsModalCard BackgroundColor({ palette.surface }) AccessibilityNode(semantic) LocalizedLabel::plain("dns_hosts_title")
            Children [
                LocalizedText::plain("dns_hosts_title") TextRole(Role::Heading)
                --
                LocalizedText::plain("dns_hosts_desc") TextRole(Role::Caption)
                --
                Node { width: percent(100), column_gap: px(8.0) }
                Children [
                    Node { width: percent(48), min_width: px(0.0), flex_direction: FlexDirection::Column, row_gap: px(4.0) }
                    DnsHostsEditorField
                    Children [ LocalizedText::plain("dns_hosts_address") TextRole(Role::Caption) --
                        @{ (localized_field_scene(String::new(), LocalizedText::plain("dns_hosts_address"), palette), bsn! { NativeTextField(0) }) } ]
                    --
                    Node { width: percent(48), min_width: px(0.0), flex_direction: FlexDirection::Column, row_gap: px(4.0) }
                    DnsHostsDomainField
                    Children [ LocalizedText::plain("dns_hosts_domain") TextRole(Role::Caption) --
                        @{ (localized_field_scene(String::new(), LocalizedText::plain("dns_hosts_domain"), palette), bsn! { NativeTextField(1) }) } ]
                ]
                --
                Node { width: percent(100), column_gap: px(8.0), justify_content: JustifyContent::SpaceBetween }
                Children [
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_add"), ButtonVariant::Default, palette), bsn! { HostAction::CommitRow }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_cancel_row"), ButtonVariant::Default, palette), bsn! { HostAction::CancelRow }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_import_legacy"), ButtonVariant::Default, palette), bsn! { HostAction::ImportLegacy }) }
                ]
                --
                Node { width: percent(100), height: px(128.0), min_height: px(40.0), flex_shrink: 1.0, flex_direction: FlexDirection::Column, row_gap: px(4.0), overflow: Overflow::scroll_y() }
                ScrollArea HostsRows
                --
                LocalizedText::plain("dns_hosts_empty") HostsEmptyHint TextRole(Role::Caption)
                --
                LocalizedText::plain("common_message") HostsLegacyHint TextRole(Role::Caption)
                --
                LocalizedText::plain("dns_hosts_unobserved") DnsHostsStatusLine TextRole(Role::Caption)
                --
                Node { width: percent(100), column_gap: px(8.0), justify_content: JustifyContent::SpaceBetween, flex_shrink: 0.0 }
                Children [
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_clear_draft"), ButtonVariant::Default, palette), bsn! { HostAction::Cancel }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_hosts_apply"), ButtonVariant::Primary, palette), bsn! { HostAction::Apply DnsHostsApplyButton }) }
                ]
            ]
        ]
    }
}
