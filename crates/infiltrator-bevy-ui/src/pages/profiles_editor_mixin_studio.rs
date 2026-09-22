//! DUAL-10-08/10/11: the Bevy Mixin pane's shared-studio widgets.
//!
//! The preflight verdict, the preset-toggle chips and the cascade pipeline
//! strip all call the shared pure functions in
//! `infiltrator_domain::mixin_studio`; this module only renders the verdicts
//! and routes toggle activations back into the shared overlay buffer.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, FlexWrap, Node, UiRect,
    Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::editor::CodeEditorState;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_domain::mixin_studio;

use crate::pages::profiles_editor_panes::{MixinEditorBody, ProfileEditorOptionsState};
use crate::pages::profiles_editor_state::ProfileEditorState;

/// One common-overlay toggle chip (index into the shared catalogue).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinToggleButton {
    pub index: usize,
}

/// Container whose children are the shared-studio rows (rebuilt on change).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinStudioBody;

/// Restamped shared preflight verdict text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinPreflightText;

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
    base_content: &str,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let mixin_text = options.mixin.buffer.full_text();
    let report = mixin_studio::preflight_mixin(base_content, &mixin_text);

    let preflight_pill = if report.is_blocking() {
        ("预检阻断", palette.danger)
    } else {
        ("预检通过", palette.success)
    };
    let preflight_detail = if report.is_blocking() {
        report.error.clone().unwrap_or_default()
    } else {
        "语法、合并与最终配置校验均已通过".to_owned()
    };

    let toggle_buttons: Vec<Box<dyn Scene>> = mixin_studio::MIXIN_PRESET_TOGGLES
        .iter()
        .enumerate()
        .map(|(index, toggle)| {
            let enabled = mixin_studio::toggle_enabled(&mixin_text, toggle.id).unwrap_or(false);
            let background = if enabled {
                palette.accent
            } else {
                palette.surface_elevated
            };
            let label = toggle.label_zh.to_owned();
            Box::new(bsn! {
                Node {
                    min_height: px(22.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                }
                BackgroundColor({ background })
                Button
                template_value(MixinToggleButton { index })
                Children [
                    ( Text({ label }) TextRole(Role::Caption) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    let cascade_text = cascade_line(&mixin_text, base_content);
    let columns = three_column_scene(&mixin_text, base_content, palette);

    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        Children [
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(space::S4),
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    (
                        Node {
                            padding: UiRect::new(Val::Px(4.0), Val::Px(4.0), Val::Px(1.0), Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ preflight_pill.1 })
                        Children [
                            ( Text({ preflight_pill.0.to_owned() }) MixinPreflightText TextRole(Role::Caption) ),
                        ]
                    ),
                    ( Text(preflight_detail) TextRole(Role::Mono) ),
                ]
            ),
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(space::S4),
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    ( Text({ "常用覆写开关".to_owned() }) TextRole(Role::Caption) ),
                    { toggle_buttons },
                ]
            ),
            ( Text(cascade_text) MixinCascadeText TextRole(Role::Mono) ),
            { vec![columns] },
        ]
    })
}

