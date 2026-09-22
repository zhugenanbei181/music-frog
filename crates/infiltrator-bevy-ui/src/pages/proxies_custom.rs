//! Custom Node Editor & Universal URI Codec scene component (自定义节点与 URI 编解码).
//!
//! DUAL-05: the card is a projection of the shared
//! `ProtocolStudioSnapshot` published by
//! `infiltrator_application::protocol_codec_application`. The chip row carries
//! the typed Shadowsocks cipher family (05-01), the VLESS REALITY/Vision
//! block (05-02) and the multiplexing parameters (05-11); the footer carries
//! the codec audit and the URI fidelity gaps the application measured.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::prelude::*;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::proxies::{LastProxiesProjection, ProxiesProjectionUpdated};

/// Marker for custom node editor card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeEditorRoot;

/// Marker for import URI button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportUriButton;

/// Marker for save custom node button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveCustomNodeButton;

/// DUAL-05: the draft URI input field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeUriField;

/// DUAL-05: read-only text slots re-covered from the shared studio snapshot.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeText(pub CustomNodeSlot);

/// Which shared fact a text node renders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CustomNodeSlot {
    /// Protocol family + every typed chip (cipher / REALITY / smux / params).
    #[default]
    Chips,
    /// Validation issues from the shared draft report.
    Issues,
    /// Codec audit line (`node_count`, unknown keys, lossless verdict).
    Audit,
    /// The canonical share link exported from the draft.
    UriPreview,
    /// Fields a share link cannot carry (measured by the application).
    Gaps,
    /// DUAL-05: non-blocking facts the pinned core ignores or falls back on.
    Notes,
}

/// Text slots in render order.
const SLOTS: [(CustomNodeSlot, &str); 6] = [
    (CustomNodeSlot::Chips, "协议事实"),
    (CustomNodeSlot::Issues, "协议校验"),
    (CustomNodeSlot::Notes, "协议提示"),
    (CustomNodeSlot::Audit, "编解码审计"),
    (CustomNodeSlot::UriPreview, "分享链接"),
    (CustomNodeSlot::Gaps, "URI 损失字段"),
];

fn slot_initial(slot: CustomNodeSlot, studio: &ProtocolStudioSnapshot) -> String {
    let chips = studio
        .report
        .as_ref()
        .map(|report| report.all_chips().join(" · "))
        .unwrap_or_else(|| "尚无节点草稿".to_owned());
    match slot {
        CustomNodeSlot::Chips => chips,
        CustomNodeSlot::Issues => {
            let issues = studio.issue_lines();
            if issues.is_empty() {
                "协议校验通过".to_owned()
            } else {
                issues.join("；")
            }
        }
        CustomNodeSlot::Notes => {
            let notes: Vec<String> = studio
                .report
                .as_ref()
                .map(|report| report.params.notes.clone())
                .unwrap_or_default();
            if notes.is_empty() {
                "无跨版本提示".to_owned()
            } else {
                notes.join("；")
            }
        }
        CustomNodeSlot::Audit => match &studio.audit {
            Some(audit) => format!(
                "{} · {} 节点 · 未知字段 {} · {}",
                audit.detail,
                audit.node_count,
                audit.unknown_fields.len(),
                if audit.lossless {
                    "结构保真"
                } else {
                    "结构有损"
                }
            ),
            None => "尚未执行编解码转换".to_owned(),
        },
        CustomNodeSlot::UriPreview => studio
            .uri_preview
            .clone()
            .unwrap_or_else(|| "尚无分享链接预览".to_owned()),
        CustomNodeSlot::Gaps => {
            if studio.uri_gaps.is_empty() {
                "分享链接可完整表达当前草稿".to_owned()
            } else {
                format!("分享链接不携带: {}", studio.uri_gaps.join(" / "))
            }
        }
    }
}

/// Custom Node Editor scene.
pub fn custom_node_scene(
    studio: &ProtocolStudioSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let uri_initial = studio.uri_preview.clone().unwrap_or_default();
    let slots: Vec<Box<dyn Scene>> = SLOTS
        .iter()
        .map(|(slot, label)| {
            let value = slot_initial(*slot, studio);
            let label = (*label).to_owned();
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        CustomNodeText({ *slot })
                        Text({ value })
                        TextRole(Role::Caption)
                    ),
                    ( Text({ label }) TextRole(Role::Caption) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                CustomNodeEditorRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Plus, 24.0, palette) } ),
                            ( Text({ "自定义节点与分享链接 (Custom Node & URI Codec)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    (
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.accent })
                        Button
                        ImportUriButton
                        Children [
                            ( Text({ "解析分享链接 URI".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                CustomNodeUriField
                Children [
                    ( { text_field_with_placeholder_scene(
                        uri_initial,
                        "粘贴 vless:// / ss:// / trojan:// / hysteria2:// / tuic:// / ssh:// / anytls:// 分享链接".to_owned(),
                        palette,
                    ) } ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    { slots },
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::top(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "分享链接不携带多路复用/传输层参数；写入配置时未知字段与其它小节均无损保留".to_owned() }) TextRole(Role::Caption) ),
                    (
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.success })
                        Button
                        SaveCustomNodeButton
                        Children [
                            ( Text({ "保存为自定义节点".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}

/// DUAL-05-14: the import button reads the typed URI from the shared field and
/// submits the decode intent; the save button submits the *shared draft* it
/// received from the application. Neither button fabricates a node locally.
pub(crate) fn on_custom_node_action_activated(
    activate: On<Activate>,
    import_buttons: Query<(), With<ImportUriButton>>,
    save_buttons: Query<(), With<SaveCustomNodeButton>>,
    uri_fields: Query<&Children, With<CustomNodeUriField>>,
    text_fields: Query<&TextField>,
    last: Option<Res<LastProxiesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if import_buttons.get(activate.entity).is_ok() {
        let uri = uri_fields
            .iter()
            .flat_map(|children| children.iter())
            .find_map(|child| text_fields.get(child).ok())
            .map(|field| field.0.text())
            .unwrap_or_default();
        let uri = uri.trim().to_owned();
        if uri.is_empty() {
            return;
        }
        handle.submit(UiCommand::ImportCustomNodeUri { uri });
        return;
    }
    if save_buttons.get(activate.entity).is_ok() {
        let draft = last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .and_then(|projection| projection.custom_node.draft.clone());
        let Some(draft) = draft else {
            return;
        };
        handle.submit(UiCommand::SaveCustomNodeDraft {
            draft: Box::new(draft),
        });
    }
}

/// Re-cover every custom-node text slot from the shared studio snapshot. Runs
/// as an observer on the same projection event so it never fights the page
/// restamp queries.
pub(crate) fn sync_custom_node_studio(
    update: On<ProxiesProjectionUpdated>,
    mut texts: Query<(&mut Text, &CustomNodeText)>,
) {
    let studio = &update.0.custom_node;
    for (mut text, slot) in &mut texts {
        text.0 = slot_initial(slot.0, studio);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_initial_reports_shared_facts_and_honest_empty_states() {
        let empty = ProtocolStudioSnapshot::default();
        assert_eq!(slot_initial(CustomNodeSlot::Chips, &empty), "尚无节点草稿");
        assert_eq!(
            slot_initial(CustomNodeSlot::UriPreview, &empty),
            "尚无分享链接预览"
        );
        assert_eq!(
            slot_initial(CustomNodeSlot::Audit, &empty),
            "尚未执行编解码转换"
        );
        assert_eq!(
            slot_initial(CustomNodeSlot::Gaps, &empty),
            "分享链接可完整表达当前草稿"
        );
    }
}
