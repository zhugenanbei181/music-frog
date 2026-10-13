//! DUAL-10-08/10/11: the Bevy Mixin pane's shared-studio widgets.
//!
//! The preflight verdict comes from the engine-scoped options codec; the
//! preset-toggle chips and cascade pipeline strip remain shared projections.
//! This module renders those facts and routes toggle activations back into the
//! shared overlay buffer.

use crate::command::CommandSinkHandle;
use crate::pages::profiles_editor_panes::MixinStudioState;
use crate::pages::profiles_editor_panes::{MixinEditorBody, ProfileEditorOptionsState};
use crate::pages::profiles_editor_state::ProfileEditorState;
use crate::pages::profiles_editor_transactions::EditorMutationControl;
use crate::pages::profiles_mixin_copy::MixinCopyRole;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, FlexWrap, Node,
    Overflow, UiRect, Val, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::mixin_studio_projection;
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::profile_document_codec::{
    ProfileMixinColumn, ProfileMixinColumnRole, ProfileMixinComposition, ProfileMixinPreflight,
    ProfileMixinToggle, ProfileMixinToggleId, ProfileMixinToggleProjection,
};

/// One common-overlay toggle chip (index into the shared catalogue).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MixinToggleButton {
    pub id: ProfileMixinToggleId,
}

impl Default for MixinToggleButton {
    fn default() -> Self {
        // `bsn!` patches the explicit toggle ID into this template value.
        Self {
            id: ProfileMixinToggleId::Ipv6,
        }
    }
}

/// Container whose children are the shared-studio rows (rebuilt on change).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinStudioBody;

/// Restamped engine-scoped codec inspection verdict text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinPreflightText;

#[derive(Component, Clone, Default)]
pub struct MixinPreflightDetail;

#[derive(Component, Clone)]
pub struct MixinColumnCaption(pub ProfileMixinColumnRole);
impl Default for MixinColumnCaption {
    fn default() -> Self {
        Self(ProfileMixinColumnRole::Base)
    }
}

/// Restamped cascade pipeline strip text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinCascadeText;

/// DUAL-10-09: the composed column body (the real cascade output).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinComposedText;

/// DUAL-10-09: the real blocking reason shown when the composed column is empty.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinComposedErrorText;

/// The preflight banner, toggle chips and cascade strip for one overlay buffer.
pub fn mixin_studio_scene(
    options: &ProfileEditorOptionsState,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let studio = match &options.mixin_studio {
        MixinStudioState::Available(studio) => Some(studio),
        MixinStudioState::NotRequested | MixinStudioState::Unavailable(_) => None,
    };
    let unavailable_preflight = ProfileMixinPreflight::Blocked {
        reason: "Mixin codec is unavailable".into(),
    };
    let preflight = studio
        .map(|studio| studio.preflight.clone())
        .unwrap_or(unavailable_preflight);

    let locale = UiLocale::default();
    let preflight_pill = (
        LocalizedText::plain(mixin_studio_projection::preflight_key(&preflight)),
        if matches!(&preflight, ProfileMixinPreflight::Accepted) {
            palette.success
        } else {
            palette.danger
        },
    );
    let preflight_detail = mixin_studio_projection::preflight_detail(&preflight, locale.code());
    let preflight_label = preflight_pill.0.render(&locale);
    let toggle_states: Vec<ProfileMixinToggle> = match studio {
        Some(studio) => match &studio.toggles {
            ProfileMixinToggleProjection::Available(toggles) => toggles.clone(),
            ProfileMixinToggleProjection::Unavailable { .. } => ProfileMixinToggleId::ALL
                .into_iter()
                .map(|id| ProfileMixinToggle { id, enabled: false })
                .collect(),
        },
        None => ProfileMixinToggleId::ALL
            .into_iter()
            .map(|id| ProfileMixinToggle { id, enabled: false })
            .collect(),
    };
    let toggle_buttons: Vec<Box<dyn Scene>> = toggle_states
        .into_iter()
        .map(|toggle| {
            let enabled = toggle.enabled;
            let background = if enabled {
                palette.accent
            } else {
                palette.surface_elevated
            };
            let label = LocalizedText::plain(mixin_studio_projection::preset_key(toggle.id));
            Box::new(bsn! {
                            Node {
                                min_height: rem(22.0),
                                padding: UiRect::horizontal(rem(space::S8)),
                                align_items: AlignItems::Center,
                            }
                            BackgroundColor({ background })
                            Button
                            EditorMutationControl
                            MixinToggleButton { id: { toggle.id } }
                            Children [
                                label TextRole(Role::Caption)
                            ]
            }) as Box<dyn Scene>
        })
        .collect();

    let cascade_text = studio
        .map(|studio| mixin_studio_projection::cascade_caption(&studio.cascade, locale.code()))
        .unwrap_or_else(|| "Mixin codec is unavailable".into());
    let verdict_role = MixinCopyRole::Verdict;
    let detail_role = MixinCopyRole::Detail;
    let cascade_role = MixinCopyRole::Cascade;
    let columns = studio.map(|studio| three_column_scene(&studio.columns, palette));

    Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S4),
            }
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: rem(space::S4),
                    row_gap: rem(space::S4),
                }
                Children [
                    Node {
                        padding: UiRect::new(Val::Px(4.0), Val::Px(4.0), Val::Px(1.0), Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                    }
                    BackgroundColor({ preflight_pill.1 })
                    Children [
                        Text(preflight_label) MixinPreflightText verdict_role TextRole(Role::Caption)
                    ]
                    --
                    Text(preflight_detail) MixinPreflightDetail detail_role TextRole(Role::Mono)
                ]
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: rem(space::S4),
                    row_gap: rem(space::S4),
                }
                Children [
                    LocalizedText::plain("mixin_studio_toggles_title") TextRole(Role::Caption)
                    --
                    { toggle_buttons }
                ]
                --
                Text(cascade_text) MixinCascadeText cascade_role TextRole(Role::Mono)
                --
                { columns.map(|scene| vec![scene]).unwrap_or_default() }
            ]
    })
}

