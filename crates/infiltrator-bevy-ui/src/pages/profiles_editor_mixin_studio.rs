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
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, Node, UiRect, Val, percent,
    px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::editor::CodeEditorState;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_domain::mixin_studio;

use crate::pages::profiles_editor_panes::ProfileEditorOptionsState;
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
        ]
    })
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
}
