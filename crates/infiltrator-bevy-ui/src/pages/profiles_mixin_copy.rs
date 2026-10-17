//! Locale replay of the Mixin studio updates captions without replacing document entities.
use crate::pages::profiles_editor_panes::{MixinStudioState, ProfileEditorOptionsState};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::profile_mixin_studio::{ProfileMixinColumnRole, ProfileMixinComposition};
use infiltrator_presentation::mixin_studio_projection;
use infiltrator_shared::locales::Lang;

#[derive(Component, Clone, Default)]
pub enum MixinCopyRole {
    #[default]
    Cascade,
    Column(ProfileMixinColumnRole),
    Error,
    Detail,
    Verdict,
}
pub fn replay_studio_copy(
    options: Res<ProfileEditorOptionsState>,
    locale: Res<UiLocale>,
    mut labels: Query<(&MixinCopyRole, &mut Text)>,
) {
    if labels.is_empty()
        || (!options.is_changed()
            && !locale.is_changed()
            && !labels.iter_mut().any(|(_, text)| text.is_added()))
    {
        return;
    }
    for (role, mut text) in &mut labels {
        let copy = match role {
            MixinCopyRole::Cascade => match &options.mixin_studio {
                MixinStudioState::Available(studio) => {
                    mixin_studio_projection::cascade_caption(&studio.cascade, &Lang(locale.code()))
                }
                MixinStudioState::NotRequested | MixinStudioState::Unavailable(_) => {
                    locale.text("editor_document_inspection_unavailable")
                }
            },
            MixinCopyRole::Column(role) => {
                let column = match (&options.mixin_studio, role) {
                    (MixinStudioState::Available(studio), ProfileMixinColumnRole::Base) => {
                        Some(&studio.columns.base)
                    }
                    (MixinStudioState::Available(studio), ProfileMixinColumnRole::Overlay) => {
                        Some(&studio.columns.overlay)
                    }
                    (MixinStudioState::Available(studio), ProfileMixinColumnRole::Composed) => {
                        match &studio.columns.composed {
                            ProfileMixinComposition::Produced(column) => Some(column),
                            ProfileMixinComposition::Blocked { .. } => None,
                        }
                    }
                    _ => None,
                };
                match column {
                    Some(column) => {
                        mixin_studio_projection::column_caption(column, &Lang(locale.code()))
                    }
                    None => mixin_studio_projection::column_caption_parts(
                        *role,
                        0,
                        false,
                        &Lang(locale.code()),
                    ),
                }
            }
            MixinCopyRole::Error => match &options.mixin_studio {
                MixinStudioState::Available(studio) => match &studio.columns.composed {
                    ProfileMixinComposition::Blocked { reason } => {
                        mixin_studio_projection::composed_error(Some(reason), &Lang(locale.code()))
                    }
                    ProfileMixinComposition::Produced(_) => String::new(),
                },
                MixinStudioState::NotRequested | MixinStudioState::Unavailable(_) => {
                    locale.text("editor_document_inspection_unavailable")
                }
            },
            MixinCopyRole::Detail => match &options.mixin_studio {
                MixinStudioState::Available(studio) => mixin_studio_projection::preflight_detail(
                    &studio.preflight,
                    &Lang(locale.code()),
                ),
                MixinStudioState::NotRequested | MixinStudioState::Unavailable(_) => {
                    locale.text("editor_document_inspection_unavailable")
                }
            },
            MixinCopyRole::Verdict => match &options.mixin_studio {
                MixinStudioState::Available(studio) => {
                    locale.text(mixin_studio_projection::preflight_key(&studio.preflight))
                }
                MixinStudioState::NotRequested | MixinStudioState::Unavailable(_) => {
                    locale.text("mixin_studio_preflight_blocked")
                }
            },
        };
        if text.0 != copy {
            text.0 = copy;
        }
    }
}
