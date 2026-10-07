//! SDK multiline inputs own IME; the shell observes their actual host values.
use crate::ime::ImeHostReport;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input_focus::InputFocus;
use bevy::text::EditableText;
use bevy::ui::{ComputedUiRenderTargetInfo, UiScale};
use bevy::window::{PrimaryWindow, Window};
use infiltrator_bevy_widgets::multiline_editor::MultilineEditor;
use infiltrator_contract::ime::{ImeCursorRect, ImeCursorSource, ImeCursorSupport, ImeFocusPlan};

pub fn native_editor_active(
    focus: &InputFocus,
    fields: &Query<&EditableText, With<MultilineEditor>>,
) -> bool {
    focus.get().is_some_and(|entity| fields.contains(entity))
}
#[derive(QueryData)]
pub struct NativeCaret {
    editable: &'static EditableText,
    target: Option<&'static ComputedUiRenderTargetInfo>,
}
pub fn report_native_ime(
    focus: Res<InputFocus>,
    ui_scale: Res<UiScale>,
    fields: Query<NativeCaret, With<MultilineEditor>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut report: ResMut<ImeHostReport>,
) {
    let Some(entity) = focus.get() else {
        return;
    };
    let Ok(caret) = fields.get(entity) else {
        return;
    };
    let Ok((window_entity, window)) = windows.single() else {
        return;
    };
    let area = caret.editable.editor().ime_cursor_area();
    let scale = caret
        .target
        .map(|target| target.scale_factor())
        .unwrap_or(1.0);
    let width = (area.x1 - area.x0) as f32 * ui_scale.0 / scale;
    let height = (area.y1 - area.y0) as f32 * ui_scale.0 / scale;
    let cursor = (window.ime_enabled
        && width.is_finite()
        && height.is_finite()
        && width > 0.0
        && height > 0.0)
        .then_some(ImeCursorRect::new(
            window.ime_position.x,
            window.ime_position.y,
            width,
            height,
        ));
    report.support = ImeCursorSupport::Hosted {
        source: ImeCursorSource::ToolkitProvided,
    };
    report.plan = ImeFocusPlan {
        enabled: window.ime_enabled,
        cursor,
    };
    report.window = Some(window_entity);
    report.focused_field = Some(entity);
}
