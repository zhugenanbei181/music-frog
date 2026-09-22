//! DUAL-14-11: the Bevy `dns.hosts` mapping editor card.
//!
//! The draft is the shared [`DnsHostEntry`] row model; the Bevy widget is a
//! single-line editor whose canonical text is the shared
//! `address domain; address domain` grammar (the same codec the Iced panel's
//! graphical row list feeds), so both surfaces submit an identical
//! `DnsSettingsPatch { hosts }`.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns::{
    DnsHostEntry, DnsHostsIssue, DnsSettingsPatch, hosts_editor_text, parse_hosts_editor,
    validate_hosts,
};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection};

/// Marker on the hosts editor text field's parent node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsHostsEditorField;

/// Marker on the hosts apply button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsHostsApplyButton;

/// Marker on the hosts editor status line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsHostsStatusLine;

/// The last text this surface applied (used to decide whether a projection
/// refresh may restamp the editor without clobbering typing).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct DnsHostsEditorState {
    pub applied_text: String,
}

/// The applied-row count line (a shared read-model fact).
pub fn hosts_summary_label(hosts: &[DnsHostEntry]) -> String {
    if hosts.is_empty() {
        "当前 dns.hosts 映射: 未配置".to_owned()
    } else {
        let domains = hosts
            .iter()
            .map(|entry| entry.domain.as_str())
            .collect::<Vec<_>>();
        let mut unique = domains.clone();
        unique.sort_unstable();
        unique.dedup();
        format!(
            "当前 dns.hosts 映射: {} 条地址 / {} 个域名",
            hosts.len(),
            unique.len()
        )
    }
}

/// Bare-Chinese issue copy for the Bevy status line.
pub fn hosts_issue_label(issue: &DnsHostsIssue) -> String {
    match issue {
        DnsHostsIssue::InvalidAddress { address } => {
            format!("地址必须是 IP、lan 或别名域名: {address}")
        }
        DnsHostsIssue::InvalidDomain { domain } => {
            format!("域名不合法: {domain}")
        }
    }
}