/// DUAL-10-09: the three-column workspace. The left column is the real base
/// document, the middle column is the editable overlay buffer (its rows are
/// restamped by `refresh_mixin_editor_body`), and the right column is the real
/// composed output of the shared cascade pipeline — or the real blocking
/// reason. Nothing here is a mock.
fn three_column_scene(
    columns: &infiltrator_ports::profile_document_codec::ProfileMixinColumns,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let base_caption =
        mixin_studio_projection::column_caption(&columns.base, UiLocale::default().code());
    let overlay_caption =
        mixin_studio_projection::column_caption(&columns.overlay, UiLocale::default().code());
    let composed_caption = match &columns.composed {
        ProfileMixinComposition::Produced(column) => {
            mixin_studio_projection::column_caption(column, UiLocale::default().code())
        }
        ProfileMixinComposition::Blocked { .. } => mixin_studio_projection::column_caption(
            &ProfileMixinColumn {
                role: ProfileMixinColumnRole::Composed,
                content: String::new(),
                editable: false,
                line_count: 0,
            },
            UiLocale::default().code(),
        ),
    };
    let base_text = columns.base.content.clone();
    let composed_text = match &columns.composed {
        ProfileMixinComposition::Produced(column) => column.content.clone(),
        ProfileMixinComposition::Blocked { .. } => String::new(),
    };
    let composed_note = mixin_studio_projection::composed_error(
        match &columns.composed {
            ProfileMixinComposition::Blocked { reason } => Some(reason.as_str()),
            ProfileMixinComposition::Produced(_) => None,
        },
        UiLocale::default().code(),
    );
    let composed_color = if matches!(&columns.composed, ProfileMixinComposition::Blocked { .. }) {
        palette.danger
    } else {
        palette.ink
    };
    let base_role = MixinCopyRole::Column(ProfileMixinColumnRole::Base);
    let overlay_role = MixinCopyRole::Column(ProfileMixinColumnRole::Overlay);
    let composed_role = MixinCopyRole::Column(ProfileMixinColumnRole::Composed);
    let error_role = MixinCopyRole::Error;
    let box_background = palette.window_clear;
    let border = palette.border;
    Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: rem(space::S8),
                align_items: AlignItems::Start,
            }
            Children [
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S2),
                }
                Children [
                    Text({ base_caption }) MixinColumnCaption(ProfileMixinColumnRole::Base) base_role TextRole(Role::Caption)
                    --
                    Node {
                        width: percent(100),
                        max_height: rem(220.0),
                        padding: UiRect::all(rem(space::S8)),
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        border: UiRect::all(Val::Px(palette.hairline_px)),
                        overflow: Overflow::scroll_y(),
                    }
                    ScrollArea
                    BackgroundColor({ box_background })
                    BorderColor { top: border, right: border, bottom: border, left: border }
                    Children [
                        Text(base_text) TextRole(Role::Mono)
                    ]
                ]
                --
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S2),
                }
                Children [
                    Text({ overlay_caption }) MixinColumnCaption(ProfileMixinColumnRole::Overlay) overlay_role TextRole(Role::Caption)
                    --
                    Node {
                        width: percent(100),
                        max_height: rem(220.0),
                        padding: UiRect::all(rem(space::S8)),
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        overflow: Overflow::scroll_y(),
                    }
                    ScrollArea
                    BackgroundColor({ box_background })
                    Children [
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                        }
                        MixinEditorBody
                    ]
                ]
                --
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S2),
                }
                Children [
                    Text({ composed_caption }) MixinColumnCaption(ProfileMixinColumnRole::Composed) composed_role TextRole(Role::Caption)
                    --
                    Node {
                        width: percent(100),
                        max_height: rem(220.0),
                        padding: UiRect::all(rem(space::S8)),
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        border: UiRect::all(Val::Px(palette.hairline_px)),
                        overflow: Overflow::scroll_y(),
                    }
                    ScrollArea
                    BackgroundColor({ box_background })
                    BorderColor { top: border, right: border, bottom: border, left: border }
                    Children [
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: rem(space::S2),
                        }
                        Children [
                            Text(composed_text) TextRole(Role::Mono) MixinComposedText
                            --
                            Text(composed_note) TextRole(Role::Mono) MixinComposedErrorText error_role TextColor({ composed_color })
                        ]
                    ]
                ]
            ]
    })
}

