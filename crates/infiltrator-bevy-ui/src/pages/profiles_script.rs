//! DUAL-10-05/14: the shared directive-DSL script-sandbox console card.
//!
//! The card is a *reader* of the one shared read model: Iced runs
//! `ScriptApplication`, and the surface reader republishes
//! `SurfaceSnapshot.script_sandbox` so this console renders exactly the same
//! projection. Nothing is executed here, and the panel says the engine is a
//! directive DSL, not JavaScript.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportOutcome, ScriptExportSnapshot,
};
use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;

use crate::pages::profiles::{ProfilesProjection, ProfilesProjectionUpdated};

/// Marker for script sandbox root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptSandboxRoot;

/// Container whose children are the shared console rows (rebuilt on update).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptSandboxBody;

/// Last projection rendered into [`ScriptSandboxBody`].
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ScriptSandboxViewState {
    pub rendered: Option<ScriptSandboxSnapshot>,
    /// DUAL-10-12: the export projection rendered next to the console rows.
    pub rendered_export: Option<ScriptExportSnapshot>,
}

fn text_row(body: String) -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            width: percent(100),
        }
        Children [
            ( Text({ body }) TextRole(Role::Mono) ),
        ]
    }) as Box<dyn Scene>
}

fn one_line(yaml: &str) -> String {
    yaml.trim().replace('\n', " ⏎ ")
}

/// The console rows for one shared projection (or the honest waiting state).
pub fn script_sandbox_body(
    snapshot: Option<&ScriptSandboxSnapshot>,
    export: Option<&ScriptExportSnapshot>,
) -> Vec<Box<dyn Scene>> {
    let mut rows: Vec<Box<dyn Scene>> = Vec::new();

    // DUAL-10-04: the preset catalogue is the shared one, never inline copy.
    let presets =
        infiltrator_application::script_application::ScriptApplication::new().builtin_presets();
    let preset_names: Vec<String> = presets.iter().map(|preset| preset.name.clone()).collect();
    rows.push(text_row(format!(
        "共享预设目录（{} 项）: {}",
        preset_names.len(),
        preset_names.join(" · ")
    )));

    match snapshot {
        None => rows.push(text_row(
            "等待脚本投影：在 Iced 控制台运行后，同一共享读模型会发布到此控制台".to_owned(),
        )),
        Some(snapshot) => {
            rows.push(text_row(format!(
                "执行引擎: {} · 引擎能力: {} · 生命周期阶段: {} ({})",
                snapshot.engine_label_zh(),
                snapshot.engine_capability_label_zh(),
                snapshot.hook_stage_label,
                snapshot.hook_stage
            )));
            rows.push(text_row(format!(
                "资源限额: 内存上限 {:.0}MB · 超时上限 {}ms · 实测 {}ms / {} 字节",
                snapshot.max_memory_limit_bytes as f64 / (1024.0 * 1024.0),
                snapshot.timeout_limit_ms,
                snapshot.execution_time_ms,
                snapshot.memory_used_bytes
            )));
            rows.push(text_row(format!(
                "熔断状态: {} · 连续失败 {}/{} · 冷却 {}ms / 剩余 {}ms",
                snapshot.circuit_breaker.label_zh(),
                snapshot.circuit_breaker.consecutive_failures,
                snapshot.circuit_breaker.failure_threshold,
                snapshot.circuit_breaker.cooldown_ms,
                snapshot.circuit_breaker.remaining_cooldown_ms
            )));
            if let Some(error) = snapshot.error_detail.as_deref() {
                rows.push(text_row(format!("执行失败: {error}")));
                rows.push(text_row(
                    "安全降级：原配置保持不变，内核继续运行".to_owned(),
                ));
            }
            if snapshot.matched_directives.is_empty() {
                rows.push(text_row("无指令匹配（未执行任何变换）".to_owned()));
            } else {
                for directive in &snapshot.matched_directives {
                    rows.push(text_row(format!(
                        "已执行指令 {} · {}（影响 {} 项）",
                        directive.id, directive.label, directive.affected
                    )));
                }
            }
            rows.push(text_row(format!(
                "console.log 捕获: {} 条",
                snapshot.log_count()
            )));
            for entry in &snapshot.console_logs {
                rows.push(text_row(format!(
                    "[{}ms] {}",
                    entry.timestamp_ms, entry.message
                )));
            }
            rows.push(text_row(format!(
                "输入 YAML: {}",
                one_line(&snapshot.input_yaml)
            )));
            rows.push(text_row(format!(
                "变换后 YAML: {}",
                one_line(snapshot.transformed_yaml.as_deref().unwrap_or("（无）"))
            )));
        }
    }
    rows.extend(export_rows(export));
    rows
}

