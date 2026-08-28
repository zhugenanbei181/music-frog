use crate::locales::{Lang, Localizer};
use crate::types::RuntimeStatus;
use crate::view::components::{WEB_ACCENT, card, modern_scrollable, premium_card, status_dot};
use crate::view::icons;
use crate::{AppState, Message};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Font, Length, border};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };

    let header = column![
        text(lang.tr("nav_overview")).size(32).font(bold_font),
        text("Dashboard metrics and core control center")
            .size(14)
            .style(|_| text::Style {
                color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.4))
            }),
    ]
    .spacing(4);

    // 1. Top Metrics Row
    let metrics = row![
        metric_card(
            "Status".to_string(),
            match &state.status {
                RuntimeStatus::Running => "ACTIVE".to_string(),
                RuntimeStatus::Starting => "STARTING".to_string(),
                _ => "STOPPED".to_string(),
            },
            WEB_ACCENT
        ),
        Space::new().width(20),
        metric_card(
            "Profiles".to_string(),
            format!("{}", state.profiles.len()),
            WEB_ACCENT
        ),
        Space::new().width(20),
        metric_card(
            "Proxies".to_string(),
            format!("{}", state.proxies.len()),
            WEB_ACCENT
        ),
    ]
    .width(Length::Fill);

    // 2. Main Content Grid
    let active_profile = state.profiles.iter().find(|p| p.active);

    let left_col = column![
        premium_card(column![
            text(lang.tr("nav_profiles"))
                .font(bold_font)
                .size(14)
                .style(|_| text::Style {
                    color: Some(WEB_ACCENT)
                }),
            Space::new().height(15),
            text(active_profile.map(|p| p.name.as_str()).unwrap_or("None"))
                .size(24)
                .font(bold_font),
            text(
                active_profile
                    .map(|p| p.path.to_string_lossy().to_string())
                    .unwrap_or_default()
            )
            .size(12)
            .style(|_| text::Style {
                color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.3))
            }),
            Space::new().height(20),
            button(
                row![
                    text(icons::REFRESH).size(14),
                    text(lang.tr("refresh")).size(14)
                ]
                .spacing(10)
            )
            .on_press(Message::LoadProfiles)
            .padding([10, 20])
            .style(button::secondary)
        ]),
        Space::new().height(24),
        card(column![
            text(lang.tr("overview_traffic")).font(bold_font).size(14),
            Space::new().height(20),
            if let Some(traffic) = &state.traffic {
                row![
                    traffic_item("UPLOAD".to_string(), traffic.up, WEB_ACCENT),
                    Space::new().width(40),
                    traffic_item(
                        "DOWNLOAD".to_string(),
                        traffic.down,
                        crate::view::components::WEB_SUCCESS
                    ),
                ]
            } else {
                row![text(lang.tr("waiting_traffic")).style(|_| text::Style {
                    color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.2))
                })]
            }
        ])
    ]
    .width(Length::FillPortion(1));

    let core_status_text = match &state.status {
        RuntimeStatus::Starting => lang.tr("status_starting"),
        RuntimeStatus::Running => lang.tr("status_running"),
        RuntimeStatus::Error(_) => lang.tr("status_error"),
        RuntimeStatus::Stopped => lang.tr("status_stopped"),
    };

    let right_col = column![
        premium_card(column![
            text(lang.tr("overview_core"))
                .font(bold_font)
                .size(14)
                .style(|_| text::Style {
                    color: Some(WEB_ACCENT)
                }),
            Space::new().height(15),
            row![
                status_dot(matches!(state.status, RuntimeStatus::Running)),
                Space::new().width(12),
                text(core_status_text.into_owned()).size(24).font(bold_font),
            ]
            .align_y(Alignment::Center),
            text(format!(
                "Operating Mode: {}",
                state.proxy_mode.as_deref().unwrap_or("rule").to_uppercase()
            ))
            .size(12)
            .style(|_| text::Style {
                color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.3))
            }),
            Space::new().height(25),
            match state.status {
                RuntimeStatus::Running => {
                    button(
                        row![
                            text(icons::CLOSE).size(14),
                            text(lang.tr("stop_proxy")).size(14)
                        ]
                        .spacing(10),
                    )
                    .on_press(Message::StopProxy)
                    .padding([12, 24])
                    .style(button::danger)
                }
                _ => {
                    button(
                        row![
                            text(icons::UPDATE).size(14),
                            text(lang.tr("start_proxy")).size(14)
                        ]
                        .spacing(10),
                    )
                    .on_press(Message::StartProxy)
                    .padding([12, 24])
                    .style(button::primary)
                }
            }
        ]),
        Space::new().height(24),
        card(column![
            text(lang.tr("nav_proxies")).font(bold_font).size(14),
            Space::new().height(15),
            text(
                state
                    .proxies
                    .get("GLOBAL")
                    .and_then(|g: &mihomo_api::Proxy| g.now())
                    .unwrap_or("Direct")
            )
            .size(20)
            .font(bold_font),
            text("Currently routed via GLOBAL")
                .size(12)
                .style(|_| text::Style {
                    color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.3))
                }),
            Space::new().height(20),
            button(
                row![
                    text(icons::PROXY).size(14),
                    text(lang.tr("btn_switch")).size(14)
                ]
                .spacing(10)
            )
            .on_press(Message::Navigate(crate::types::Route::Proxies))
            .padding([10, 20])
            .style(button::secondary)
        ])
    ]
    .width(Length::FillPortion(1));

    let main_grid = row![left_col, Space::new().width(24), right_col].width(Length::Fill);

    let content = column![
        header,
        Space::new().height(32),
        metrics,
        Space::new().height(32),
        main_grid,
    ]
    .max_width(1200)
    .spacing(10);

    // 核心修复：移除所有 Scrollable 外层的 padding 和 center_x，
    // 由 view_root 统领布局，确保不因嵌套溢出而“内容失踪”
    modern_scrollable(content).height(Length::Fill).into()
}

fn metric_card<'a>(label: String, value: String, color: Color) -> Element<'a, Message> {
    container(column![
        text(label.to_uppercase())
            .size(10)
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .style(|_| text::Style {
                color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.3))
            }),
        Space::new().height(5),
        text(value)
            .size(24)
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .style(move |_| text::Style { color: Some(color) }),
    ])
    .padding(20)
    .width(Length::FillPortion(1))
    .style(|_| container::Style {
        background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.02).into()),
        border: Border {
            radius: border::Radius::from(12.0),
            width: 1.0,
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.05),
        },
        ..Default::default()
    })
    .into()
}

fn traffic_item<'a>(label: String, bytes: u64, color: Color) -> Element<'a, Message> {
    column![
        text(label).size(10).style(|_| text::Style {
            color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.3))
        }),
        row![
            text(crate::utils::format_bytes(bytes)).size(24).font(Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
            Space::new().width(8),
            text("B/s")
                .size(12)
                .style(move |_| text::Style { color: Some(color) }),
        ]
        .align_y(Alignment::End)
    ]
    .into()
}
