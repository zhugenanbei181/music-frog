//! Native multiline workbench and an independent export review layer.
use crate::localized_widgets::localized_button_scene;
use crate::pages::profiles_script_workbench::{ScriptControl, ScriptInput};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{CommandsSceneExt, Scene, bsn, on};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BorderRadius, Display, FlexDirection, FlexWrap, GlobalZIndex, JustifyContent, Node,
    Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::ScrollArea;
use bevy::ui_widgets::{Activate, ModalDialog, ModalDialogBarrier, RequestClose};
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::multiline_editor::multiline_editor_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::script_export::ScriptExportKind;
use infiltrator_contract::script_run::ScriptEditorField;

/// One-off layout value (not a ladder rung): the widest the script export
/// review modal card may grow on a wide viewport.
const SCRIPT_REVIEW_MODAL_MAX_WIDTH_PX: f32 = 560.0;

#[derive(Component, Clone, Copy, Default)]
pub struct ScriptWorkbenchCard;
#[derive(Component, Clone, Copy, Default)]
pub struct ScriptResultBody;
#[derive(Component, Clone, Copy, Default)]
pub struct ScriptResultRow(pub usize);
#[derive(Component, Clone, Copy, Default)]
pub struct ScriptExportOverlay;
#[derive(Component, Clone, Copy, Default)]
pub struct ScriptReviewControl;
#[derive(Component, Clone, Copy)]
pub enum ScriptReviewLine {
    Status,
    Details,
    Content,
    Path,
}

pub fn workbench_scene(palette: &UiPalette) -> impl Scene + use<> {
    let code_input = ScriptInput(ScriptEditorField::Code);
    let yaml_input = ScriptInput(ScriptEditorField::InputYaml);
    bsn! {
        Node { width: percent(100), min_width: px(0), flex_direction: FlexDirection::Column, row_gap: rem(space::S12), padding: UiRect::all(rem(space::S16)) }
        ThemeBackgroundColor({Token::Surface}) ScriptWorkbenchCard
        Children [
            LocalizedText::plain("script_sandbox_title") TextRole(Role::Heading)
            -- LocalizedText::plain("script_sandbox_presets") TextRole(Role::Caption)
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S8), row_gap: rem(space::S8) }
            Children [
                @{control("script_preset_country", ScriptControl::Preset("auto-country-groups"), palette)}
                --
                @{control("script_preset_remove_ads", ScriptControl::Preset("remove-ads"), palette)}
                --
                @{control("script_preset_streaming", ScriptControl::Preset("streaming-groups"), palette)}
                --
                @{control("script_preset_direct_china", ScriptControl::Preset("direct-china"), palette)}
                --
                @{control("script_sandbox_run", ScriptControl::Run, palette)}
                --
                @{control("logs_clear", ScriptControl::Clear, palette)}
                --
                @{control("script_workbench_retry", ScriptControl::Retry, palette)}
                --
                @{control("script_export_kind_directive_js", ScriptControl::Export(ScriptExportKind::DirectiveDslScript), palette)}
                --
                @{control("script_export_kind_package_json", ScriptControl::Export(ScriptExportKind::ExtensionPackageJson), palette)}
            ]
            --
            LocalizedText::plain("script_sandbox_code_input") TextRole(Role::Caption)
            --
            @{multiline_editor_scene("", "script_sandbox_code_input", 40, palette)} code_input
            --
            LocalizedText::plain("script_sandbox_input_preview") TextRole(Role::Caption)
            --
            @{multiline_editor_scene("", "script_sandbox_input_preview", 41, palette)} yaml_input
            --
            Node { width: percent(100), min_width: px(0), flex_direction: FlexDirection::Column, row_gap: rem(space::S6) }
            ScriptResultBody
        ]
    }
}
pub fn control(
    key: &'static str,
    action: ScriptControl,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! { @{localized_button_scene(LocalizedText::plain(key), ButtonVariant::Secondary, palette)} action }
}
pub fn spawn_review(mut commands: Commands, palette: Res<UiPalette>) {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    let status = ScriptReviewLine::Status;
    let details = ScriptReviewLine::Details;
    let content = ScriptReviewLine::Content;
    let path = ScriptReviewLine::Path;
    commands.spawn_scene(bsn! {
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), display: Display::None,
            align_items: AlignItems::Center, justify_content: JustifyContent::Center }
        ScriptExportOverlay GlobalZIndex(122)
        // The export review is a modal dialog on Bevy's official behaviour
        // layer: the barrier owns the click-outside close and the Escape
        // interception, the card owns the focus containment, and every close
        // request travels as the official `RequestClose`.
        //
        // The barrier is the review root — the only common ancestor of the
        // scrim and the card — because the official Escape interception
        // reaches the barrier through the focused entity and the official
        // `RequestClose` reaches it through the closing control, both of
        // which live under the card.
        ModalDialogBarrier
        on(|close: On<RequestClose>, cancels: Query<(Entity, &ScriptControl)>, mut commands: Commands| {
            // A close request that names the review's own cancel control is
            // that control's business (it already asked to close); every other
            // close request — the barrier's click-outside and the Escape
            // interception — runs the review's own close path.
            for (entity, control) in &cancels {
                if matches!(control, ScriptControl::Cancel) && entity != close.original_event_target() {
                    commands.trigger(Activate { entity });
                }
            }
        })
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim ScriptReviewControl ThemeBackgroundColor({Token::Scrim})
            --
            Node { width: percent(90), max_width: rem(SCRIPT_REVIEW_MODAL_MAX_WIDTH_PX), max_height: percent(85), padding: UiRect::all(rem(space::S16)), border_radius: BorderRadius::all(rem(palette.modal_radius_px))
                flex_direction: FlexDirection::Column, row_gap: rem(space::S12) }
            ModalDialogCard AccessibilityNode(semantic) LocalizedLabel::plain("script_export_title") ThemeBackgroundColor({Token::Surface})
            ModalDialog
            Children [
                LocalizedText::plain("script_export_title") TextRole(Role::Heading)
                --
                Node { width: percent(100), min_height: px(0), flex_shrink: 1.0, overflow: Overflow::scroll_y() }
                ScrollArea
                Children [
                    Node { width: percent(100), min_width: px(0), flex_direction: FlexDirection::Column, row_gap: rem(space::S8) }
                    Children [ Text(String::new()) status TextRole(Role::Body)
                        -- Text(String::new()) details TextRole(Role::Body)
                        -- Text(String::new()) content TextRole(Role::Mono)
                        -- Text(String::new()) path TextRole(Role::Body) ]
                ]
                --
                Node { flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S8), row_gap: rem(space::S8), flex_shrink: 0.0 }
                Children [
                    @{control("logs_export_cancel", ScriptControl::Cancel, &palette)} ScriptReviewControl
                    // The cancel control asks the official layer to close; the
                    // barrier observer runs the review's own close path.
                    on(|activate: On<Activate>, mut commands: Commands| {
                        commands.trigger(RequestClose { source: activate.entity });
                    })
                    -- @{control("logs_export_confirm", ScriptControl::Confirm, &palette)} ScriptReviewControl
                    -- @{control("logs_export_retry", ScriptControl::Retry, &palette)} ScriptReviewControl
                ]
            ]
        ]
    });
}
