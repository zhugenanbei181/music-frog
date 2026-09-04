//!  Editor page for raw profile YAML editing, Mixin overlay editing and
//! per-profile subscription filtering with history snapshot restoration.

use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::view::components::{
    BadgeKind, badge, card_surface, chip, kbd_badge, modern_scrollable, segmented_control,
    style_accent, style_ghost,
};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text, text_editor};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};
use std::path::PathBuf;

const SNIPPET_SS: &str = "
  - name: SS-Node
    type: ss
    server: server.example.com
    port: 8388
    cipher: aes-256-gcm
    password: password
";
const SNIPPET_VMESS: &str = "
  - name: Vmess-Node
    type: vmess
    server: server.example.com
    port: 443
    uuid: a3482e88-7d8f-4a42-9988-1a2b3c4d5e6f
    alterId: 0
    cipher: auto
    tls: true
";
const SNIPPET_TROJAN: &str = "
  - name: Trojan-Node
    type: trojan
    server: server.example.com
    port: 443
    password: password
    sni: example.com
";
const SNIPPET_HY2: &str = "
  - name: Hy2-Node
    type: hysteria2
    server: server.example.com
    port: 443
    password: password
    sni: example.com
";
const SNIPPET_SELECT: &str = "
  - name: PROXIES
    type: select
    proxies:
      - DIRECT
";
const SNIPPET_URLTEST: &str = "
  - name: AUTO-TEST
    type: url-test
    url: http://www.gstatic.com/generate_204
    interval: 300
    proxies:
      - DIRECT
";
const SNIPPET_RULE_DOMAIN: &str = "
  - DOMAIN-SUFFIX,google.com,PROXIES
";
const SNIPPET_RULE_GEOIP: &str = "
  - GEOIP,CN,DIRECT
";

/// Truncate full SHA-256 to 8-character short hash pill string.
pub fn format_short_sha(sha: &str) -> String {
    sha.chars().take(8).collect()
}

