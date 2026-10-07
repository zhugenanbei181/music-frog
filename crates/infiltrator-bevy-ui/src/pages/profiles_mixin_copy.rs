//! Locale replay of the Mixin studio updates captions without replacing document entities.
use crate::pages::profiles_editor_panes::ProfileEditorOptionsState;
use crate::pages::profiles_editor_state::ProfileEditorState;
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use infiltrator_application::mixin_studio_projection;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_domain::mixin_studio::{
    MixinColumnRole, mixin_editor_columns, preflight_mixin, preview_cascade_from_yaml,
};

#[derive(Component, Clone, Default)]
pub enum MixinCopyRole {
    #[default]
    Cascade,
    Column(MixinColumnRole),
    Error,
    Detail,
    Verdict,
}
pub fn replay_studio_copy(
    options: Res<ProfileEditorOptionsState>,
    document: Res<ProfileEditorState>,
    locale: Res<UiLocale>,
    mut labels: Query<(&MixinCopyRole, &mut Text)>,
) {
    if labels.is_empty()
        || (!options.is_changed()
            && !document.is_changed()
            && !locale.is_changed()
            && !labels.iter_mut().any(|(_, text)| text.is_added()))
    {
        return;
    }
    let base = document.buffer.full_text();
    let overlay = options.mixin.buffer.full_text();
    let columns = mixin_editor_columns(&base, &overlay);
    let report = preview_cascade_from_yaml(&base, &overlay);
    let preflight = preflight_mixin(&base, &overlay);
    for (role, mut text) in &mut labels {
        let copy = match role {
            MixinCopyRole::Cascade => {
                mixin_studio_projection::cascade_caption(&report, locale.code())
            }
            MixinCopyRole::Column(role) => {
                let column = match role {
                    MixinColumnRole::Base => &columns.base,
                    MixinColumnRole::Overlay => &columns.overlay,
                    MixinColumnRole::Composed => &columns.composed,
                };
                mixin_studio_projection::column_caption(column, locale.code())
            }
            MixinCopyRole::Error => {
                mixin_studio_projection::composed_error(columns.error.as_deref(), locale.code())
            }
            MixinCopyRole::Detail => {
                mixin_studio_projection::preflight_detail(&preflight, locale.code())
            }
            MixinCopyRole::Verdict => {
                locale.text(mixin_studio_projection::preflight_key(&preflight))
            }
        };
        if text.0 != copy {
            text.0 = copy;
        }
    }
}
