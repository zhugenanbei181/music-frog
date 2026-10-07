//! Native Hosts launcher and editor state; the shared application owns draft semantics.
use crate::localized_widgets::localized_button_scene;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::scene::{Scene, bsn};
use bevy::ui::{FlexDirection, Node, percent, px};
use bevy::ui_widgets::Button;
use infiltrator_application::dns_hosts_editor::DnsHostsEditor;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_contract::command::RequestId;

#[derive(Resource, Default)]
pub struct DnsHostsEditorState {
    pub editor: DnsHostsEditor,
    pub request: Option<(RequestId, u64)>,
    pub sync_fields: bool,
}
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
#[require(Button, ButtonDisabled)]
pub enum HostAction {
    #[default]
    Open,
    Cancel,
    CommitRow,
    CancelRow,
    Edit(u64),
    Remove(u64),
    ImportLegacy,
    Apply,
}
#[derive(Component, Clone, Copy, Default)]
pub struct DnsHostsEditorField;
#[derive(Component, Clone, Copy, Default)]
pub struct DnsHostsDomainField;
#[derive(Component, Clone, Copy, Default)]
pub struct DnsHostsApplyButton;
#[derive(Component, Clone, Copy, Default)]
pub struct DnsHostsStatusLine;
#[derive(Component, Clone, Copy, Default)]
pub struct HostsSummary;
#[derive(Component, Clone, Copy, Default)]
pub struct HostsLegacyHint;
#[derive(Component, Clone, Copy, Default)]
pub struct HostsModalRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct HostsModalCard;
#[derive(Component, Clone, Copy, Default)]
pub struct HostsRows;
#[derive(Component, Clone, Copy, Default)]
pub struct HostsEmptyHint;
#[derive(Component, Clone, Copy, Default)]
pub struct HostRowIdentity(pub u64);
#[derive(Component, Clone, Copy, Default)]
pub struct HostRowText(pub u64, pub bool);

pub fn dns_hosts_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    surface_scene(
        vec![Box::new(bsn! {
            Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(8.0) }
            Children [
                LocalizedText::plain("dns_hosts_title") TextRole(Role::BodyStrong)
                --
                LocalizedText::plain("dns_hosts_unobserved") HostsSummary TextRole(Role::Caption)
                --
                @{ (localized_button_scene(LocalizedText::plain("dns_hosts_open"), ButtonVariant::Default, palette), bsn! { HostAction::Open }) }
            ]
        })],
        palette,
    )
}
