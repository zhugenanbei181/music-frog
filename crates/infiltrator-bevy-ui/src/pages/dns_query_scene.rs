//! Persistent BSN modal: native query input, every record type, three sections and paging.
use crate::localized_widgets::localized_field_scene;
use crate::pages::dns_query::{
    QueryAction, QueryLine, QueryModalCard, QueryModalRoot, QueryNameField, QueryTypeMenu,
};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, FlexWrap, GlobalZIndex, JustifyContent,
    Node, Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::ScrollArea;
use infiltrator_application::dns_query_actions::QuerySection;
use infiltrator_bevy_widgets::button::{
    ButtonLabel, ButtonSize, ButtonVariant, button_with_label_scene,
};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_contract::dns_query::DnsRecordType;

pub fn query_button(
    action: QueryAction,
    copy: LocalizedText,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let label = copy.clone();
    (
        button_with_label_scene(
            bsn! {
                LocalizedText { key: { copy.key }, params: { copy.params } }
                TextRole(Role::Caption) ButtonLabel
            },
            ButtonVariant::Default,
            ButtonSize::Sm,
            palette,
        ),
        bsn! { action LocalizedLabel(label) },
    )
}
pub fn spawn(mut commands: Commands, palette: Res<UiPalette>) {
    commands.spawn_scene(modal(&palette));
}
fn modal(palette: &UiPalette) -> impl Scene + use<> {
    let types: Vec<Box<dyn Scene>> = DnsRecordType::ALL
        .into_iter()
        .map(|kind| {
            Box::new(query_button(
                QueryAction::Type(kind),
                LocalizedText::new(
                    "dns_query_type_choice",
                    vec![("type", kind.wire().to_string())],
                ),
                palette,
            )) as Box<dyn Scene>
        })
        .collect();
    let sections: Vec<Box<dyn Scene>> = QuerySection::ALL
        .into_iter()
        .map(|section| {
            Box::new(query_button(
                QueryAction::Section(section),
                LocalizedText::plain(section.key()),
                palette,
            )) as Box<dyn Scene>
        })
        .collect();
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100), height: percent(100),
            display: Display::None,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        QueryModalRoot GlobalZIndex(116)
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim BackgroundColor({ palette.scrim }) QueryAction::Cancel
            --
            Node {
                width: percent(90), max_width: px(640.0), height: percent(90), max_height: px(640.0),
                padding: UiRect::all(px(12.0)),
                flex_direction: FlexDirection::Column, row_gap: px(6.0),
            }
            ModalDialogCard QueryModalCard BackgroundColor({ palette.surface })
            AccessibilityNode(semantic) LocalizedLabel::plain("dns_query_title")
            Children [
                LocalizedText::plain("dns_query_title") TextRole(Role::Heading)
                --
                @{ (localized_field_scene(String::new(), LocalizedText::plain("dns_query_name"), palette),
                    bsn! { QueryNameField NativeTextField(0) }) }
                --
                Node { width: percent(100), column_gap: px(8.0), align_items: AlignItems::Center }
                Children [
                    @{ query_button(QueryAction::TypePicker, LocalizedText::plain("dns_query_record_type"), palette) }
                    --
                    Text(String::new()) QueryLine::RecordType TextRole(Role::BodyStrong)
                ]
                --
                Node {
                    position_type: PositionType::Absolute, top: px(100.0), left: px(12.0), right: px(12.0),
                    flex_wrap: FlexWrap::Wrap, column_gap: px(4.0), row_gap: px(4.0),
                    padding: UiRect::all(px(8.0)), display: Display::None,
                }
                QueryTypeMenu GlobalZIndex(117) BackgroundColor({ palette.surface_elevated })
                Children [ { types } ]
                --
                Text(String::new()) QueryLine::Status TextRole(Role::Caption)
                --
                Node { width: percent(100), column_gap: px(8.0) }
                Children [ { sections } ]
                --
                Node {
                    width: percent(100), flex_grow: 1.0, min_height: px(0.0),
                    overflow: Overflow::scroll_y(), flex_direction: FlexDirection::Column,
                    row_gap: px(6.0),
                }
                ScrollArea
                Children [
                    Text(String::new()) QueryLine::Provenance TextRole(Role::Caption)
                    --
                    Text(String::new()) QueryLine::Question TextRole(Role::BodyStrong)
                    --
                    Text(String::new()) QueryLine::Flags TextRole(Role::Mono)
                    --
                    Text(String::new()) QueryLine::Records TextRole(Role::Mono)
                ]
                --
                Node { width: percent(100), column_gap: px(8.0), align_items: AlignItems::Center }
                Children [
                    @{ query_button(QueryAction::Previous, LocalizedText::plain("dns_query_previous_page"), palette) }
                    --
                    Text(String::new()) QueryLine::Counter TextRole(Role::Caption)
                    --
                    @{ query_button(QueryAction::Next, LocalizedText::plain("dns_query_next_page"), palette) }
                ]
                --
                Node { width: percent(100), column_gap: px(8.0), justify_content: JustifyContent::FlexEnd }
                Children [
                    @{ query_button(QueryAction::Cancel, LocalizedText::plain("dns_query_close"), palette) }
                    --
                    @{ query_button(QueryAction::Run, LocalizedText::plain("dns_query_run"), palette) }
                    --
                    @{ query_button(QueryAction::Retry, LocalizedText::plain("dns_query_retry"), palette) }
                    --
                    @{ query_button(QueryAction::Settings, LocalizedText::plain("dns_query_settings"), palette) }
                ]
            ]
        ]
    }
}
