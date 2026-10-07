//! In-place editor copy replay. No adoption, input mutation or subtree replacement.
use crate::pages::profiles::{LastProfilesProjection, ProfileProtectionText};
use crate::pages::profiles_editor::{ProfileEditorProtectionText, ProfileEditorTitle};
use crate::pages::profiles_editor_panes::ProfileEditorOptionsState;
use crate::pages::profiles_editor_panes_sync::{
    MixinDiagnosticFilter, MixinPillFilter, MixinStatusFilter,
};
use crate::pages::profiles_editor_state::{ProfileEditorState, diagnostic_line, status_line};
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res, SystemParam};
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;
use infiltrator_application::profile_editor_projection;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;

pub fn replay_document_copy(
    state: Res<ProfileEditorState>,
    last: Option<Res<LastProfilesProjection>>,
    locale: Res<UiLocale>,
    mut titles: Query<
        &mut Text,
        (
            With<ProfileEditorTitle>,
            Without<ProfileEditorProtectionText>,
        ),
    >,
    mut protections: Query<
        &mut Text,
        (
            With<ProfileEditorProtectionText>,
            Without<ProfileEditorTitle>,
        ),
    >,
) {
    let title = profile_editor_projection::title(state.buffer.line_count(), locale.code());
    for mut text in &mut titles {
        if text.0 != title {
            text.0.clone_from(&title);
        }
    }
    let protection = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.profile_document.as_ref())
        .map(|document| document.write_protection)
        .unwrap_or_default();
    let notice = state
        .format_note
        .map(|reason| profile_editor_projection::format_note(reason, locale.code()));
    let banner = profile_editor_projection::protection_banner(
        protection,
        state.notice.as_deref().or(notice.as_deref()),
        locale.code(),
    );
    for mut text in &mut protections {
        if text.0 != banner {
            text.0.clone_from(&banner);
        }
    }
}

#[derive(SystemParam)]
pub struct MixinCopyTargets<'w, 's> {
    status: Query<'w, 's, &'static mut Text, MixinStatusFilter>,
    diagnostics: Query<'w, 's, (&'static mut Text, &'static mut TextColor), MixinDiagnosticFilter>,
    pills: Query<'w, 's, (&'static mut Text, &'static mut BackgroundColor), MixinPillFilter>,
}
pub fn replay_mixin_copy(
    options: Res<ProfileEditorOptionsState>,
    last: Option<Res<LastProfilesProjection>>,
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    targets: MixinCopyTargets,
) {
    let MixinCopyTargets {
        mut status,
        mut diagnostics,
        mut pills,
    } = targets;
    let projection = last.as_ref().and_then(|last| last.0.as_ref());
    let label = status_line(&options.mixin, projection, locale.code());
    for mut text in &mut status {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
    let (label, has_error) = diagnostic_line(&options.mixin, locale.code());
    let color = if has_error {
        palette.danger
    } else {
        palette.success
    };
    for (mut text, mut foreground) in &mut diagnostics {
        if text.0 != label {
            text.0.clone_from(&label);
        }
        if foreground.0 != color {
            foreground.0 = color;
        }
    }
    let label = profile_editor_projection::syntax_label(has_error, locale.code());
    for (mut text, mut background) in &mut pills {
        if text.0 != label {
            text.0.clone_from(&label);
        }
        if background.0 != color {
            background.0 = color;
        }
    }
}

/// Profile-list protection badges have their own scope; the Diff inspector doesn't own them.
pub fn replay_profile_protection(
    last: Res<LastProfilesProjection>,
    locale: Res<UiLocale>,
    mut badges: Query<(&ProfileProtectionText, &mut Text)>,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    for (marker, mut text) in &mut badges {
        let Some(profile) = projection.profiles.get(marker.0) else {
            continue;
        };
        let label =
            profile_editor_projection::protection_label(profile.write_protection, locale.code());
        if text.0 != label {
            text.0 = label;
        }
    }
}
