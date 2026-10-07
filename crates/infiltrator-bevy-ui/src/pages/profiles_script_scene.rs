//! Native multiline workbench and an independent export review layer.
use crate::localized_widgets::localized_button_scene;
use crate::pages::profiles_script_workbench::{ScriptControl, ScriptInput};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, FlexWrap, GlobalZIndex, JustifyContent,
    Node, Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::multiline_editor::multiline_editor_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_contract::script_export::ScriptExportKind;
use infiltrator_contract::script_run::ScriptEditorField;

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
        Node { width: percent(100), min_width: px(0), flex_direction: FlexDirection::Column, row_gap: px(12), padding: UiRect::all(px(16)) }
        BackgroundColor({palette.surface}) ScriptWorkbenchCard
        Children [
            LocalizedText::plain("script_sandbox_title") TextRole(Role::Heading)
            -- LocalizedText::plain("script_sandbox_presets") TextRole(Role::Caption)
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: px(8), row_gap: px(8) }
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
            Node { width: percent(100), min_width: px(0), flex_direction: FlexDirection::Column, row_gap: px(6) }
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
    let cancel = ScriptControl::Cancel;
    let status = ScriptReviewLine::Status;
    let details = ScriptReviewLine::Details;
    let content = ScriptReviewLine::Content;
    let path = ScriptReviewLine::Path;
    commands.spawn_scene(bsn! {
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), display: Display::None,
            align_items: AlignItems::Center, justify_content: JustifyContent::Center }
        ScriptExportOverlay GlobalZIndex(122)
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim cancel ScriptReviewControl BackgroundColor({palette.scrim})
            --
            Node { width: percent(90), max_width: px(560), max_height: percent(85), padding: UiRect::all(px(16)),
                flex_direction: FlexDirection::Column, row_gap: px(12) }
            ModalDialogCard AccessibilityNode(semantic) LocalizedLabel::plain("script_export_title") BackgroundColor({palette.surface})
            Children [
                LocalizedText::plain("script_export_title") TextRole(Role::Heading)
                --
                Node { width: percent(100), min_height: px(0), flex_shrink: 1.0, overflow: Overflow::scroll_y() }
                ScrollArea
                Children [
                    Node { width: percent(100), min_width: px(0), flex_direction: FlexDirection::Column, row_gap: px(8) }
                    Children [ Text(String::new()) status TextRole(Role::Body)
                        -- Text(String::new()) details TextRole(Role::Body)
                        -- Text(String::new()) content TextRole(Role::Mono)
                        -- Text(String::new()) path TextRole(Role::Body) ]
                ]
                --
                Node { flex_wrap: FlexWrap::Wrap, column_gap: px(8), row_gap: px(8), flex_shrink: 0.0 }
                Children [
                    @{control("logs_export_cancel", ScriptControl::Cancel, &palette)} ScriptReviewControl
                    -- @{control("logs_export_confirm", ScriptControl::Confirm, &palette)} ScriptReviewControl
                    -- @{control("logs_export_retry", ScriptControl::Retry, &palette)} ScriptReviewControl
                ]
            ]
        ]
    });
}
