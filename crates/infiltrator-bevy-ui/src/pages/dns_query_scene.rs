//! Persistent BSN modal: native query input, every record type, three sections and paging.
use crate::localized_widgets::localized_field_scene;
use crate::pages::dns_query::{
    QueryAction, QueryLine, QueryModalCard, QueryModalRoot, QueryNameField, QueryTypeMenu,
};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{CommandsSceneExt, Scene, bsn, on};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, Display, FlexDirection, FlexWrap, GlobalZIndex, JustifyContent, Node, Overflow,
    PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::ScrollArea;
use bevy::ui_widgets::{Activate, ModalDialog, ModalDialogBarrier, RequestClose};
use infiltrator_bevy_widgets::button::{
    ButtonLabel, ButtonSize, ButtonVariant, button_with_label_scene,
};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns_query::DnsRecordType;
use infiltrator_presentation::dns_query_actions::QuerySection;

/// The query modal dialog's maximum width in baseline pixels.
const DNS_QUERY_MODAL_MAX_WIDTH_PX: f32 = 640.0;

/// The query modal dialog's maximum height in baseline pixels.
const DNS_QUERY_MODAL_MAX_HEIGHT_PX: f32 = 640.0;

/// The record-type menu's top offset inside the modal, in baseline pixels.
const DNS_QUERY_TYPE_MENU_TOP_PX: f32 = 100.0;

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
        // The query modal is a modal dialog on Bevy's official behaviour
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
        on(|close: On<RequestClose>, cancels: Query<(Entity, &QueryAction)>, mut commands: Commands| {
            // A close request that names the modal's own close control is that
            // control's business (it already asked to close); every other
            // close request — the barrier's click-outside and the Escape
            // interception — runs the modal's own close path.
            for (entity, action) in &cancels {
                if matches!(action, QueryAction::Cancel) && entity != close.original_event_target() {
                    commands.trigger(Activate { entity });
                }
            }
        })
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim ThemeBackgroundColor({ Token::Scrim })
            --
            Node {
                width: percent(90), max_width: rem(DNS_QUERY_MODAL_MAX_WIDTH_PX), height: percent(90), max_height: rem(DNS_QUERY_MODAL_MAX_HEIGHT_PX),
                padding: UiRect::all(rem(space::S12)),
                flex_direction: FlexDirection::Column, row_gap: rem(space::S6),
            }
            ModalDialogCard QueryModalCard ThemeBackgroundColor({ Token::Surface })
            ModalDialog
            AccessibilityNode(semantic) LocalizedLabel::plain("dns_query_title")
            Children [
                LocalizedText::plain("dns_query_title") TextRole(Role::Heading)
                --
                @{ (localized_field_scene(String::new(), LocalizedText::plain("dns_query_name"), palette),
                    bsn! { QueryNameField NativeTextField(0) }) }
                --
                Node { width: percent(100), column_gap: rem(space::S8), align_items: AlignItems::Center }
                Children [
                    @{ query_button(QueryAction::TypePicker, LocalizedText::plain("dns_query_record_type"), palette) }
                    --
                    Text(String::new()) QueryLine::RecordType TextRole(Role::BodyStrong)
                ]
                --
                Node {
                    position_type: PositionType::Absolute, top: rem(DNS_QUERY_TYPE_MENU_TOP_PX), left: rem(space::S12), right: rem(space::S12),
                    flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S4), row_gap: rem(space::S4),
                    padding: UiRect::all(rem(space::S8)), display: Display::None,
                }
                QueryTypeMenu GlobalZIndex(117) ThemeBackgroundColor({ Token::SurfaceElevated })
                Children [ { types } ]
                --
                Text(String::new()) QueryLine::Status TextRole(Role::Caption)
                --
                Node { width: percent(100), column_gap: rem(space::S8) }
                Children [ { sections } ]
                --
                Node {
                    width: percent(100), flex_grow: 1.0, min_height: px(0.0),
                    overflow: Overflow::scroll_y(), flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S6),
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
                Node { width: percent(100), column_gap: rem(space::S8), align_items: AlignItems::Center }
                Children [
                    @{ query_button(QueryAction::Previous, LocalizedText::plain("dns_query_previous_page"), palette) }
                    --
                    Text(String::new()) QueryLine::Counter TextRole(Role::Caption)
                    --
                    @{ query_button(QueryAction::Next, LocalizedText::plain("dns_query_next_page"), palette) }
                ]
                --
                Node { width: percent(100), column_gap: rem(space::S8), justify_content: JustifyContent::FlexEnd }
                Children [
                    @{ (query_button(QueryAction::Cancel, LocalizedText::plain("dns_query_close"), palette), bsn! {
                        // The close control asks the official layer to close;
                        // the barrier observer runs the modal's own close path.
                        on(|activate: On<Activate>, mut commands: Commands| {
                            commands.trigger(RequestClose { source: activate.entity });
                        })
                    }) }
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
