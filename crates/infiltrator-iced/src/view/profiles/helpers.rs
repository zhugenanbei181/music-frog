//! Formatting, clipboard and per-profile traffic widgets for the Profiles page.

use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge};
use crate::view::theme;
use crate::view::theme::{FONT_MEDIUM, MONO, tokens};
use chrono::{DateTime, Local, Utc};
use iced::widget::{Space, button, column, progress_bar, row, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_application::profile_metadata_projection::{UsageGrade, traffic, traffic_caption};
use infiltrator_desktop::clipboard_helper::ClipboardHelper;
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};
#[cfg(target_os = "linux")]
use std::env::var_os;
use std::process;

/// Format an optional UTC timestamp as localized datetime with a fallback string.
pub(super) fn format_datetime(value: Option<DateTime<Utc>>, fallback: &str) -> String {
    value
        .map(|ts| {
            ts.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| fallback.to_string())
}

/// Read plain text or subscription URL from system clipboard across platforms.
pub(super) fn read_clipboard_url() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = process::Command::new("pbpaste").output() {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    let clean = ClipboardHelper::sanitize_clipboard_text(&s);
                    if !clean.is_empty() {
                        return Some(
                            ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean),
                        );
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "Get-Clipboard"])
            .output()
        {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    let clean = ClipboardHelper::sanitize_clipboard_text(&s);
                    if !clean.is_empty() {
                        return Some(
                            ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean),
                        );
                    }
                }
            }
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if var_os("WAYLAND_DISPLAY").is_some()
            && let Ok(output) = process::Command::new("wl-paste")
                .args(["--no-newline"])
                .output()
            && output.status.success()
            && !output.stdout.is_empty()
            && let Ok(s) = String::from_utf8(output.stdout)
        {
            let clean = ClipboardHelper::sanitize_clipboard_text(&s);
            if !clean.is_empty() {
                return Some(ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean));
            }
        }
        if var_os("DISPLAY").is_some() {
            if let Ok(output) = process::Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output()
                && output.status.success()
                && !output.stdout.is_empty()
                && let Ok(s) = String::from_utf8(output.stdout)
            {
                let clean = ClipboardHelper::sanitize_clipboard_text(&s);
                if !clean.is_empty() {
                    return Some(
                        ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean),
                    );
                }
            }
            if let Ok(output) = process::Command::new("xsel")
                .args(["--clipboard", "--output"])
                .output()
                && output.status.success()
                && !output.stdout.is_empty()
                && let Ok(s) = String::from_utf8(output.stdout)
            {
                let clean = ClipboardHelper::sanitize_clipboard_text(&s);
                if !clean.is_empty() {
                    return Some(
                        ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean),
                    );
                }
            }
        }
    }
    None
}

/// User-Agent preset chip button: clicking it quickly fills the UA input field (P12-11).
pub(super) fn ua_preset_chip<'a>(label: &'static str, current_val: &str) -> Element<'a, Message> {
    let is_selected = current_val == label;
    button(text(label).size(11).font(FONT_MEDIUM))
        .padding([4, 10])
        .style(move |t: &Theme, status| {
            let tk = tokens(t);
            button::Style {
                background: Some(
                    if is_selected {
                        tk.accent_soft
                    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                        tk.control_bg
                    } else {
                        tk.chip_bg
                    }
                    .into(),
                ),
                border: Border {
                    radius: border::Radius::from(theme::R_CHIP),
                    width: if is_selected { 1.5 } else { 1.0 },
                    color: if is_selected {
                        tk.accent
                    } else {
                        tk.card_border
                    },
                },
                text_color: if is_selected {
                    tk.accent
                } else {
                    tk.text_secondary
                },
                ..Default::default()
            }
        })
        .on_press(Message::UpdateSubscriptionUserAgent(label.to_string()))
        .into()
}