/// The hosts editor card.
pub fn dns_hosts_card_scene(projection: &DnsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let editor_text = hosts_editor_text(&projection.hosts);
    let summary = hosts_summary_label(&projection.hosts);

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    (
                        Text({ "自定义 Hosts 映射编辑 (DUAL-14-11)".to_owned() })
                        TextRole(Role::BodyStrong)
                    ),
                    (
                        Text(summary)
                        DnsLine(DnsLineKind::HostsSummary)
                        TextRole(Role::Caption)
                    ),
                ]
            }),
            Box::new(bsn! {
                Node { width: percent(100) }
                DnsHostsEditorField
                Children [
                    (
                        { text_field_with_placeholder_scene(
                            editor_text,
                            "192.168.1.1 router.lan; 10.0.0.1 nas.lan".to_owned(),
                            palette,
                        ) }
                    ),
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
                    (
                        Text({ "地址 域名，多行或分号分隔；留空可清空 dns.hosts".to_owned() })
                        DnsHostsStatusLine
                        TextRole(Role::Caption)
                        TextColor({ palette.ink_dim })
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
                        DnsHostsApplyButton
                        Button
                        Children [
                            ( Text({ "应用 Hosts 映射 (Apply)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}

/// The editor status line for the current submitted/typed state.
pub fn hosts_status_label(applied_text: &str, typed: &str) -> String {
    if typed == applied_text {
        return "Hosts 编辑内容与已应用配置一致".to_owned();
    }
    "有未应用的 Hosts 修改，点击应用提交共享补丁".to_owned()
}

fn read_editor(
    fields: &Query<(&Children, &DnsHostsEditorField)>,
    text_fields: &Query<&TextField>,
) -> Option<String> {
    for (children, _) in fields {
        for child in children.iter() {
            if let Ok(field) = text_fields.get(*child) {
                return Some(field.0.text().to_owned());
            }
        }
    }
    None
}

/// Activation seam: submit the shared hosts patch.
pub(crate) fn on_dns_hosts_activated(
    activate: On<Activate>,
    apply_buttons: Query<(), With<DnsHostsApplyButton>>,
    fields: Query<(&Children, &DnsHostsEditorField)>,
    text_fields: Query<&TextField>,
    handle: Option<bevy::ecs::system::Res<CommandSinkHandle>>,
    mut status_lines: Query<&mut Text, With<DnsHostsStatusLine>>,
    mut state: Option<ResMut<DnsHostsEditorState>>,
) {
    if apply_buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(typed) = read_editor(&fields, &text_fields) else {
        return;
    };
    let entries = parse_hosts_editor(&typed);
    let issues = validate_hosts(&entries);
    let label = match issues.first() {
        Some(issue) => format!("本地校验未通过: {}", hosts_issue_label(issue)),
        None => {
            if let Some(handle) = handle {
                let patch = if entries.is_empty() {
                    DnsSettingsPatch {
                        clear_hosts: true,
                        ..DnsSettingsPatch::default()
                    }
                } else {
                    DnsSettingsPatch {
                        hosts: Some(entries),
                        ..DnsSettingsPatch::default()
                    }
                };
                handle.submit(UiCommand::ApplyDnsSettings { patch });
            }
            if let Some(state) = state.as_deref_mut() {
                state.applied_text = typed;
            }
            "已提交共享 dns.hosts 补丁".to_owned()
        }
    };
    for mut text in &mut status_lines {
        text.0 = label.clone();
    }
}

/// Projection seam: restamp the editor only while it still holds the last
/// applied text (a user edit is never clobbered).
pub(crate) fn apply_dns_hosts_projection(
    update: On<crate::pages::dns::DnsProjectionUpdated>,
    fields: Query<(&Children, &DnsHostsEditorField)>,
    mut text_fields: Query<&mut TextField>,
    mut lines: Query<(&mut Text, &DnsLine)>,
    mut state: Option<ResMut<DnsHostsEditorState>>,
) {
    let wanted = hosts_editor_text(&update.0.hosts);
    let summary = hosts_summary_label(&update.0.hosts);

    for (children, _) in &fields {
        for child in children.iter() {
            if let Ok(mut field) = text_fields.get_mut(*child) {
                let current = field.0.text().to_owned();
                let untouched = state
                    .as_deref()
                    .map(|state| current == state.applied_text)
                    .unwrap_or(true);
                if untouched && current != wanted {
                    field.0.apply(
                        infiltrator_bevy_widgets::text_input::state::TextFieldInput::SetText(
                            wanted.clone(),
                        ),
                    );
                }
            }
        }
    }
    if let Some(state) = state.as_deref_mut()
        && state.applied_text != wanted
    {
        state.applied_text = wanted;
    }

    for (mut text, line) in &mut lines {
        if line.0 == DnsLineKind::HostsSummary && text.0 != summary {
            text.0 = summary.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts_addresses_and_domains() {
        assert_eq!(hosts_summary_label(&[]), "当前 dns.hosts 映射: 未配置");
        let rows = vec![
            DnsHostEntry {
                domain: "multi.example.com".to_owned(),
                address: "1.1.1.1".to_owned(),
            },
            DnsHostEntry {
                domain: "multi.example.com".to_owned(),
                address: "8.8.8.8".to_owned(),
            },
            DnsHostEntry {
                domain: "router.lan".to_owned(),
                address: "192.168.1.1".to_owned(),
            },
        ];
        let label = hosts_summary_label(&rows);
        assert!(label.contains("3 条地址"));
        assert!(label.contains("2 个域名"));
    }

    #[test]
    fn status_reflects_applied_vs_typed() {
        assert!(hosts_status_label("1.1.1.1 a.com", "1.1.1.1 a.com").contains("一致"));
        assert!(hosts_status_label("1.1.1.1 a.com", "8.8.8.8 b.com").contains("未应用"));
        assert!(
            hosts_issue_label(&DnsHostsIssue::InvalidAddress {
                address: "nope".to_owned()
            })
            .contains("nope")
        );
    }
}