/// DUAL-10-11: flip one shared toggle in the overlay buffer.
pub fn on_mixin_toggle_activated(
    activate: On<Activate>,
    buttons: Query<&MixinToggleButton>,
    document: Res<ProfileEditorState>,
    handle: Option<Res<CommandSinkHandle>>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let toggle = button.id;
    if !options.mixin.session.can_edit() {
        return;
    }
    let Some(handle) = handle else {
        options.mixin.notice = Some("Profile options codec is unavailable".into());
        return;
    };
    let Some(source) = document.session.source() else {
        options.mixin.notice = Some("No profile document is open".into());
        return;
    };
    let text = options.mixin.buffer.full_text();
    let enabled = handle.inspect_mixin_toggle(&source.document, &text, toggle);
    let updated = enabled
        .and_then(|enabled| handle.set_mixin_toggle(&source.document, &text, toggle, !enabled));
    match updated {
        Ok(updated) => {
            options.mixin.buffer = CodeEditorState::new(&updated);
            options.mixin.dirty = true;
            options.mixin.generation = options.mixin.generation.wrapping_add(1);
            options.mixin.notice = None;
        }
        Err(failure) => {
            options.mixin.notice = Some(failure.message);
        }
    }
}

/// Rebuild the shared-studio rows when either the document or the overlay moved.
pub fn refresh_mixin_studio_body(
    mut options: ResMut<ProfileEditorOptionsState>,
    document: Res<ProfileEditorState>,
    handle: Option<Res<CommandSinkHandle>>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    bodies: Query<Entity, With<MixinStudioBody>>,
) {
    let key = (document.generation, options.mixin.generation);
    if options.studio_generation == key {
        return;
    }
    options.studio_generation = key;
    let base = document.buffer.full_text();
    let studio = match (handle.as_ref(), document.session.source()) {
        (Some(handle), Some(source)) => {
            handle.inspect_mixin_studio(&source.document, &base, &options.mixin.buffer.full_text())
        }
        _ => Err(Failure::new(
            ErrorCode::NotReady,
            "Profile options codec is unavailable",
            true,
        )),
    };
    options.mixin_studio = match studio {
        Ok(projection) => {
            options
                .mixin
                .record_mixin_preflight_status(&projection.preflight);
            MixinStudioState::Available(projection)
        }
        Err(failure) => {
            options.mixin.record_mixin_preflight(Err(failure.clone()));
            MixinStudioState::Unavailable(failure)
        }
    };
    for entity in &bodies {
        commands.entity(entity).despawn_children();
        let scene = mixin_studio_scene(&options, &palette);
        commands.spawn_scene(scene).insert(ChildOf(entity));
    }
    // DUAL-10-09: the rebuild just respawned the middle column's (empty)
    // editor body, so the editor rows must be restamped even when the overlay
    // buffer itself did not move. `refresh_mixin_editor_body` runs after this
    // system in the same frame and refills the fresh body.
    options.mixin.last_rendered = u64::MAX;
}
