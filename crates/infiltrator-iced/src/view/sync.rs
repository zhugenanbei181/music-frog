use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, banner_alert, card, form_field_label, form_input_style, form_toggle_row,
    modern_scrollable, row_card_surface, section_header, status_dot, style_accent, style_ghost,
    text_btn,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_LG, SP_MD, tokens};
use iced::widget::{Space, button, column, container, progress_bar, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let is_en = state.shell.lang.starts_with("en");

    let header = column![
        text(lang.tr("sync_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        text(if is_en {
            "Manage remote WebDAV profile synchronization, backups, and conflict resolution."
        } else {
            "管理远端 WebDAV 配置同步、备份与冲突解决。"
        })
        .size(12)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        }),
    ]
    .spacing(theme::SP_XS);

    // 1. Live Sync Status Card
    let status_card = build_status_card(state, &lang, is_en);

    // 2. Sync Conflicts Panel (if any conflicts exist)
    let conflicts_card = build_conflicts_card(state, &lang, is_en);

    // 3. WebDAV Settings Form Card
    let settings_form = build_settings_form(state, &lang, is_en);

    let mut content = column![
        header,
        Space::new().height(theme::SP_MD),
        section_header(if is_en { "Live Status" } else { "实时状态" }, None),
        status_card,
        Space::new().height(SP_LG),
    ]
    .spacing(SP_MD);

    if let Some(conflicts) = conflicts_card {
        content = content.push(conflicts).push(Space::new().height(SP_LG));
    }

    content = content
        .push(section_header(lang.tr("sync_settings").as_ref(), None))
        .push(settings_form)
        .push(Space::new().height(SP_LG));

    modern_scrollable(content)
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

fn build_status_card<'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
    is_en: bool,
) -> Element<'a, Message> {
    let is_active = state.profile.is_syncing || state.profile.webdav_enabled;

    let (status_title, badge_text, badge_kind) = if state.profile.is_syncing {
        (
            if is_en {
                "Syncing in progress...".to_string()
            } else {
                "正在同步...".to_string()
            },
            if is_en { "SYNCING" } else { "同步中" },
            BadgeKind::Accent,
        )
    } else if state.profile.webdav_enabled {
        (
            lang.tr("sync_auto_enabled").to_string(),
            if is_en { "ACTIVE" } else { "已启用" },
            BadgeKind::Success,
        )
    } else {
        (
            if is_en {
                "WebDAV auto sync disabled".to_string()
            } else {
                "WebDAV 自动同步未开启".to_string()
            },
            if is_en { "DISABLED" } else { "未启用" },
            BadgeKind::Neutral,
        )
    };

    let status_header_row = row![
        text(status_title)
            .size(13)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(theme::SP_SM),
        badge(badge_text, badge_kind),
    ]
    .align_y(Alignment::Center);

    let progress_or_hint: Element<'_, Message> = if let Some(progress) = &state.profile.sync_progress {
        let ratio = if progress.total == 0 {
            0.0
        } else {
            (progress.current as f32 / progress.total as f32).clamp(0.0, 1.0)
        };
        let progress_label = format!(
            "{} {}/{}",
            progress.phase, progress.current, progress.total
        );
        row![
            text(progress_label)
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            Space::new().width(theme::SP_SM),
            progress_bar(0.0..=1.0, ratio).length(Length::Fixed(160.0)),
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center)
        .into()
    } else {
        let hint_text = if state.profile.webdav_sync_on_startup {
            if is_en {
                "Sync on startup enabled".to_string()
            } else {
                "启动时自动同步".to_string()
            }
        } else {
            lang.tr("sync_manual_only").to_string()
        };
        text(hint_text)
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            })
            .into()
    };

    let interval_badge = row![
        svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).text_tertiary),
        Space::new().width(theme::SP_XS),
        text(format!(
            "{} {}",
            state.profile.webdav_sync_interval_mins,
            lang.tr("sync_interval_suffix")
        ))
        .size(12)
        .font(MONO)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        }),
    ]
    .align_y(Alignment::Center);

    card(
        None,
        row![
            status_dot(is_active),
            Space::new().width(theme::SP_MD),
            column![status_header_row, progress_or_hint].spacing(3),
            Space::new().width(Length::Fill),
            interval_badge,
        ]
        .align_y(Alignment::Center),
    )
}

