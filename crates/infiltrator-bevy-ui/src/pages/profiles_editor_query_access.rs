//! Scoped native component access for profiles editor systems.

use super::{
    ProfileEditorDiagnosticPill, ProfileEditorDiagnosticText, ProfileEditorProtectionText,
    ProfileEditorProtectionToggleLabel, ProfileEditorStatusText,
};
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;

#[derive(QueryFilter)]
pub struct RestampProfileEditorStatusFilter {
    with_profile_editor_status_text: With<ProfileEditorStatusText>,
    without_profile_editor_protection_text: Without<ProfileEditorProtectionText>,
    without_profile_editor_diagnostic_text: Without<ProfileEditorDiagnosticText>,
    without_profile_editor_diagnostic_pill: Without<ProfileEditorDiagnosticPill>,
}

#[derive(QueryFilter)]
pub struct RestampProfileEditorDiagnosticsFilter {
    with_profile_editor_diagnostic_text: With<ProfileEditorDiagnosticText>,
    without_profile_editor_status_text: Without<ProfileEditorStatusText>,
    without_profile_editor_diagnostic_pill: Without<ProfileEditorDiagnosticPill>,
}

#[derive(QueryFilter)]
pub struct RestampProfileEditorPillsFilter {
    with_profile_editor_diagnostic_pill: With<ProfileEditorDiagnosticPill>,
    without_profile_editor_status_text: Without<ProfileEditorStatusText>,
    without_profile_editor_diagnostic_text: Without<ProfileEditorDiagnosticText>,
    without_profile_editor_protection_toggle_label: Without<ProfileEditorProtectionToggleLabel>,
}

#[derive(QueryFilter)]
pub struct RestampProfileEditorTogglesFilter {
    with_profile_editor_protection_toggle_label: With<ProfileEditorProtectionToggleLabel>,
    without_profile_editor_status_text: Without<ProfileEditorStatusText>,
    without_profile_editor_diagnostic_text: Without<ProfileEditorDiagnosticText>,
    without_profile_editor_diagnostic_pill: Without<ProfileEditorDiagnosticPill>,
}

#[derive(SystemParam)]
pub struct EditorVisualTargets<'w, 's> {
    pub status: Query<'w, 's, &'static mut Text, RestampProfileEditorStatusFilter>,
    pub diagnostics: Query<
        'w,
        's,
        (&'static mut Text, &'static mut TextColor),
        RestampProfileEditorDiagnosticsFilter,
    >,
    pub pills: Query<
        'w,
        's,
        (&'static mut Text, &'static mut BackgroundColor),
        RestampProfileEditorPillsFilter,
    >,
    pub toggles: Query<
        'w,
        's,
        (&'static mut Text, &'static mut BackgroundColor),
        RestampProfileEditorTogglesFilter,
    >,
}