/// DUAL-10-12: the export rows. The export is *executed* by the Iced console
/// through the shared application and the host save-file port; this card
/// renders the same shared projection (file name, bytes, SHA-256, typed host
/// outcome) and states where the action comes from.
fn export_rows(export: Option<&ScriptExportSnapshot>) -> Vec<Box<dyn Scene>> {
    let mut rows: Vec<Box<dyn Scene>> = Vec::new();
    let kinds: Vec<&str> = [
        ScriptExportKind::MixinOverlayYaml,
        ScriptExportKind::DirectiveDslScript,
        ScriptExportKind::ExtensionPackageJson,
    ]
    .iter()
    .map(|kind| kind.label_zh())
    .collect();
    rows.push(text_row(format!(
        "导出格式（共享用例，{} 种）: {}",
        kinds.len(),
        kinds.join(" · ")
    )));
    match export {
        None => rows.push(text_row(
            "尚未导出：在 Iced 控制台触发导出后，同一共享读模型（含真实文件名/字节数/宿主结果）会发布到此卡片"
                .to_owned(),
        )),
        Some(export) => {
            rows.push(text_row(format!(
                "导出文件: {}（{} 字节，{}）",
                export.file_name,
                export.byte_len(),
                export.media_type
            )));
            rows.push(text_row(format!(
                "宿主结果: {}{}",
                export.outcome_label_zh(),
                match &export.outcome {
                    ScriptExportOutcome::Saved { path, .. } => format!(" · 已写入 {path}"),
                    ScriptExportOutcome::Unsupported { reason }
                    | ScriptExportOutcome::Failed { reason } => format!(" · {reason}"),
                    ScriptExportOutcome::Prepared => String::new(),
                }
            )));
            if let Some(checksum) = export.checksum.as_deref() {
                rows.push(text_row(format!("SHA-256: {checksum}")));
            }
            rows.push(text_row(format!("诚实说明: {}", export.honest_note)));
            rows.push(text_row(format!(
                "导出内容预览: {}",
                one_line(&export.content_preview(400))
            )));
        }
    }
    rows
}

/// Script Sandbox scene.
pub fn script_sandbox_scene(
    projection: &ProfilesProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let rows = script_sandbox_body(
        projection.script_sandbox.as_ref(),
        projection.script_export.as_ref(),
    );
    let engine_note = projection
        .script_sandbox
        .as_ref()
        .map(|snapshot| snapshot.engine_label_zh().to_owned())
        .unwrap_or_else(|| "尚无投影 · 指令 DSL（非 JavaScript 引擎）".to_owned());

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: bevy::ui::prelude::JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                ScriptSandboxRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Settings, 24.0, palette) } ),
                            ( Text({ "脚本指令 DSL 控制台 (Script Sandbox)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    (
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.window_clear })
                        Children [
                            ( Text({ engine_note }) TextRole(Role::Caption) ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::all(Val::Px(space::S8)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.window_clear })
                ScriptSandboxBody
                Children [
                    { rows },
                ]
            }),
        ],
        palette,
    )
}

/// Rebuild the console rows when the shared projection moves.
pub fn rebuild_script_sandbox_body(
    update: On<ProfilesProjectionUpdated>,
    mut state: ResMut<ScriptSandboxViewState>,
    bodies: Query<Entity, With<ScriptSandboxBody>>,
    mut commands: Commands,
) {
    let snapshot = update.0.script_sandbox.clone();
    let export = update.0.script_export.clone();
    if state.rendered == snapshot && state.rendered_export == export {
        return;
    }
    state.rendered = snapshot.clone();
    state.rendered_export = export.clone();
    for entity in &bodies {
        commands.entity(entity).despawn_children();
        for row in script_sandbox_body(snapshot.as_ref(), export.as_ref()) {
            commands.spawn_scene(row).insert(ChildOf(entity));
        }
    }
}