fn build_conflicts_card<'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
    is_en: bool,
) -> Option<Element<'a, Message>> {
    if state.profile.sync_conflicts.is_empty() {
        return None;
    }

    let alert_banner = banner_alert(
        BadgeKind::Warning,
        lang.tr("sync_conflict_title").to_string(),
        lang.tr("sync_conflict_hint").to_string(),
        None,
    );

    let mut conflict_rows = column![].spacing(theme::SP_SM);
    for conflict in &state.profile.sync_conflicts {
        let profile_info = row![
            svg_icons::icon_themed(Icon::FileText, 16.0, |t: &Theme| tokens(t).warning),
            Space::new().width(theme::SP_SM),
            column![
                row![
                    text(conflict.profile.clone())
                        .size(13)
                        .font(FONT_SEMIBOLD)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary),
                        }),
                    Space::new().width(theme::SP_SM),
                    badge(if is_en { "CONFLICT" } else { "冲突" }, BadgeKind::Warning),
                ]
                .align_y(Alignment::Center),
                text(conflict.remote_path.display().to_string())
                    .size(10)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_tertiary),
                    }),
            ]
            .spacing(2),
        ]
        .align_y(Alignment::Center);

        let action_buttons = row![
            text_btn(
                lang.tr("sync_conflict_merge").to_string(),
                style_ghost,
                Some(Message::LoadSyncDiff(conflict.profile.clone())),
            ),
            Space::new().width(theme::SP_SM),
            text_btn(
                lang.tr("sync_conflict_take_remote").to_string(),
                style_accent,
                Some(Message::ResolveSyncConflict(conflict.profile.clone())),
            ),
            Space::new().width(theme::SP_SM),
            text_btn(
                lang.tr("sync_conflict_keep_local").to_string(),
                style_ghost,
                Some(Message::DismissSyncConflict(conflict.profile.clone())),
            ),
        ]
        .align_y(Alignment::Center);

        let row_container = container(
            row![
                profile_info,
                Space::new().width(Length::Fill),
                action_buttons,
            ]
            .align_y(Alignment::Center),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(row_card_surface);

        conflict_rows = conflict_rows.push(row_container);
    }

    let mut card_content = column![
        alert_banner,
        Space::new().height(theme::SP_XS),
        conflict_rows,
    ]
    .spacing(theme::SP_SM);

    if let Some(diff_panel) = crate::view::sync_diff::diff_panel(state) {
        card_content = card_content
            .push(Space::new().height(theme::SP_SM))
            .push(diff_panel);
    }

    Some(card(
        Some(lang.tr("sync_conflict_title").to_string()),
        card_content,
    ))
}

