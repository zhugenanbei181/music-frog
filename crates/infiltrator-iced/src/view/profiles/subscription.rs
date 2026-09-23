//! Subscription scheduler settings card.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::view::components::{
    card, form_input_style, form_pick_style, form_toggle_row, segmented_control, style_accent,
    style_ghost, text_btn,
};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, column, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

use super::helpers::{format_datetime, ua_preset_chip};

pub(super) fn subscription_section<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let profile_options: Vec<String> = state
        .profile
        .profiles
        .iter()
        .map(|p| p.name.clone())
        .collect();
    let selected_profile = if state.profile.subscription_profile_name.is_empty() {
        None
    } else {
        Some(&state.profile.subscription_profile_name)
    };
    let selected_profile_meta = state
        .profile
        .profiles
        .iter()
        .find(|p| p.name == state.profile.subscription_profile_name);
    let interval_options: Vec<String> = ["12", "24", "48", "168"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let selected_interval = if state
        .profile
        .subscription_update_interval_hours
        .trim()
        .is_empty()
    {
        Some("24".to_string())
    } else {
        Some(state.profile.subscription_update_interval_hours.clone())
    };

    let subscription_save_action: Element<'_, Message> = if state.profile.is_saving_subscription {
        text_btn(
            lang.tr("profiles_saving_subscription").to_string(),
            style_accent,
            None,
        )
    } else {
        text_btn(
            lang.tr("profiles_save_subscription").to_string(),
            style_accent,
            Some(Message::SaveSubscriptionSettings),
        )
    };

    let subscription_update_now_action: Element<'_, Message> =
        if state.profile.is_updating_subscription_now {
            text_btn(
                lang.tr("profiles_updating_subscription").to_string(),
                style_ghost,
                None,
            )
        } else {
            text_btn(
                lang.tr("profiles_update_now").to_string(),
                style_ghost,
                Some(Message::UpdateSubscriptionNow),
            )
        };

    let interval_labels: Vec<String> = ["12h", "24h", "48h", "168h"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let interval_selected = ["12", "24", "48", "168"]
        .iter()
        .position(|h| Some(h.to_string()) == selected_interval)
        .unwrap_or(usize::MAX);
    let interval_control = segmented_control(&interval_labels, interval_selected, |index| {
        let hours = match index {
            1 => "24",
            2 => "48",
            3 => "168",
            _ => "12",
        };
        Message::UpdateSubscriptionInterval(hours.to_string())
    });

    let ua_presets = row![
        text(lang.tr("profiles_ua_preset").to_string())
            .size(11)
            .font(FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().width(theme::SP_XS),
        ua_preset_chip("Clash.Meta", &state.profile.subscription_user_agent),
        Space::new().width(theme::SP_XS),
        ua_preset_chip("ClashVerge", &state.profile.subscription_user_agent),
        Space::new().width(theme::SP_XS),
        ua_preset_chip("Shadowrocket", &state.profile.subscription_user_agent),
    ]
    .spacing(theme::SP_XS)
    .align_y(Alignment::Center);

    card(
        Some(lang.tr("profiles_subscription_settings_title").to_string()),
        column![
            pick_list(
                profile_options,
                selected_profile,
                Message::SelectSubscriptionProfile
            )
            .placeholder(lang.tr("profiles_select_profile").as_ref())
            .width(Length::Fill)
            .style(form_pick_style),
            Space::new().height(theme::SP_MD),
            text_input(
                lang.tr("profiles_subscription_url").as_ref(),
                &state.profile.subscription_url
            )
            .on_input(Message::UpdateSubscriptionUrl)
            .padding([8, 12])
            .size(13)
            .width(Length::Fill)
            .style(form_input_style),
            Space::new().height(theme::SP_SM),
            text_input(
                lang.tr("profiles_user_agent_placeholder").as_ref(),
                &state.profile.subscription_user_agent
            )
            .on_input(Message::UpdateSubscriptionUserAgent)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .width(Length::Fill)
            .style(form_input_style),
            Space::new().height(theme::SP_XS),
            ua_presets,
            Space::new().height(theme::SP_SM),
            form_toggle_row(
                lang.tr("profiles_insecure_skip_verify").to_string(),
                state.profile.subscription_insecure_skip_verify,
                Message::UpdateSubscriptionInsecureSkipVerify
            ),
            Space::new().height(theme::SP_XS),
            text(lang.tr("profiles_insecure_skip_verify_hint").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary)
                }),
            Space::new().height(theme::SP_SM),
            form_toggle_row(
                lang.tr("profiles_auto_reload_core").to_string(),
                state.profile.subscription_auto_reload_core,
                Message::UpdateSubscriptionAutoReload
            ),
            Space::new().height(theme::SP_XS),
            text(lang.tr("profiles_auto_reload_core_hint").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary)
                }),
            Space::new().height(theme::SP_SM),
            if let Some(profile) = selected_profile_meta {
                let conditional = if profile.etag.is_some() || profile.last_modified.is_some() {
                    format!(
                        "{} · ETag: {} · Last-Modified: {}",
                        lang.tr("profiles_conditional_request"),
                        profile.etag.as_deref().unwrap_or("-"),
                        profile.last_modified.as_deref().unwrap_or("-")
                    )
                } else {
                    lang.tr("profiles_conditional_request_empty").into_owned()
                };
                Element::from(text(conditional).size(11).font(MONO).style(|t: &Theme| {
                    text::Style {
                        color: Some(tokens(t).text_secondary),
                    }
                }))
            } else {
                Element::from(Space::new().width(0))
            },
            Space::new().height(theme::SP_SM),
            if let Some(profile) = selected_profile_meta {
                let backup_text = if profile.has_backup {
                    lang.tr("profiles_backup_available").to_string()
                } else {
                    lang.tr("profiles_backup_none").to_string()
                };
                let restore_action: Element<'_, Message> = if profile.has_backup {
                    text_btn(
                        lang.tr("profiles_restore_backup").to_string(),
                        style_ghost,
                        Some(Message::RestoreSubscriptionBackup),
                    )
                } else {
                    text_btn(
                        lang.tr("profiles_restore_backup").to_string(),
                        style_ghost,
                        None,
                    )
                };
                Element::from(
                    row![
                        text(backup_text)
                            .size(11)
                            .font(MONO)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary)
                            }),
                        Space::new().width(theme::SP_MD),
                        restore_action,
                        Space::new().width(Length::Fill),
                    ]
                    .align_y(Alignment::Center),
                )
            } else {
                Element::from(Space::new().width(0))
            },
            Space::new().height(theme::SP_MD),
            form_toggle_row(
                lang.tr("profiles_auto_update").to_string(),
                state.profile.subscription_auto_update_enabled,
                Message::UpdateSubscriptionAutoUpdate
            ),
            Space::new().height(theme::SP_SM),
            row![
                pick_list(
                    interval_options,
                    selected_interval.clone(),
                    Message::UpdateSubscriptionInterval
                )
                .placeholder(lang.tr("profiles_update_interval").as_ref())
                .text_size(13)
                .width(Length::Fixed(180.0))
                .style(form_pick_style),
                Space::new().width(theme::SP_MD),
                interval_control,
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            text_input(
                lang.tr("profiles_cron_placeholder").as_ref(),
                &state.profile.subscription_cron_expression
            )
            .on_input(Message::UpdateSubscriptionCron)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .width(Length::Fill)
            .style(form_input_style),
            Space::new().height(theme::SP_XS),
            text(lang.tr("profiles_cron_hint").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary)
                }),
            if let Some(profile) = selected_profile_meta {
                Element::from(
                    row![
                        text(format!(
                            "{} {}",
                            lang.tr("profiles_last_updated"),
                            format_datetime(
                                profile.last_updated,
                                lang.tr("profiles_never").as_ref()
                            )
                        ))
                        .size(12)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary)
                        }),
                        Space::new().width(theme::SP_MD),
                        text(format!(
                            "{} {}",
                            lang.tr("profiles_next_update"),
                            format_datetime(
                                profile.next_update,
                                lang.tr("profiles_not_scheduled").as_ref()
                            )
                        ))
                        .size(12)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary)
                        }),
                    ]
                    .align_y(Alignment::Center),
                )
            } else {
                Element::from(Space::new().width(0))
            },
            Space::new().height(theme::SP_MD),
            row![
                subscription_save_action,
                Space::new().width(theme::SP_MD),
                subscription_update_now_action,
                Space::new().width(theme::SP_MD),
                text_btn(
                    lang.tr("profiles_open_overlay").to_string(),
                    style_ghost,
                    selected_profile_meta.and_then(|p| (!p.path.is_empty()).then_some(
                        Message::EditProfileAs(p.path.clone().into(), EditorPane::Mixin)
                    ))
                ),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