/// DUAL-10-09: the three-column workspace. The left column is the real base
/// document, the middle column is the editable overlay buffer (its rows are
/// restamped by `refresh_mixin_editor_body`), and the right column is the real
/// composed output of the shared cascade pipeline — or the real blocking
/// reason. Nothing here is a mock.
fn three_column_scene(mixin_text: &str, base_content: &str, palette: &UiPalette) -> Box<dyn Scene> {
    let columns = mixin_studio::mixin_editor_columns(base_content, mixin_text);
    let base_caption = column_caption_zh(columns.base.label_zh, &columns.base, false);
    let overlay_caption = column_caption_zh(columns.overlay.label_zh, &columns.overlay, true);
    let composed_caption = column_caption_zh(columns.composed.label_zh, &columns.composed, false);
    let base_text = columns.base.content.clone();
    let composed_text = columns.composed.content.clone();
    let composed_note = match columns.error.clone() {
        Some(error) => format!("合成被阻断：{error}"),
        None => String::new(),
    };
    let composed_color = if columns.is_blocked() {
        palette.danger
    } else {
        palette.ink
    };
    let box_background = palette.window_clear;
    let border = palette.border;
    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(space::S8),
            align_items: AlignItems::Start,
        }
        Children [
            (
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S2),
                }
                Children [
                    ( Text({ base_caption }) TextRole(Role::Caption) ),
                    (
                        Node {
                            width: percent(100),
                            max_height: px(220.0),
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            overflow: bevy::ui::prelude::Overflow::scroll_y(),
                        }
                        BackgroundColor({ box_background })
                        BorderColor { top: border, right: border, bottom: border, left: border }
                        Children [
                            ( Text(base_text) TextRole(Role::Mono) ),
                        ]
                    ),
                ]
            ),
            (
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S2),
                }
                Children [
                    ( Text({ overlay_caption }) TextRole(Role::Caption) ),
                    (
                        Node {
                            width: percent(100),
                            max_height: px(220.0),
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            overflow: bevy::ui::prelude::Overflow::scroll_y(),
                        }
                        BackgroundColor({ box_background })
                        Children [
                            (
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Column,
                                }
                                MixinEditorBody
                            ),
                        ]
                    ),
                ]
            ),
            (
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S2),
                }
                Children [
                    ( Text({ composed_caption }) TextRole(Role::Caption) ),
                    (
                        Node {
                            width: percent(100),
                            max_height: px(220.0),
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            overflow: bevy::ui::prelude::Overflow::scroll_y(),
                        }
                        BackgroundColor({ box_background })
                        BorderColor { top: border, right: border, bottom: border, left: border }
                        Children [
                            (
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(space::S2),
                                }
                                Children [
                                    ( Text(composed_text) TextRole(Role::Mono) MixinComposedText ),
                                    ( Text(composed_note) TextRole(Role::Mono) MixinComposedErrorText bevy::text::TextColor({ composed_color }) ),
                                ]
                            ),
                        ]
                    ),
                ]
            ),
        ]
    })
}

/// `标题 · N 行 · 只读/可编辑` caption for one column.
pub fn column_caption_zh(
    label: &str,
    column: &mixin_studio::MixinColumn,
    editable: bool,
) -> String {
    format!(
        "{} · {} 行 · {}",
        label,
        column.line_count,
        if editable { "可编辑" } else { "只读" }
    )
}

/// The cascade pipeline line, computed by the real shared pipeline preview.
pub fn cascade_line(mixin_text: &str, base_content: &str) -> String {
    let report = mixin_studio::preview_cascade_from_yaml(base_content, mixin_text);
    if report.blocked {
        return format!("覆写流水线：阻断 · {}", report.error.unwrap_or_default());
    }
    let stages: Vec<String> = report
        .stages
        .iter()
        .map(|stage| {
            if stage.applied {
                format!("{} {} 行", stage.label_zh, stage.line_count)
            } else {
                format!("{} (未声明)", stage.label_zh)
            }
        })
        .collect();
    format!(
        "覆写流水线：{} · 合成输出 {} 行",
        stages.join(" → "),
        report.merged_line_count()
    )
}

/// DUAL-10-11: flip one shared toggle in the overlay buffer.
pub fn on_mixin_toggle_activated(
    activate: On<Activate>,
    buttons: Query<&MixinToggleButton>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(toggle) = mixin_studio::MIXIN_PRESET_TOGGLES.get(button.index) else {
        return;
    };
    let text = options.mixin.buffer.full_text();
    let enabled = mixin_studio::toggle_enabled(&text, toggle.id).unwrap_or(false);
    match mixin_studio::set_toggle(&text, toggle.id, !enabled) {
        Ok(updated) => {
            options.mixin.buffer = CodeEditorState::new(&updated);
            options.mixin.dirty = true;
            options.mixin.generation = options.mixin.generation.wrapping_add(1);
            options.mixin.refresh_preflight();
            options.mixin.notice = None;
        }
        Err(error) => {
            options.mixin.notice = Some(error);
        }
    }
}

/// Rebuild the shared-studio rows when either the document or the overlay moved.
pub fn refresh_mixin_studio_body(
    mut options: ResMut<ProfileEditorOptionsState>,
    document: Res<ProfileEditorState>,
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
    for entity in &bodies {
        commands.entity(entity).despawn_children();
        let scene = mixin_studio_scene(&options, &base, &palette);
        commands.spawn_scene(scene).insert(ChildOf(entity));
    }
    // DUAL-10-09: the rebuild just respawned the middle column's (empty)
    // editor body, so the editor rows must be restamped even when the overlay
    // buffer itself did not move. `refresh_mixin_editor_body` runs after this
    // system in the same frame and refills the fresh body.
    options.mixin.last_rendered = u64::MAX;
}
