//! Profiles page toolbar and import cards (remote subscription + local YAML).

use crate::state::AppState;
use crate::types::app::{ConfirmAction, ToastStatus};
use crate::types::message::Message;
use crate::view::component_forms::{
    banner_alert, form_field_label, form_input_style, form_toggle_row, search_input, style_accent,
    style_danger, style_ghost, text_btn,
};
use crate::view::components::{BadgeKind, card};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

use super::helpers::read_clipboard_url;

pub(super) fn header<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let clear_profiles_btn: Element<'_, Message> = if state.profile.is_loading_profiles {
        text_btn(lang.tr("profiles_clearing").to_string(), style_danger, None)
    } else {
        text_btn(
            lang.tr("profiles_clear_all").to_string(),
            style_danger,
            Some(Message::RequestConfirmation(ConfirmAction::ClearProfiles)),
        )
    };

    let search_box = container(search_input(
        lang.tr("profiles_search_placeholder").as_ref(),
        &state.profile.profiles_filter,
        Message::UpdateProfilesFilter,
        Message::UpdateProfilesFilter(String::new()),
    ))
    .width(Length::Fill)
    .max_width(360.0);

    // DUAL-07-11: toolbar entry that refreshes every subscription at once.
    let update_all_btn: Element<'_, Message> = if state.profile.is_updating_subscription_now {
        text_btn(
            lang.tr("profiles_updating_subscription").to_string(),
            style_ghost,
            None,
        )
    } else {
        text_btn(
            lang.tr("profiles_update_all").to_string(),
            style_ghost,
            Some(Message::UpdateAllSubscriptionsNow),
        )
    };

    let header = row![
        text(lang.tr("profiles_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            }),
        Space::new().width(theme::SP_LG),
        search_box,
        Space::new().width(theme::SP_SM),
        update_all_btn,
        Space::new().width(theme::SP_SM),
        clear_profiles_btn,
        Space::new().width(theme::SP_SM),
        button(
            row![
                svg_icons::icon_themed(Icon::LayoutGrid, 13.0, |t: &Theme| tokens(t)
                    .text_secondary),
                Space::new().width(theme::SP_XS),
                text(lang.tr("aggregator_title").to_string()).size(12),
            ]
            .align_y(Alignment::Center)
        )
        .padding([6, 10])
        .style(style_ghost)
        .on_press(Message::OpenAggregatorModal),
        Space::new().width(Length::Fill),
        text_btn(
            lang.tr("profiles_open_folder").to_string(),
            style_ghost,
            Some(Message::OpenConfigDir)
        ),
    ]
    .align_y(Alignment::Center);
    header.into()
}

pub(super) fn active_alert<'a>(state: &'a AppState) -> Option<Element<'a, Message>> {
    let lang = Lang(&state.shell.lang);
    let active_alert: Option<Element<'_, Message>> =
        if let Some(p) = state.profile.profiles.iter().find(|p| p.active) {
            let now = chrono::Utc::now().timestamp();
            let is_expired = p.expire_at.is_some_and(|exp| exp > 0 && exp <= now);
            let is_expiring_soon = p
                .expire_at
                .is_some_and(|exp| exp > now && exp - now < 3 * 86400);
            let total = p.traffic_total.unwrap_or(0);
            let used = p
                .traffic_upload
                .unwrap_or(0)
                .saturating_add(p.traffic_download.unwrap_or(0));
            let fraction = if total > 0 {
                (used as f32 / total as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let is_exhausted = total > 0 && fraction >= 0.80;

            if is_expired {
                Some(banner_alert(
                    BadgeKind::Danger,
                    lang.tr("profiles_active_expired_title").to_string(),
                    lang.tr("profiles_active_expired_desc").to_string(),
                    Some(text_btn(
                        lang.tr("profiles_update_now").to_string(),
                        style_ghost,
                        Some(Message::UpdateSubscriptionNow),
                    )),
                ))
            } else if is_expiring_soon {
                Some(banner_alert(
                    BadgeKind::Warning,
                    lang.tr("profiles_active_expiring_title").to_string(),
                    lang.tr("profiles_active_expiring_desc").to_string(),
                    Some(text_btn(
                        lang.tr("profiles_update_now").to_string(),
                        style_ghost,
                        Some(Message::UpdateSubscriptionNow),
                    )),
                ))
            } else if is_exhausted {
                Some(banner_alert(
                    BadgeKind::Warning,
                    lang.tr("profiles_active_exhausted_title").to_string(),
                    format!(
                        "{}: {:.1}%",
                        lang.tr("profiles_used_traffic"),
                        fraction * 100.0
                    ),
                    Some(text_btn(
                        lang.tr("profiles_update_now").to_string(),
                        style_ghost,
                        Some(Message::UpdateSubscriptionNow),
                    )),
                ))
            } else {
                None
            }
        } else {
            None
        };
    active_alert
}

pub(super) fn import_section<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let paste_msg = if let Some(url) = read_clipboard_url() {
        Message::UpdateImportUrl(url)
    } else {
        Message::ShowToast(
            lang.tr("profiles_no_sub_in_clipboard").to_string(),
            ToastStatus::Warning,
        )
    };

    let paste_btn = button(
        row![
            svg_icons::icon_themed(Icon::Copy, 13.0, |t: &Theme| tokens(t).text_secondary),
            text(lang.tr("profiles_paste").to_string())
                .size(12)
                .font(FONT_MEDIUM)
        ]
        .spacing(theme::SP_XS)
        .align_y(Alignment::Center),
    )
    .padding([7, 12])
    .style(style_ghost)
    .on_press(paste_msg);

    let import_actions: Element<'_, Message> = if state.profile.is_importing {
        text_btn(
            lang.tr("profiles_importing").to_string(),
            style_accent,
            None,
        )
    } else {
        button(
            row![
                svg_icons::icon_themed(Icon::Plus, 14.0, |t: &Theme| tokens(t).on_accent),
                text(lang.tr("profiles_import_btn").to_string())
                    .size(12)
                    .font(FONT_MEDIUM)
            ]
            .spacing(theme::SP_SM)
            .align_y(Alignment::Center),
        )
        .padding([7, 14])
        .style(style_accent)
        .on_press(Message::ImportProfile)
        .into()
    };

    card(
        Some(lang.tr("profiles_import_sub").to_string()),
        column![
            row![
                column![
                    form_field_label(lang.tr("profiles_import_name_placeholder").to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input(
                        lang.tr("profiles_import_name_placeholder").as_ref(),
                        &state.profile.import_name
                    )
                    .on_input(Message::UpdateImportName)
                    .padding([8, 12])
                    .size(13)
                    .style(form_input_style)
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![
                    form_field_label(lang.tr("profiles_sub_url").to_string()),
                    Space::new().height(theme::SP_XS),
                    row![
                        text_input(
                            lang.tr("profiles_sub_url").as_ref(),
                            &state.profile.import_url
                        )
                        .on_input(Message::UpdateImportUrl)
                        .padding([8, 12])
                        .size(13)
                        .width(Length::Fill)
                        .style(form_input_style),
                        Space::new().width(theme::SP_XS),
                        paste_btn
                    ]
                    .align_y(Alignment::Center)
                ]
                .width(Length::FillPortion(2))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![Space::new().height(18.0), import_actions].spacing(theme::SP_XS),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_MD),
            form_toggle_row(
                lang.tr("profiles_import_activate").to_string(),
                state.profile.import_activate,
                Message::UpdateImportActivate
            ),
        ],
    )
}

pub(super) fn local_import_section<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let local_import_action: Element<'_, Message> = if state.profile.is_importing_local {
        text_btn(
            lang.tr("profiles_importing").to_string(),
            style_accent,
            None,
        )
    } else {
        button(
            row![
                svg_icons::icon_themed(Icon::Plus, 14.0, |t: &Theme| tokens(t).on_accent),
                text(lang.tr("profiles_import_local_btn").to_string())
                    .size(12)
                    .font(FONT_MEDIUM)
            ]
            .spacing(theme::SP_SM),
        )
        .padding([7, 14])
        .style(style_accent)
        .on_press(Message::ImportLocalProfile)
        .into()
    };

    card(
        Some(lang.tr("profiles_local_import_title").to_string()),
        column![
            row![
                column![
                    form_field_label(lang.tr("profiles_local_path_placeholder").to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input(
                        lang.tr("profiles_local_path_placeholder").as_ref(),
                        &state.profile.local_import_path
                    )
                    .on_input(Message::UpdateLocalImportPath)
                    .padding([8, 12])
                    .size(13)
                    .style(form_input_style)
                ]
                .width(Length::FillPortion(2))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![
                    Space::new().height(18.0),
                    text_btn(
                        lang.tr("profiles_browse_btn").to_string(),
                        style_ghost,
                        Some(Message::BrowseLocalImportFile)
                    )
                ]
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![
                    form_field_label(lang.tr("profiles_local_name_placeholder").to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input(
                        lang.tr("profiles_local_name_placeholder").as_ref(),
                        &state.profile.local_import_name
                    )
                    .on_input(Message::UpdateLocalImportName)
                    .padding([8, 12])
                    .size(13)
                    .style(form_input_style)
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![Space::new().height(18.0), local_import_action].spacing(theme::SP_XS),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_MD),
            form_toggle_row(
                lang.tr("profiles_import_activate").to_string(),
                state.profile.local_import_activate,
                Message::UpdateLocalImportActivate
            ),
        ],
    )
}
