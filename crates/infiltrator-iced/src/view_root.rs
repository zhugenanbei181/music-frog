use crate::state::AppState;
use crate::types::app::{ConfirmAction, Route, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::RebuildFlowState;
use crate::view;
use iced::widget::{button, column, container, row, stack, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};
use infiltrator_shared::locales::Lang;
use std::time::Instant;

impl AppState {
    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = view::sidebar::sidebar(self);

        // 声明式动画进度计算
        let progress = if let Some(start) = self.shell.transition.start_time {
            let elapsed = Instant::now().duration_since(start).as_millis() as f32;
            let duration = self.shell.transition.duration.as_millis() as f32;
            (elapsed / duration).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // 核心性能优化：不再同时渲染两个页面。转场时只渲染新页面并做淡入。
        let main_content = container(match self.shell.current_route {
            Route::Overview => view::overview::view(self),
            Route::Profiles => view::profiles::view(self),
            Route::Proxies => view::proxies::view(self),
            Route::Runtime => view::runtime::view(self),
            Route::Rules => view::rules::view(self),
            Route::Dns => view::dns::view(self),
            Route::Sync => view::sync::view(self),
            Route::Editor => view::editor::view(self),
            Route::Settings => view::settings::view(self),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(48)
        .style(move |theme: &Theme| {
            container::Style {
                background: Some(crate::view::theme::tokens(theme).canvas.into()),
                // 仅对文字颜色进行透明度插值，这在 CPU 渲染下极快
                text_color: Some(Color {
                    a: progress,
                    ..theme.palette().text
                }),
                ..Default::default()
            }
        });

        let main_view = row![sidebar, main_content];

        let mut layers: Vec<Element<Message>> = vec![main_view.into()];

        if !self.shell.toasts.is_empty() {
            let mut toast_column = column![].spacing(10);
            for (content, status) in &self.shell.toasts {
                let color = move |theme: &Theme| {
                    let tokens = crate::view::theme::tokens(theme);
                    match status {
                        ToastStatus::Info => tokens.accent,
                        ToastStatus::Success => tokens.success,
                        ToastStatus::Warning => tokens.warning,
                        ToastStatus::Error => tokens.danger,
                    }
                };

                toast_column = toast_column.push(
                    container(text(content.clone()).size(13))
                        .padding([12, 24])
                        .style(move |theme: &Theme| {
                            let tokens = crate::view::theme::tokens(theme);
                            container::Style {
                                background: Some(tokens.overlay.into()),
                                border: Border {
                                    radius: 12.0.into(),
                                    width: 1.0,
                                    color: color(theme),
                                },
                                text_color: Some(tokens.overlay_text),
                                ..Default::default()
                            }
                        }),
                );
            }

            layers.push(
                container(toast_column)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(30)
                    .align_x(Alignment::End)
                    .align_y(Alignment::End)
                    .into(),
            );
        }

        if let Some(error) = &self.shell.error_msg {
            let is_en = self.shell.lang.starts_with("en");
            let title = if is_en {
                "Operation failed"
            } else {
                "操作失败"
            };
            let dismiss = if is_en { "Dismiss" } else { "关闭" };
            layers.push(
                container(
                    container(
                        row![
                            column![
                                text(title).size(13),
                                text(error.clone())
                                    .size(12)
                                    .style(|theme: &Theme| text::Style {
                                        color: Some(
                                            crate::view::theme::tokens(theme).overlay_text_muted,
                                        ),
                                    })
                            ]
                            .spacing(4)
                            .width(Length::Fill),
                            button(text(dismiss).size(11))
                                .padding([5, 9])
                                .style(button::secondary)
                                .on_press(Message::ClearError),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding([10, 14])
                    .style(|theme: &Theme| {
                        let tokens = crate::view::theme::tokens(theme);
                        container::Style {
                            background: Some(tokens.overlay.into()),
                            border: Border {
                                radius: 10.0.into(),
                                width: 1.0,
                                color: tokens.danger,
                            },
                            text_color: Some(tokens.overlay_text),
                            ..Default::default()
                        }
                    }),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([16, 24])
                .align_x(Alignment::Center)
                .align_y(Alignment::Start)
                .into(),
            );
        }

        if !matches!(self.runtime.rebuild_flow, RebuildFlowState::Idle) {
            let (title, detail, color): (&str, &str, fn(&Theme) -> Color) =
                match &self.runtime.rebuild_flow {
                    RebuildFlowState::Saving { label } => {
                        ("Saving configuration", label.as_str(), |theme: &Theme| {
                            crate::view::theme::tokens(theme).accent
                        })
                    }
                    RebuildFlowState::Rebuilding { label } => {
                        ("Rebuilding runtime", label.as_str(), |theme: &Theme| {
                            crate::view::theme::tokens(theme).warning
                        })
                    }
                    RebuildFlowState::Done { label } => {
                        ("Completed", label.as_str(), |theme: &Theme| {
                            crate::view::theme::tokens(theme).success
                        })
                    }
                    RebuildFlowState::Failed { label, .. } => {
                        ("Failed", label.as_str(), |theme: &Theme| {
                            crate::view::theme::tokens(theme).danger
                        })
                    }
                    RebuildFlowState::Idle => ("", "", |theme: &Theme| {
                        crate::view::theme::tokens(theme).overlay_text
                    }),
                };

            layers.push(
                container(
                    container(
                        column![
                            text(title).size(14),
                            text(detail).size(12).style(|theme: &Theme| text::Style {
                                color: Some(crate::view::theme::tokens(theme).overlay_text_muted,)
                            })
                        ]
                        .spacing(4),
                    )
                    .padding([12, 18])
                    .style(move |theme: &Theme| {
                        let tokens = crate::view::theme::tokens(theme);
                        container::Style {
                            background: Some(tokens.overlay.into()),
                            border: Border {
                                radius: 10.0.into(),
                                width: 1.0,
                                color: color(theme),
                            },
                            text_color: Some(tokens.overlay_text),
                            ..Default::default()
                        }
                    }),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([16, 0])
                .align_x(Alignment::Center)
                .align_y(Alignment::Start)
                .into(),
            );
        }

        // FPS 诊断 HUD (调试用)。ui-fix: 默认隐藏 —— perf_panel_visible 现在
        // 同时控制 "0 FPS / Perf" 徽标与性能快照面板（此前徽标无条件渲染，
        // 出现在每张截图右上角）。Message::TogglePerfPanel 仍负责切换。
        if self.diag.perf_panel_visible {
            let fps_counter = container(
                row![
                    text(format!("{} FPS", self.diag.fps))
                        .size(10)
                        .style(|theme: &Theme| text::Style {
                            color: Some(crate::view::theme::tokens(theme).text_tertiary),
                        }),
                    button(text("Perf").size(10))
                        .padding([2, 8])
                        .style(button::secondary)
                        .on_press(Message::TogglePerfPanel)
                ]
                .spacing(8),
            )
            .padding(10);

            layers.push(
                container(fps_counter)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Alignment::End)
                    .align_y(Alignment::Start)
                    .into(),
            );
        }

        if let Some(action) = &self.shell.confirmation {
            let (title, detail, confirm_label) =
                confirmation_copy(action, Lang(&self.shell.lang).0.starts_with("en"));
            let cancel_label = if self.shell.lang.starts_with("en") {
                "Cancel"
            } else {
                "取消"
            };
            let dialog = container(
                column![
                    text(title).size(17),
                    text(detail).size(12).style(|theme: &Theme| text::Style {
                        color: Some(crate::view::theme::tokens(theme).text_secondary),
                    }),
                    row![
                        button(text(cancel_label).size(12))
                            .padding([8, 14])
                            .style(button::secondary)
                            .on_press(Message::CancelConfirmation),
                        button(text(confirm_label).size(12))
                            .padding([8, 14])
                            .style(button::danger)
                            .on_press(Message::ConfirmAction),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                ]
                .spacing(14),
            )
            .width(Length::Fixed(420.0))
            .padding(24)
            .style(|theme: &Theme| {
                let tokens = crate::view::theme::tokens(theme);
                container::Style {
                    background: Some(tokens.card_bg.into()),
                    border: Border {
                        radius: 14.0.into(),
                        width: 1.0,
                        color: tokens.card_border,
                    },
                    shadow: tokens.card_shadow,
                    text_color: Some(tokens.text_primary),
                    ..Default::default()
                }
            });

            layers.push(
                container(
                    container(dialog)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_theme: &Theme| container::Style {
                    background: Some(
                        Color {
                            a: 0.45,
                            ..Color::BLACK
                        }
                        .into(),
                    ),
                    ..Default::default()
                })
                .into(),
            );
        }

        if self.diag.perf_panel_visible {
            layers.push(
                container(
                    container(
                        column![
                            text("Performance Snapshot").size(13),
                            text(format!(
                                "Navigate->FirstPaint: {:?}",
                                self.diag.perf_snapshot.navigate_to_first_paint_ms
                            ))
                            .size(11),
                            text(format!(
                                "Rules cache build: {} ms",
                                self.diag.perf_snapshot.rules_cache_build_ms
                            ))
                            .size(11),
                            text(format!(
                                "Rules editor apply: {} ms",
                                self.diag.perf_snapshot.rules_with_text_apply_ms
                            ))
                            .size(11),
                            text(format!(
                                "DNS editor apply: {} ms",
                                self.diag.perf_snapshot.dns_with_text_apply_ms
                            ))
                            .size(11),
                            text(format!(
                                "Rules visible rows: {}",
                                self.diag.perf_snapshot.rules_visible_rows
                            ))
                            .size(11),
                        ]
                        .spacing(6),
                    )
                    .padding([10, 12])
                    .style(|theme: &Theme| {
                        let tokens = crate::view::theme::tokens(theme);
                        container::Style {
                            background: Some(tokens.overlay.into()),
                            border: Border {
                                radius: 10.0.into(),
                                width: 1.0,
                                color: tokens.overlay_border,
                            },
                            text_color: Some(tokens.overlay_text),
                            ..Default::default()
                        }
                    }),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(iced::Padding {
                    top: 46.0,
                    right: 12.0,
                    bottom: 0.0,
                    left: 0.0,
                })
                .align_x(Alignment::End)
                .align_y(Alignment::Start)
                .into(),
            );
        }

        // demo-mode: emit the capture marker only after the first real view
        // pass for the requested page (write-once; see demo.rs).
        if self.shell.demo {
            self.write_capture_marker();
        }

        stack(layers).into()
    }
}

fn confirmation_copy(action: &ConfirmAction, is_en: bool) -> (String, String, String) {
    if is_en {
        match action {
            ConfirmAction::FactoryReset => (
                "Reset application?".to_string(),
                "This stops the core and removes local settings, profiles and installed cores."
                    .to_string(),
                "Reset".to_string(),
            ),
            ConfirmAction::ClearProfiles => (
                "Reset profiles?".to_string(),
                "All profiles will be replaced with the default profile.".to_string(),
                "Reset profiles".to_string(),
            ),
            ConfirmAction::DeleteProfile(name) => (
                "Delete profile?".to_string(),
                format!("The profile \"{name}\" will be permanently deleted."),
                "Delete".to_string(),
            ),
            ConfirmAction::DeleteKernel(version) => (
                "Delete core version?".to_string(),
                format!("The installed core {version} will be removed."),
                "Delete".to_string(),
            ),
            ConfirmAction::CloseAllConnections => (
                "Close all connections?".to_string(),
                "Every active connection will be disconnected.".to_string(),
                "Close all".to_string(),
            ),
        }
    } else {
        match action {
            ConfirmAction::FactoryReset => (
                "恢复出厂设置？".to_string(),
                "这将停止内核并删除本地设置、配置文件和已安装内核。".to_string(),
                "恢复出厂".to_string(),
            ),
            ConfirmAction::ClearProfiles => (
                "重置配置？".to_string(),
                "所有配置将被默认配置替换。".to_string(),
                "重置配置".to_string(),
            ),
            ConfirmAction::DeleteProfile(name) => (
                "删除配置？".to_string(),
                format!("配置“{name}”将被永久删除。"),
                "删除".to_string(),
            ),
            ConfirmAction::DeleteKernel(version) => (
                "删除内核版本？".to_string(),
                format!("已安装的内核 {version} 将被删除。"),
                "删除".to_string(),
            ),
            ConfirmAction::CloseAllConnections => (
                "断开全部连接？".to_string(),
                "所有活动连接都会被断开。".to_string(),
                "全部断开".to_string(),
            ),
        }
    }
}