fn build_settings_form<'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
    is_en: bool,
) -> Element<'a, Message> {
    let save_settings_btn: Element<'_, Message> = if state.profile.is_saving_app_settings {
        text_btn(
            if is_en {
                "Saving...".to_string()
            } else {
                "保存中...".to_string()
            },
            style_ghost,
            None,
        )
    } else {
        text_btn(
            if is_en {
                "Save Settings".to_string()
            } else {
                "保存设置".to_string()
            },
            style_accent,
            Some(Message::SaveAppSettings),
        )
    };

    let cancel_sync_btn: Option<Element<'_, Message>> = if state.profile.is_syncing {
        Some(text_btn(
            if is_en {
                "Cancel Sync".to_string()
            } else {
                "取消同步".to_string()
            },
            style_ghost,
            Some(Message::CancelWebDavSync),
        ))
    } else {
        None
    };

    let test_connection_btn = text_btn(
        if state.profile.is_testing_webdav {
            if is_en {
                "Testing...".to_string()
            } else {
                "测试中...".to_string()
            }
        } else {
            if is_en {
                "Test Connection".to_string()
            } else {
                "测试连接".to_string()
            }
        },
        style_ghost,
        (!state.profile.is_syncing && !state.profile.is_testing_webdav)
            .then_some(Message::TestWebDavConnection),
    );

    let upload_btn = button(
        row![
            svg_icons::icon_themed(Icon::ArrowUp, 14.0, |t: &Theme| tokens(t).text_secondary),
            text(lang.tr("sync_upload").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .spacing(theme::SP_XS)
        .align_y(Alignment::Center),
    )
    .padding([7, 14])
    .style(style_ghost)
    .on_press_maybe(
        (!state.profile.is_syncing && !state.profile.is_testing_webdav)
            .then_some(Message::SyncUpload),
    );

    let download_btn = button(
        row![
            svg_icons::icon_themed(Icon::ArrowDown, 14.0, |t: &Theme| tokens(t).on_accent),
            text(lang.tr("sync_download").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .spacing(theme::SP_XS)
        .align_y(Alignment::Center),
    )
    .padding([7, 14])
    .style(style_accent)
    .on_press_maybe(
        (!state.profile.is_syncing && !state.profile.is_testing_webdav)
            .then_some(Message::SyncDownload),
    );

    let mut left_actions = row![save_settings_btn]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center);
    if let Some(cancel) = cancel_sync_btn {
        left_actions = left_actions.push(cancel);
    }
    left_actions = left_actions.push(test_connection_btn);

    let actions_row = row![
        left_actions,
        Space::new().width(Length::Fill),
        upload_btn,
        Space::new().width(theme::SP_SM),
        download_btn,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    card(
        None,
        column![
            form_toggle_row(
                if is_en {
                    "Enable WebDAV Auto Sync".to_string()
                } else {
                    "开启 WebDAV 自动同步".to_string()
                },
                state.profile.webdav_enabled,
                Message::UpdateWebDavEnabled,
            ),
            Space::new().height(theme::SP_MD),
            column![
                form_field_label(lang.tr("sync_url").to_string()),
                Space::new().height(theme::SP_XS),
                text_input("https://dav.example.com/dav/", &state.profile.webdav_url)
                    .on_input(Message::UpdateWebDavUrl)
                    .padding([8, 12])
                    .size(13)
                    .style(form_input_style),
            ]
            .spacing(theme::SP_XS),
            Space::new().height(theme::SP_MD),
            row![
                column![
                    form_field_label(lang.tr("sync_user").to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input(lang.tr("sync_user").as_ref(), &state.profile.webdav_user)
                        .on_input(Message::UpdateWebDavUser)
                        .padding([8, 12])
                        .size(13)
                        .style(form_input_style),
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_LG),
                column![
                    form_field_label(lang.tr("sync_pass").to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input(lang.tr("sync_pass").as_ref(), &state.profile.webdav_pass)
                        .on_input(Message::UpdateWebDavPass)
                        .padding([8, 12])
                        .size(13)
                        .secure(true)
                        .style(form_input_style),
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
            ],
            Space::new().height(theme::SP_MD),
            row![
                column![
                    form_field_label(if is_en {
                        "Sync Interval (mins)".to_string()
                    } else {
                        "自动同步间隔 (分钟)".to_string()
                    }),
                    Space::new().height(theme::SP_XS),
                    text_input("60", &state.profile.webdav_sync_interval_mins)
                        .on_input(Message::UpdateWebDavSyncInterval)
                        .padding([8, 12])
                        .size(13)
                        .font(theme::MONO)
                        .style(form_input_style),
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_LG),
                column![
                    form_field_label(if is_en {
                        "Startup Behavior".to_string()
                    } else {
                        "启动行为".to_string()
                    }),
                    Space::new().height(theme::SP_XS),
                    form_toggle_row(
                        if is_en {
                            "Sync on startup".to_string()
                        } else {
                            "启动时自动同步".to_string()
                        },
                        state.profile.webdav_sync_on_startup,
                        Message::UpdateWebDavSyncOnStartup,
                    ),
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
            ],
            Space::new().height(theme::SP_LG),
            actions_row,
        ]
        .spacing(theme::SP_SM),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app::{SyncConflict, SyncProgress};
    use crate::types::options::{SyncDiffBundle, SyncDiffState};
    use std::path::PathBuf;

    #[test]
    fn test_sync_view_idle_zh() {
        let (mut state, _) = AppState::new();
        state.shell.lang = "zh-CN".into();
        state.profile.webdav_enabled = true;
        state.profile.webdav_url = "https://dav.example.com".into();
        state.profile.webdav_user = "user".into();
        state.profile.webdav_pass = "pass".into();
        state.profile.webdav_sync_interval_mins = "30".into();
        state.profile.webdav_sync_on_startup = true;

        let _element: Element<'_, Message> = view(&state);
    }

    #[test]
    fn test_sync_view_idle_en() {
        let (mut state, _) = AppState::new();
        state.shell.lang = "en-US".into();
        state.profile.webdav_enabled = false;

        let _element: Element<'_, Message> = view(&state);
    }

    #[test]
    fn test_sync_view_syncing_with_progress() {
        let (mut state, _) = AppState::new();
        state.profile.is_syncing = true;
        state.profile.sync_progress = Some(SyncProgress {
            phase: "Uploading".to_string(),
            current: 2,
            total: 5,
        });

        let _element: Element<'_, Message> = view(&state);
    }

    #[test]
    fn test_sync_view_with_conflicts_and_diff() {
        let (mut state, _) = AppState::new();
        state.profile.sync_conflicts = vec![SyncConflict {
            profile: "config-a".into(),
            remote_path: PathBuf::from("/tmp/config-a.remote.yaml"),
        }];
        state.profile.sync_diff = Some(SyncDiffState::new(SyncDiffBundle {
            profile: "config-a".into(),
            remote_path: PathBuf::from("/tmp/config-a.remote.yaml"),
            added: vec!["dns".into()],
            removed: vec![],
            modified: vec![("mode".into(), "rule".into(), "global".into())],
        }));

        let _element: Element<'_, Message> = view(&state);
    }
}
