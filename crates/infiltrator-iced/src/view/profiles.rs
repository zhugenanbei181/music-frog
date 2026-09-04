//! Profiles & Subscriptions page view: remote subscription imports, local YAML imports,
//! auto-update scheduler settings, and profile card management with traffic quota and expiry tracking.

use crate::state::AppState;
use crate::types::app::{ConfirmAction, ToastStatus};
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::view::components::{
    badge, banner_alert, card, chip, empty_state, form_field_label, form_input_style,
    form_pick_style, form_toggle_row, icon_button, kbd_badge, modern_scrollable, search_input,
    section_header, segmented_control, style_accent, style_danger, style_ghost, text_btn,
    BadgeKind,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, tokens, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CARD, SP_MD};
use chrono::{DateTime, Local, Utc};
use iced::widget::{
    button, column, container, pick_list, progress_bar, row, text, text_input, Space,
};
use iced::{border, Alignment, Border, Color, Element, Length, Theme};
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_desktop::clipboard_helper::ClipboardHelper;
use infiltrator_shared::locales::{Lang, Localizer};

/// Human-readable byte size (B / KB / MB / GB / TB), formatted with two decimals above 1 KB.
fn format_bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = value as f64;
    let mut unit = 0usize;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{value} {}", UNITS[0]) } else { format!("{size:.2} {}", UNITS[unit]) }
}

/// Format an optional UTC timestamp as localized datetime with a fallback string.
fn format_datetime(value: Option<DateTime<Utc>>, fallback: &str) -> String {
    value
        .map(|ts| ts.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| fallback.to_string())
}

/// Read plain text or subscription URL from system clipboard across platforms.
fn read_clipboard_url() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("pbpaste").output() {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    let clean = ClipboardHelper::sanitize_clipboard_text(&s);
                    if !clean.is_empty() {
                        return Some(ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean));
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("powershell").args(["-NoProfile", "-Command", "Get-Clipboard"]).output() {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    let clean = ClipboardHelper::sanitize_clipboard_text(&s);
                    if !clean.is_empty() {
                        return Some(ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean));
                    }
                }
            }
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some()
            && let Ok(output) = std::process::Command::new("wl-paste").args(["--no-newline"]).output()
                && output.status.success() && !output.stdout.is_empty()
                    && let Ok(s) = String::from_utf8(output.stdout) {
                        let clean = ClipboardHelper::sanitize_clipboard_text(&s);
                        if !clean.is_empty() {
                            return Some(ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean));
                        }
                    }
        if std::env::var_os("DISPLAY").is_some() {
            if let Ok(output) = std::process::Command::new("xclip").args(["-selection", "clipboard", "-o"]).output()
                && output.status.success() && !output.stdout.is_empty()
                    && let Ok(s) = String::from_utf8(output.stdout) {
                        let clean = ClipboardHelper::sanitize_clipboard_text(&s);
                        if !clean.is_empty() {
                            return Some(ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean));
                        }
                    }
            if let Ok(output) = std::process::Command::new("xsel").args(["--clipboard", "--output"]).output()
                && output.status.success() && !output.stdout.is_empty()
                    && let Ok(s) = String::from_utf8(output.stdout) {
                        let clean = ClipboardHelper::sanitize_clipboard_text(&s);
                        if !clean.is_empty() {
                            return Some(ClipboardHelper::extract_subscription_url(&clean).unwrap_or(clean));
                        }
                    }
        }
    }
    None
}

/// User-Agent preset chip button: clicking it quickly fills the UA input field (P12-11).
fn ua_preset_chip<'a>(label: &'static str, current_val: &str) -> Element<'a, Message> {
    let is_selected = current_val == label;
    button(text(label).size(11).font(FONT_MEDIUM))
        .padding([4, 10])
        .style(move |t: &Theme, status| {
            let tk = tokens(t);
            button::Style {
                background: Some(if is_selected { tk.accent_soft } else if matches!(status, button::Status::Hovered | button::Status::Pressed) { tk.control_bg } else { tk.chip_bg }.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CHIP),
                    width: if is_selected { 1.5 } else { 1.0 },
                    color: if is_selected { tk.accent } else { tk.card_border },
                },
                text_color: if is_selected { tk.accent } else { tk.text_secondary },
                ..Default::default()
            }
        })
        .on_press(Message::UpdateSubscriptionUserAgent(label.to_string()))
        .into()
}