/// Format the syntax error line badge text.
pub fn format_syntax_line_pill(line: usize) -> String {
    format!("Line {line}")
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let filename = state
        .editor
        .editor_path
        .as_ref()
        .and_then(|p: &PathBuf| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| lang.tr("editor_untitled").to_string());

    // Pane switch: raw profile YAML, the mixin overlay and the per-profile
    // subscription filter (both applied on top of the profile document).
    let pane_labels = vec![
        lang.tr("editor_pane_yaml").to_string(),
        lang.tr("editor_pane_mixin").to_string(),
        lang.tr("editor_pane_filter").to_string(),
        lang.tr("script_sandbox_title").to_string(),
    ];
    let pane_index = match state.editor.editor_pane {
        EditorPane::Profile => 0,
        EditorPane::Mixin => 1,
        EditorPane::Filter => 2,
        EditorPane::Script => 3,
    };
    let pane_switch = segmented_control(&pane_labels, pane_index, |index| {
        Message::SetEditorPane(match index {
            1 => EditorPane::Mixin,
            2 => EditorPane::Filter,
            3 => EditorPane::Script,
            _ => EditorPane::Profile,
        })
    });

    let pane_icon = match state.editor.editor_pane {
        EditorPane::Profile => Icon::FileText,
        EditorPane::Mixin => Icon::Code2,
        EditorPane::Filter => Icon::ListChecks,
        EditorPane::Script => Icon::Zap,
    };

    let pane_tag = match state.editor.editor_pane {
        EditorPane::Profile => "YAML",
        EditorPane::Mixin => "Mixin",
        EditorPane::Filter => "Filter",
        EditorPane::Script => "QuickJS",
    };

    // File info block with icon chip, filename, and format chip
    let file_info = row![
        container(icon_themed(pane_icon, 16.0, |t: &Theme| tokens(t).accent))
            .width(32)
            .height(32)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|t: &Theme| container::Style {
                background: Some(tokens(t).accent_soft.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }),
        column![
            row![
                text(filename)
                    .size(16)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                Space::new().width(theme::SP_SM),
                chip(pane_tag),
            ]
            .align_y(Alignment::Center),
            text(
                state
                    .editor
                    .editor_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
            )
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        ]
        .spacing(2),
    ]
    .spacing(theme::SP_MD)
    .align_y(Alignment::Center);

    // Save action button with saving state and keyboard shortcut badge
    let (save_label, saving) = match state.editor.editor_pane {
        EditorPane::Profile => (
            lang.tr("btn_save").to_string(),
            state.profile.is_saving_profile,
        ),
        EditorPane::Mixin => (
            if state.editor.is_saving_mixin {
                lang.tr("editor_applying").to_string()
            } else {
                lang.tr("editor_apply_mixin").to_string()
            },
            state.editor.is_saving_mixin,
        ),
        EditorPane::Filter => (
            if state.editor.is_saving_filter {
                lang.tr("editor_applying").to_string()
            } else {
                lang.tr("editor_apply_filter").to_string()
            },
            state.editor.is_saving_filter,
        ),
        EditorPane::Script => (
            lang.tr("script_sandbox_run").to_string(),
            state.editor.script_sandbox.is_running,
        ),
    };

    let save_btn = button(
        row![
            text(save_label).size(12).font(FONT_MEDIUM),
            Space::new().width(theme::SP_SM),
            kbd_badge("Ctrl+S"),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_accent)
    .on_press_maybe((!saving && state.editor.editor_path.is_some()).then_some(
        match state.editor.editor_pane {
            EditorPane::Profile => Message::SaveProfile,
            EditorPane::Mixin => Message::SaveMixin,
            EditorPane::Filter => Message::SaveProfileFilter,
            EditorPane::Script => Message::RunScriptSandboxTest,
        },
    ));

    let cancel_btn = button(
        row![
            icon_themed(Icon::X, 14.0, |t: &Theme| tokens(t).text_secondary),
            text(lang.tr("btn_cancel").to_string())
                .size(12)
                .font(FONT_MEDIUM),
            Space::new().width(theme::SP_XS),
            kbd_badge("Esc"),
        ]
        .spacing(theme::SP_XS)
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press(Message::Navigate(Route::Profiles));

    // Toolbar row: file info + pane switch + action buttons (Save / Cancel).
    let toolbar = row![
        file_info,
        Space::new().width(Length::Fill),
        pane_switch,
        Space::new().width(theme::SP_LG),
        save_btn,
        Space::new().width(theme::SP_SM),
        cancel_btn,
    ]
    .align_y(Alignment::Center);

    // Editor area framed in a card surface, mono typeface for YAML. The
    // Mixin pane shows the overlay document; the Filter pane renders the
    // per-profile filter form instead of a text editor.
    let editor_document: Element<'_, Message> = match state.editor.editor_pane {
        EditorPane::Profile => text_editor(&state.editor.editor_content)
            .on_action(Message::EditorAction)
            .font(MONO)
            .padding(14)
            .height(Length::Fill)
            .into(),
        EditorPane::Mixin => text_editor(&state.editor.mixin_content)
            .on_action(Message::MixinEditorAction)
            .font(MONO)
            .padding(14)
            .height(Length::Fill)
            .into(),
        EditorPane::Filter => crate::view::profile_filter::filter_pane(state),
        EditorPane::Script => crate::view::script_console::view(state),
    };
    let editor = container(editor_document)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(card_surface);

    // History snapshots side panel
    let history_panel = build_history_panel(state, &lang);

    let hint_style = |t: &Theme| text::Style {
        color: Some(tokens(t).text_secondary),
    };
    let pane_hint: Option<Element<'_, Message>> = match state.editor.editor_pane {
        EditorPane::Profile => None,
        EditorPane::Mixin => Some(
            text(lang.tr("editor_mixin_hint").to_string())
                .size(11)
                .style(hint_style)
                .into(),
        ),
        EditorPane::Filter => Some(
            text(lang.tr("editor_filter_hint").to_string())
                .size(11)
                .style(hint_style)
                .into(),
        ),
        EditorPane::Script => Some(
            text(lang.tr("script_sandbox_subtitle").to_string())
                .size(11)
                .style(hint_style)
                .into(),
        ),
    };

    let syntax_alert: Option<Element<'_, Message>> =
        state.editor.syntax_error.as_ref().map(|msg| {
            let line_badge = state.editor.syntax_error_line.map(|l| {
                container(
                    text(format_syntax_line_pill(l))
                        .size(10)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).danger),
                        }),
                )
                .padding([2, 8])
                .style(|t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(Color { a: 0.16, ..tk.danger }.into()),
                        border: Border {
                            radius: border::Radius::from(theme::R_CHIP),
                            width: 1.0,
                            color: Color { a: 0.35, ..tk.danger },
                        },
                        ..Default::default()
                    }
                })
            });

            let mut header_row = row![
                icon_themed(Icon::Activity, 16.0, |t: &Theme| tokens(t).danger),
                text(lang.tr("yaml_status_error").to_string())
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).danger),
                    }),
            ]
            .spacing(theme::SP_SM)
            .align_y(Alignment::Center);

            if let Some(lb) = line_badge {
                header_row = header_row.push(lb);
            }

            let banner_body = column![
                header_row,
                text(msg.clone())
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
            ]
            .spacing(theme::SP_XS);

            container(banner_body)
                .padding([10, 14])
                .width(Length::Fill)
                .style(|t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(Color { a: 0.10, ..tk.danger }.into()),
                        border: Border {
                            radius: border::Radius::from(theme::R_CONTROL),
                            width: 1.0,
                            color: Color { a: 0.30, ..tk.danger },
                        },
                        ..Default::default()
                    }
                })
                .into()
        });

        let snippets_bar = row![
        text(lang.tr("yaml_snippets_title").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) }),
        Space::new().width(theme::SP_SM),
        snip_btn("+ Shadowsocks", SNIPPET_SS),
        Space::new().width(theme::SP_XS),
        snip_btn("+ Vmess", SNIPPET_VMESS),
        Space::new().width(theme::SP_XS),
        snip_btn("+ Trojan", SNIPPET_TROJAN),
        Space::new().width(theme::SP_XS),
        snip_btn("+ Hy2", SNIPPET_HY2),
        Space::new().width(theme::SP_XS),
        snip_btn("+ Select", SNIPPET_SELECT),
        Space::new().width(theme::SP_XS),
        snip_btn("+ URL-Test", SNIPPET_URLTEST),
        Space::new().width(theme::SP_XS),
        snip_btn("+ DOMAIN", SNIPPET_RULE_DOMAIN),
        Space::new().width(theme::SP_XS),
        snip_btn("+ GEOIP", SNIPPET_RULE_GEOIP),
        Space::new().width(Length::Fill),
        button(row![icon_themed(Icon::Code2, 12.0, |t: &Theme| tokens(t).accent), Space::new().width(4.0), text(lang.tr("yaml_format_btn").to_string()).size(11).font(FONT_MEDIUM)].align_y(Alignment::Center))
            .style(style_ghost).padding([3, 8]).on_press(Message::FormatYamlEditor),
    ].align_y(Alignment::Center);
    let mut content = column![toolbar, Space::new().height(theme::SP_XS), snippets_bar, Space::new().height(theme::SP_SM)];
    if let Some(alert) = syntax_alert {
        content = content.push(alert).push(Space::new().height(theme::SP_SM));
    }
    if let Some(hint) = pane_hint {
        content = content.push(hint).push(Space::new().height(theme::SP_SM));
    }
    // The Filter pane owns its full-width form; the document panes share the
    // editor + history side panel layout.
    if state.editor.editor_pane == EditorPane::Filter || state.editor.editor_pane == EditorPane::Script {
        content = content.push(editor);
    } else {
        content = content.push(
            row![
                editor,
                Space::new().width(theme::SP_MD),
                history_panel
            ]
            .height(Length::Fill),
        );
    }
    let content = content.spacing(theme::SP_SM);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn build_history_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let mut history_header = row![
        icon_themed(Icon::RefreshCw, 14.0, |t: &Theme| tokens(t).text_secondary),
        text(lang.tr("editor_history").to_string())
            .font(FONT_SEMIBOLD)
            .size(13)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);

    if !state.editor.profile_snapshots.is_empty() {
        history_header = history_header.push(Space::new().width(Length::Fill)).push(
            badge(
                format!("{}", state.editor.profile_snapshots.len()),
                BadgeKind::Neutral,
            ),
        );
    }

    let mut items_col = column![].spacing(theme::SP_SM);

    if state.editor.is_loading_snapshots {
        items_col = items_col.push(
            text(lang.tr("editor_history_loading").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    } else if state.editor.profile_snapshots.is_empty() {
        items_col = items_col.push(
            text(lang.tr("editor_history_empty").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    } else {
        for snapshot in state.editor.profile_snapshots.iter().take(12) {
            let short_hash = format_short_sha(&snapshot.sha256);
            let hash_pill = container(
                text(short_hash)
                    .size(10)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
            )
            .padding([2, 6])
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(tk.control_bg.into()),
                    border: Border {
                        radius: border::Radius::from(4.0),
                        width: 1.0,
                        color: tk.card_border,
                    },
                    ..Default::default()
                }
            });

            let timestamp_text = text(snapshot.timestamp.format("%m-%d %H:%M").to_string())
                .size(11)
                .font(FONT_MEDIUM)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                });

            let restore_btn = button(
                text(if state.editor.is_restoring_snapshot {
                    "...".to_string()
                } else {
                    lang.tr("editor_restore").to_string()
                })
                .size(11)
                .font(FONT_MEDIUM),
            )
            .padding([4, 10])
            .style(style_ghost)
            .on_press_maybe(
                (!state.editor.is_restoring_snapshot)
                    .then_some(Message::RestoreProfileSnapshot(snapshot.path.clone())),
            );

            let snapshot_card = container(
                row![
                    column![timestamp_text, hash_pill].spacing(3).width(Length::Fill),
                    restore_btn,
                ]
                .spacing(theme::SP_SM)
                .align_y(Alignment::Center),
            )
            .padding([8, 10])
            .width(Length::Fill)
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(tk.control_bg.into()),
                    border: Border {
                        radius: border::Radius::from(theme::R_CONTROL),
                        width: 1.0,
                        color: tk.card_border,
                    },
                    ..Default::default()
                }
            });

            items_col = items_col.push(snapshot_card);
        }
    }

    let panel_body = column![
        history_header,
        Space::new().height(theme::SP_XS),
        modern_scrollable(items_col).height(Length::Fill),
    ]
    .spacing(theme::SP_SM);

    container(panel_body)
        .width(Length::Fixed(260.0))
        .height(Length::Fill)
        .padding(theme::SP_MD)
        .style(card_surface)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_short_sha() {
        assert_eq!(format_short_sha("1a2b3c4d5e6f7890"), "1a2b3c4d");
        assert_eq!(format_short_sha("short"), "short");
        assert_eq!(format_short_sha(""), "");
    }

    #[test]
    fn test_format_syntax_line_pill() {
        assert_eq!(format_syntax_line_pill(10), "Line 10");
        assert_eq!(format_syntax_line_pill(1), "Line 1");
    }

    #[test]
    fn test_editor_view_render_all_panes() {
        {
            let (mut state, _) = AppState::new();
            state.editor.editor_pane = EditorPane::Profile;
            let _v = view(&state);
        }
        {
            let (mut state, _) = AppState::new();
            state.editor.editor_pane = EditorPane::Mixin;
            let _v = view(&state);
        }
        {
            let (mut state, _) = AppState::new();
            state.editor.editor_pane = EditorPane::Filter;
            let _v = view(&state);
        }
    }

    #[test]
    fn test_editor_view_with_syntax_error() {
        let (mut state, _) = AppState::new();
        state.editor.syntax_error = Some("Mapping values are not allowed here".into());
        state.editor.syntax_error_line = Some(14);
        let _v = view(&state);
    }
}

fn snip_btn<'a>(label: &'static str, snippet: &'static str) -> Element<'a, Message> {
    button(text(label).size(10).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }))
        .padding([2, 6])
        .style(|t: &Theme, status| {
            let tk = tokens(t);
            button::Style {
                background: match status {
                    button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
                    _ => Some(tk.chip_bg.into()),
                },
                border: Border { radius: border::Radius::from(theme::R_CHIP), width: 1.0, color: tk.card_border },
                ..Default::default()
            }
        })
        .on_press(Message::InsertYamlSnippet(snippet))
        .into()
}