/// Traffic usage row for subscription profiles: formatted upload/download usage,
/// total quota, progress bar with color-coded warning (<50% green, 50-80% blue,
/// 80-90% amber, >90% red), and expiration countdown badge (P12-05, P12-06).
pub(super) fn traffic_row<'a>(
    profile: &ProfileInfo,
    lang: &Lang<'_>,
) -> Option<Element<'a, Message>> {
    if profile.subscription_url.is_none()
        && profile.traffic_upload.is_none()
        && profile.traffic_download.is_none()
        && profile.traffic_total.is_none()
        && profile.expire_at.is_none()
    {
        return None;
    }
    let facts = traffic(
        profile.traffic_upload,
        profile.traffic_download,
        profile.traffic_total,
    );
    let fraction = facts.fraction.unwrap_or(0.0);
    let is_exhausted = matches!(facts.grade, UsageGrade::High);
    let is_depleted = matches!(facts.grade, UsageGrade::Exhausted);
    let is_warning = matches!(facts.grade, UsageGrade::Warning);
    let now = Utc::now().timestamp();
    let is_expired = profile.expire_at.is_some_and(|exp| exp > 0 && exp <= now);
    let is_expiring_soon = profile
        .expire_at
        .is_some_and(|exp| exp > now && exp - now < 3 * 86400);

    let expire_suffix = profile
        .expire_at
        .and_then(|sec| DateTime::from_timestamp(sec, 0))
        .map(|exp| {
            let d = exp.with_timezone(&Local).format("%Y-%m-%d").to_string();
            format!(
                "  {}",
                interpolate(&lang.tr("profiles_expires_at"), &[("d", &d)])
            )
        })
        .unwrap_or_default();

    let usage_label = format!("{}{expire_suffix}", traffic_caption(&facts, lang.0));

    let mut info_row = row![
        text(usage_label)
            .size(11)
            .font(MONO)
            .style(move |t: &Theme| {
                let tk = tokens(t);
                let col = if is_depleted || is_expired {
                    tk.danger
                } else if is_exhausted || is_expiring_soon || is_warning {
                    tk.warning
                } else {
                    tk.text_secondary
                };
                text::Style { color: Some(col) }
            }),
        Space::new().width(Length::Fill),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);

    if is_depleted {
        info_row = info_row.push(badge(
            lang.tr("profiles_exhausted").to_string(),
            BadgeKind::Danger,
        ));
    } else if is_exhausted {
        info_row = info_row.push(badge(
            lang.tr("profiles_almost_exhausted").to_string(),
            BadgeKind::Danger,
        ));
    } else if is_warning {
        info_row = info_row.push(badge(
            lang.tr("profiles_almost_exhausted").to_string(),
            BadgeKind::Warning,
        ));
    }

    if is_expired {
        info_row = info_row.push(badge(
            lang.tr("profiles_expired").to_string(),
            BadgeKind::Danger,
        ));
    } else if is_expiring_soon {
        let days = profile
            .expire_at
            .map(|exp| ((exp - now) / 86400).max(1))
            .unwrap_or(1);
        let label = interpolate(
            &lang.tr("profiles_expiring_soon"),
            &[("days", &days.to_string())],
        );
        info_row = info_row.push(badge(label, BadgeKind::Warning));
    }

    let bar: Element<'a, Message> = if facts.fraction.is_some() {
        progress_bar(0.0..=1.0, fraction)
            .length(Length::Fill)
            .style(move |t: &Theme| {
                let tk = tokens(t);
                let bar_color = if fraction >= 0.90 {
                    tk.danger
                } else if fraction >= 0.80 {
                    tk.warning
                } else if fraction >= 0.50 {
                    tk.accent
                } else {
                    tk.success
                };
                progress_bar::Style {
                    background: tk.control_bg.into(),
                    bar: bar_color.into(),
                    border: Border {
                        radius: border::Radius::from(theme::R_XS),
                        ..Default::default()
                    },
                }
            })
            .into()
    } else {
        Space::new().width(0).height(0).into()
    };

    Some(
        column![bar, Space::new().height(2.0), info_row]
            .spacing(2)
            .into(),
    )
}