/// Traffic usage row for subscription profiles: formatted upload/download usage,
/// total quota, progress bar with color-coded warning (<50% green, 50-80% blue,
/// 80-90% amber, >90% red), and expiration countdown badge (P12-05, P12-06).
fn traffic_row<'a>(
    profile: &ProfileInfo,
    lang: &Lang<'_>,
) -> Option<Element<'a, Message>> {
    let total = profile.traffic_total.unwrap_or(0);
    let upload = profile.traffic_upload.unwrap_or(0);
    let download = profile.traffic_download.unwrap_or(0);
    let used = upload.saturating_add(download);

    if profile.traffic_total.is_none() && profile.traffic_upload.is_none() && profile.traffic_download.is_none() && profile.expire_at.is_none() {
        return None;
    }

    let fraction = if total > 0 { (used as f32 / total as f32).clamp(0.0, 1.0) } else { 0.0 };
    let is_exhausted = total > 0 && fraction >= 0.90;
    let is_depleted = total > 0 && fraction >= 1.0;
    let is_warning = total > 0 && (0.80..0.90).contains(&fraction);
    let now = chrono::Utc::now().timestamp();
    let is_expired = profile.expire_at.is_some_and(|exp| exp > 0 && exp <= now);
    let is_expiring_soon = profile.expire_at.is_some_and(|exp| exp > now && exp - now < 3 * 86400);

    let expire_suffix = profile.expire_at.and_then(|sec| chrono::DateTime::from_timestamp(sec, 0)).map(|exp| {
        let d = exp.with_timezone(&Local).format("%Y-%m-%d").to_string();
        format!("  {}", infiltrator_shared::i18n_interpolator::interpolate(&lang.tr("profiles_expires_at"), &[("d", &d)]))
    }).unwrap_or_default();

    let usage_label = if total > 0 {
        format!("↑ {}  ↓ {}  •  {} / {} ({:.1}%){expire_suffix}", format_bytes(upload), format_bytes(download), format_bytes(used), format_bytes(total), fraction * 100.0)
    } else {
        format!("↑ {}  ↓ {}{expire_suffix}", format_bytes(upload), format_bytes(download))
    };

    let mut info_row = row![
        text(usage_label).size(11).font(MONO).style(move |t: &Theme| {
            let tk = tokens(t);
            let col = if is_depleted || is_expired { tk.danger } else if is_exhausted || is_expiring_soon || is_warning { tk.warning } else { tk.text_secondary };
            text::Style { color: Some(col) }
        }),
        Space::new().width(Length::Fill),
    ].spacing(theme::SP_SM).align_y(Alignment::Center);

    if is_depleted {
        info_row = info_row.push(badge(lang.tr("profiles_exhausted").to_string(), BadgeKind::Danger));
    } else if is_exhausted {
        info_row = info_row.push(badge(lang.tr("profiles_almost_exhausted").to_string(), BadgeKind::Danger));
    } else if is_warning {
        info_row = info_row.push(badge(lang.tr("profiles_almost_exhausted").to_string(), BadgeKind::Warning));
    }

    if is_expired {
        info_row = info_row.push(badge(lang.tr("profiles_expired").to_string(), BadgeKind::Danger));
    } else if is_expiring_soon {
        let days = profile.expire_at.map(|exp| ((exp - now) / 86400).max(1)).unwrap_or(1);
        let label = infiltrator_shared::i18n_interpolator::interpolate(&lang.tr("profiles_expiring_soon"), &[("days", &days.to_string())]);
        info_row = info_row.push(badge(label, BadgeKind::Warning));
    }

    let bar: Element<'a, Message> = if total > 0 {
        progress_bar(0.0..=1.0, fraction).length(Length::Fill).style(move |t: &Theme| {
            let tk = tokens(t);
            let bar_color = if fraction >= 0.90 { tk.danger } else if fraction >= 0.80 { tk.warning } else if fraction >= 0.50 { tk.accent } else { tk.success };
            progress_bar::Style {
                background: tk.control_bg.into(),
                bar: bar_color.into(),
                border: Border { radius: border::Radius::from(3.0), ..Default::default() },
            }
        }).into()
    } else {
        Space::new().width(0).height(0).into()
    };

    Some(column![bar, Space::new().height(2.0), info_row].spacing(2).into())
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let _is_zh = !matches!(state.shell.lang.as_str(), "en-US" | "en");

    let clear_profiles_btn: Element<'_, Message> = if state.profile.is_loading_profiles {
        text_btn(lang.tr("profiles_clearing").to_string(), style_danger, None)
    } else {
        text_btn(lang.tr("profiles_clear_all").to_string(), style_danger, Some(Message::RequestConfirmation(ConfirmAction::ClearProfiles)))
    };

    let search_box = container(search_input(
        lang.tr("profiles_search_placeholder").as_ref(),
        &state.profile.profiles_filter,
        Message::UpdateProfilesFilter,
        Message::UpdateProfilesFilter(String::new()),
    )).width(Length::Fixed(280.0));

    let header = row![
        text(lang.tr("profiles_title").to_string()).size(24).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
        Space::new().width(theme::SP_LG),
        search_box,
        Space::new().width(theme::SP_SM),
        clear_profiles_btn,
        Space::new().width(theme::SP_SM),
        button(
            row![
                svg_icons::icon_themed(Icon::LayoutGrid, 13.0, |t: &Theme| tokens(t).text_secondary),
                Space::new().width(theme::SP_XS),
                text(lang.tr("aggregator_title").to_string()).size(12),
            ]
            .align_y(Alignment::Center)
        )
        .padding([6, 10])
        .style(style_ghost)
        .on_press(Message::OpenAggregatorModal),
        Space::new().width(Length::Fill),
        text_btn(lang.tr("profiles_open_folder").to_string(), style_ghost, Some(Message::OpenConfigDir)),
    ].align_y(Alignment::Center);

    let active_alert: Option<Element<'_, Message>> = if let Some(p) = state.profile.profiles.iter().find(|p| p.active) {
        let now = chrono::Utc::now().timestamp();
        let is_expired = p.expire_at.is_some_and(|exp| exp > 0 && exp <= now);
        let is_expiring_soon = p.expire_at.is_some_and(|exp| exp > now && exp - now < 3 * 86400);
        let total = p.traffic_total.unwrap_or(0);
        let used = p.traffic_upload.unwrap_or(0).saturating_add(p.traffic_download.unwrap_or(0));
        let fraction = if total > 0 { (used as f32 / total as f32).clamp(0.0, 1.0) } else { 0.0 };
        let is_exhausted = total > 0 && fraction >= 0.80;

        if is_expired {
            Some(banner_alert(BadgeKind::Danger, lang.tr("profiles_active_expired_title").to_string(), lang.tr("profiles_active_expired_desc").to_string(), Some(text_btn(lang.tr("profiles_update_now").to_string(), style_ghost, Some(Message::UpdateSubscriptionNow)))))
        } else if is_expiring_soon {
            Some(banner_alert(BadgeKind::Warning, lang.tr("profiles_active_expiring_title").to_string(), lang.tr("profiles_active_expiring_desc").to_string(), Some(text_btn(lang.tr("profiles_update_now").to_string(), style_ghost, Some(Message::UpdateSubscriptionNow)))))
        } else if is_exhausted {
            Some(banner_alert(BadgeKind::Warning, lang.tr("profiles_active_exhausted_title").to_string(), format!("{}: {:.1}%", lang.tr("profiles_used_traffic"), fraction * 100.0), Some(text_btn(lang.tr("profiles_update_now").to_string(), style_ghost, Some(Message::UpdateSubscriptionNow)))))
        } else {
            None
        }
    } else {
        None
    };

    let paste_msg = if let Some(url) = read_clipboard_url() {
        Message::UpdateImportUrl(url)
    } else {
        Message::ShowToast(lang.tr("profiles_no_sub_in_clipboard").to_string(), ToastStatus::Warning)
    };

    let paste_btn = button(
        row![svg_icons::icon_themed(Icon::Copy, 13.0, |t: &Theme| tokens(t).text_secondary), text(lang.tr("profiles_paste").to_string()).size(12).font(FONT_MEDIUM)].spacing(theme::SP_XS).align_y(Alignment::Center),
    ).padding([7, 12]).style(style_ghost).on_press(paste_msg);

    let import_actions: Element<'_, Message> = if state.profile.is_importing {
        text_btn(lang.tr("profiles_importing").to_string(), style_accent, None)
    } else {
        button(row![svg_icons::icon_themed(Icon::Plus, 14.0, |t: &Theme| tokens(t).on_accent), text(lang.tr("profiles_import_btn").to_string()).size(12).font(FONT_MEDIUM)].spacing(theme::SP_SM).align_y(Alignment::Center)).padding([7, 14]).style(style_accent).on_press(Message::ImportProfile).into()
    };

    let import_section = card(
        Some(lang.tr("profiles_import_sub").to_string()),
        column![
            row![
                column![form_field_label(lang.tr("profiles_import_name_placeholder").to_string()), Space::new().height(theme::SP_XS), text_input(lang.tr("profiles_import_name_placeholder").as_ref(), &state.profile.import_name).on_input(Message::UpdateImportName).padding([8, 12]).size(13).style(form_input_style)].width(Length::FillPortion(1)).spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![form_field_label(lang.tr("profiles_sub_url").to_string()), Space::new().height(theme::SP_XS), row![text_input(lang.tr("profiles_sub_url").as_ref(), &state.profile.import_url).on_input(Message::UpdateImportUrl).padding([8, 12]).size(13).width(Length::Fill).style(form_input_style), Space::new().width(theme::SP_XS), paste_btn].align_y(Alignment::Center)].width(Length::FillPortion(2)).spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![Space::new().height(18.0), import_actions].spacing(theme::SP_XS),
            ].align_y(Alignment::Center),
            Space::new().height(theme::SP_MD),
            form_toggle_row(lang.tr("profiles_import_activate").to_string(), state.profile.import_activate, Message::UpdateImportActivate),
        ],
    );

    let local_import_action: Element<'_, Message> = if state.profile.is_importing_local {
        text_btn(lang.tr("profiles_importing").to_string(), style_accent, None)
    } else {
        button(row![svg_icons::icon_themed(Icon::Plus, 14.0, |t: &Theme| tokens(t).on_accent), text(lang.tr("profiles_import_local_btn").to_string()).size(12).font(FONT_MEDIUM)].spacing(theme::SP_SM)).padding([7, 14]).style(style_accent).on_press(Message::ImportLocalProfile).into()
    };

    let local_import_section = card(
        Some(lang.tr("profiles_local_import_title").to_string()),
        column![
            row![
                column![form_field_label(lang.tr("profiles_local_path_placeholder").to_string()), Space::new().height(theme::SP_XS), text_input(lang.tr("profiles_local_path_placeholder").as_ref(), &state.profile.local_import_path).on_input(Message::UpdateLocalImportPath).padding([8, 12]).size(13).style(form_input_style)].width(Length::FillPortion(2)).spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![Space::new().height(18.0), text_btn(lang.tr("profiles_browse_btn").to_string(), style_ghost, Some(Message::BrowseLocalImportFile))].spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![form_field_label(lang.tr("profiles_local_name_placeholder").to_string()), Space::new().height(theme::SP_XS), text_input(lang.tr("profiles_local_name_placeholder").as_ref(), &state.profile.local_import_name).on_input(Message::UpdateLocalImportName).padding([8, 12]).size(13).style(form_input_style)].width(Length::FillPortion(1)).spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![Space::new().height(18.0), local_import_action].spacing(theme::SP_XS),
            ].align_y(Alignment::Center),
            Space::new().height(theme::SP_MD),
            form_toggle_row(lang.tr("profiles_import_activate").to_string(), state.profile.local_import_activate, Message::UpdateLocalImportActivate),
        ],
    );

    let profile_options: Vec<String> = state.profile.profiles.iter().map(|p| p.name.clone()).collect();
    let selected_profile = if state.profile.subscription_profile_name.is_empty() { None } else { Some(&state.profile.subscription_profile_name) };
    let selected_profile_meta = state.profile.profiles.iter().find(|p| p.name == state.profile.subscription_profile_name);
    let interval_options: Vec<String> = ["12", "24", "48", "168"].iter().map(|s| (*s).to_string()).collect();
    let selected_interval = if state.profile.subscription_update_interval_hours.trim().is_empty() { Some("24".to_string()) } else { Some(state.profile.subscription_update_interval_hours.clone()) };

    let subscription_save_action: Element<'_, Message> = if state.profile.is_saving_subscription {
        text_btn(lang.tr("profiles_saving_subscription").to_string(), style_accent, None)
    } else {
        text_btn(lang.tr("profiles_save_subscription").to_string(), style_accent, Some(Message::SaveSubscriptionSettings))
    };

    let subscription_update_now_action: Element<'_, Message> = if state.profile.is_updating_subscription_now {
        text_btn(lang.tr("profiles_updating_subscription").to_string(), style_ghost, None)
    } else {
        text_btn(lang.tr("profiles_update_now").to_string(), style_ghost, Some(Message::UpdateSubscriptionNow))
    };

    let interval_labels: Vec<String> = ["12h", "24h", "48h", "168h"].iter().map(|s| (*s).to_string()).collect();
    let interval_selected = ["12", "24", "48", "168"].iter().position(|h| Some(h.to_string()) == selected_interval).unwrap_or(usize::MAX);
    let interval_control = segmented_control(&interval_labels, interval_selected, |index| {
        let hours = match index { 1 => "24", 2 => "48", 3 => "168", _ => "12" };
        Message::UpdateSubscriptionInterval(hours.to_string())
    });

    let ua_presets = row![
        text(lang.tr("profiles_ua_preset").to_string()).size(11).font(FONT_MEDIUM).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        Space::new().width(theme::SP_XS),
        ua_preset_chip("Clash.Meta", &state.profile.subscription_user_agent),
        Space::new().width(theme::SP_XS),
        ua_preset_chip("ClashVerge", &state.profile.subscription_user_agent),
        Space::new().width(theme::SP_XS),
        ua_preset_chip("Shadowrocket", &state.profile.subscription_user_agent),
    ].spacing(theme::SP_XS).align_y(Alignment::Center);

    let subscription_section = card(
        Some(lang.tr("profiles_subscription_settings_title").to_string()),
        column![
            pick_list(profile_options, selected_profile, Message::SelectSubscriptionProfile).placeholder(lang.tr("profiles_select_profile").as_ref()).width(Length::Fill).style(form_pick_style),
            Space::new().height(theme::SP_MD),
            text_input(lang.tr("profiles_subscription_url").as_ref(), &state.profile.subscription_url).on_input(Message::UpdateSubscriptionUrl).padding([8, 12]).size(13).width(Length::Fill).style(form_input_style),
            Space::new().height(theme::SP_SM),
            text_input("User-Agent (e.g. Clash.Meta / ClashVerge / Shadowrocket)", &state.profile.subscription_user_agent).on_input(Message::UpdateSubscriptionUserAgent).padding([8, 12]).size(12).font(MONO).width(Length::Fill).style(form_input_style),
            Space::new().height(theme::SP_XS),
            ua_presets,
            Space::new().height(theme::SP_MD),
            form_toggle_row(lang.tr("profiles_auto_update").to_string(), state.profile.subscription_auto_update_enabled, Message::UpdateSubscriptionAutoUpdate),
            Space::new().height(theme::SP_SM),
            row![
                pick_list(interval_options, selected_interval.clone(), Message::UpdateSubscriptionInterval).placeholder(lang.tr("profiles_update_interval").as_ref()).text_size(13).width(Length::Fixed(180.0)).style(form_pick_style),
                Space::new().width(theme::SP_MD),
                interval_control,
                Space::new().width(Length::Fill),
            ].align_y(Alignment::Center),
            if let Some(profile) = selected_profile_meta {
                Element::from(row![
                    text(format!("{} {}", lang.tr("profiles_last_updated"), format_datetime(profile.last_updated, lang.tr("profiles_never").as_ref()))).size(12).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                    Space::new().width(theme::SP_MD),
                    text(format!("{} {}", lang.tr("profiles_next_update"), format_datetime(profile.next_update, lang.tr("profiles_not_scheduled").as_ref()))).size(12).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                ].align_y(Alignment::Center))
            } else {
                Element::from(Space::new().width(0))
            },
            Space::new().height(theme::SP_MD),
            row![
                subscription_save_action,
                Space::new().width(theme::SP_MD),
                subscription_update_now_action,
                Space::new().width(theme::SP_MD),
                            text_btn(lang.tr("profiles_open_overlay").to_string(), style_ghost, selected_profile_meta.and_then(|p| (!p.path.is_empty()).then_some(Message::EditProfileAs(p.path.clone().into(), EditorPane::Mixin)))),
            ].align_y(Alignment::Center),
        ].spacing(theme::SP_SM),
    );

    let mut profiles_list = column![].spacing(SP_MD);
    let profile_filter = state.profile.profiles_filter.trim().to_lowercase();
    let filtered_profiles: Vec<_> = state.profile.profiles.iter().filter(|p| {
        profile_filter.is_empty() || p.name.to_lowercase().contains(&profile_filter) || p.path.to_lowercase().contains(&profile_filter)
    }).collect();

    if state.profile.is_loading_profiles {
        profiles_list = profiles_list.push(empty_state(Icon::FileText, lang.tr("loading_profiles").as_ref(), ""));
    } else if state.profile.profiles.is_empty() {
        profiles_list = profiles_list.push(empty_state(Icon::FileText, lang.tr("no_profiles").as_ref(), ""));
    } else if filtered_profiles.is_empty() {
        profiles_list = profiles_list.push(empty_state(Icon::Search, lang.tr("profiles_no_match").as_ref(), ""));
    } else {
        for profile in filtered_profiles {
            let is_active = profile.active;
            let is_subscription = profile.subscription_url.is_some();

            let source_badge: Element<'_, Message> = if is_subscription {
                badge(lang.tr("subscription").as_ref(), BadgeKind::Accent)
            } else {
                chip(lang.tr("profiles_badge_local").to_string())
            };

            let mut actions = row![].spacing(theme::SP_SM).align_y(Alignment::Center);
            if !is_active {
                actions = actions.push(text_btn(lang.tr("use").to_string(), style_ghost, Some(Message::SetActiveProfile(profile.name.clone()))));
            }
            actions = actions.push(icon_button(Icon::Pencil, 14.0, Message::EditProfile(profile.path.clone().into())));
            actions = actions.push(icon_button(Icon::Code2, 14.0, Message::EditProfileAs(profile.path.clone().into(), EditorPane::Mixin)));
            if !is_active {
                actions = actions.push(icon_button(Icon::Trash2, 14.0, Message::RequestConfirmation(ConfirmAction::DeleteProfile(profile.name.clone()))));
            }

            profiles_list = profiles_list.push(
                container(
                    column![
                        row![
                            column![
                                row![
                                    text(&profile.name).size(15).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                                    Space::new().width(theme::SP_SM),
                                    source_badge,
                                    if is_active { Space::new().width(theme::SP_SM) } else { Space::new().width(0) },
                                    if is_active { Element::from(badge(lang.tr("active_tag").trim().to_string(), BadgeKind::Success)) } else { Element::from(Space::new().width(0)) },
                                ].align_y(Alignment::Center),
                                row![
                                    kbd_badge(if is_subscription { "SUB" } else { "YAML" }),
                                    Space::new().width(theme::SP_XS),
                                    text(profile.path.clone()).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) }),
                                ].align_y(Alignment::Center),
                            ].spacing(theme::SP_XS).width(Length::Fill),
                            actions,
                        ].align_y(Alignment::Center),
                        if is_subscription {
                            let last_up = format_datetime(profile.last_updated, lang.tr("profiles_never").as_ref());
                            let mut sub_details = column![row![text(format!("{} {}", lang.tr("profiles_last_updated"), last_up)).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) })].align_y(Alignment::Center)].spacing(theme::SP_XS);
                            if let Some(traffic_elem) = traffic_row(profile, &lang) {
                                sub_details = sub_details.push(traffic_elem);
                            }
                            Element::from(sub_details)
                        } else {
                            Element::from(Space::new().width(0).height(0))
                        },
                    ].spacing(theme::SP_XS),
                )
                .padding(SP_MD)
                .width(Length::Fill)
                .style(move |t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(if is_active { Color { a: 0.03, ..tk.accent } } else { tk.card_bg }.into()),
                        border: Border {
                            radius: border::Radius::from(R_CARD),
                            width: if is_active { 2.0 } else { 1.0 },
                            color: if is_active { tk.accent } else { tk.card_border },
                        },
                        shadow: tk.card_shadow,
                        ..Default::default()
                    }
                }),
            );
        }
    }

    let profiles_section = card(
        None,
        column![
            section_header("PROFILES", Some(text(format!("{}", state.profile.profiles.len())).size(12).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) }).into())),
            Space::new().height(theme::SP_MD),
            profiles_list,
        ],
    );

    let mut content_items: Vec<Element<'_, Message>> = vec![header.into(), Space::new().height(theme::SP_LG).into()];
    if let Some(alert) = active_alert {
        content_items.push(alert);
        content_items.push(Space::new().height(SP_MD).into());
    }
    content_items.push(import_section);
    content_items.push(Space::new().height(SP_MD).into());
    content_items.push(local_import_section);
    content_items.push(Space::new().height(SP_MD).into());
    content_items.push(subscription_section);
    content_items.push(Space::new().height(theme::SP_MD).into());
    content_items.push(crate::view::sub_quota_card::sub_quota_card(state, &lang));
    content_items.push(Space::new().height(theme::SP_LG).into());
    content_items.push(profiles_section);
    content_items.push(Space::new().height(theme::SP_XL).into());

    let content = column(content_items).spacing(10);
    modern_scrollable(content).height(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_scale() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
        assert_eq!(format_bytes(1099511627776), "1.00 TB");
    }

    #[test]
    fn test_format_datetime_fallback() {
        assert_eq!(format_datetime(None, "Never"), "Never");
        let fixed = DateTime::from_timestamp(1700000000, 0).unwrap();
        let formatted = format_datetime(Some(fixed), "Never");
        assert!(!formatted.is_empty());
        assert_ne!(formatted, "Never");
    }

    #[test]
    fn test_traffic_row_none_without_info() {
        let p = ProfileInfo {
            name: "test".to_string(),
            path: "/tmp/test.yaml".to_string(),
            ..Default::default()
        };
        assert!(traffic_row(&p, &Lang("zh-CN")).is_none());
        assert!(traffic_row(&p, &Lang("en-US")).is_none());
    }

    #[test]
    fn test_traffic_row_with_quota_and_expire() {
        let mut p = ProfileInfo {
            name: "sub".to_string(),
            path: "/tmp/sub.yaml".to_string(),
            active: true,
            ..Default::default()
        };
        p.subscription_url = Some("https://example.com/sub".to_string());
        p.traffic_upload = Some(1024 * 1024 * 100);
        p.traffic_download = Some(1024 * 1024 * 900);
        p.traffic_total = Some(1024 * 1024 * 1000);
        p.expire_at = Some(1900000000);

        assert!(traffic_row(&p, &Lang("zh-CN")).is_some());
        assert!(traffic_row(&p, &Lang("en-US")).is_some());
    }

    #[test]
    fn test_traffic_row_threshold_tiers() {
        let mut p = ProfileInfo {
            name: "sub".to_string(),
            path: "/tmp/sub.yaml".to_string(),
            active: true,
            ..Default::default()
        };
        p.subscription_url = Some("https://example.com/sub".to_string());
        p.traffic_total = Some(1000);

        p.traffic_download = Some(300);
        assert!(traffic_row(&p, &Lang("zh-CN")).is_some());

        p.traffic_download = Some(650);
        assert!(traffic_row(&p, &Lang("zh-CN")).is_some());

        p.traffic_download = Some(850);
        assert!(traffic_row(&p, &Lang("zh-CN")).is_some());

        p.traffic_download = Some(950);
        assert!(traffic_row(&p, &Lang("zh-CN")).is_some());

        p.traffic_download = Some(1050);
        assert!(traffic_row(&p, &Lang("zh-CN")).is_some());
    }

    #[test]
    fn test_ua_preset_chip_widget() {
        let _chip1: Element<'_, Message> = ua_preset_chip("Clash.Meta", "Clash.Meta");
        let _chip2: Element<'_, Message> = ua_preset_chip("ClashVerge", "Clash.Meta");
        let _chip3: Element<'_, Message> = ua_preset_chip("Shadowrocket", "");
    }
}
