//! Confirmation modal and its per-action copy.

use super::card::{modal_backdrop, modal_card};
use super::rule_statistics::statistics_confirmation;
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use crate::types::rule_trace::RuleTraceAction;
use crate::view::component_forms::{style_danger, style_ghost};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, SP_SM, SP_XS, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::rule_condition_projection::location_text;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn confirmation_modal<'a>(
    state: &'a AppState,
    action: &'a ConfirmAction,
) -> Element<'a, Message> {
    if *action == ConfirmAction::RuleStatisticsCleanup {
        return statistics_confirmation(state);
    }
    let lang = Lang(&state.shell.lang);
    let (title, detail, confirm_label) = confirmation_copy(action, &lang);
    let cancel_label = lang.tr("modal_cancel");
    let pending = matches!(action, ConfirmAction::TracerOverride(_))
        && state.editor.rule_trace.override_pending.is_some();

    let mut actions = row![];
    let guide = matches!(action, ConfirmAction::TracerOverride(_))
        && state
            .editor
            .rule_trace
            .override_failure
            .as_ref()
            .is_some_and(|failure| {
                matches!(
                    failure.code,
                    ErrorCode::Permission | ErrorCode::Authentication
                )
            });
    if guide {
        actions = actions.push(
            button(text(lang.tr("dns_query_settings").to_string())).on_press_maybe(
                (!pending).then_some(Message::RuleTrace(RuleTraceAction::Settings)),
            ),
        );
    }

    let content = column![
        row![
            text(title)
                .size(16)
                .font(FONT_SEMIBOLD)
                .style(|theme: &Theme| text::Style {
                    color: Some(tokens(theme).text_primary),
                }),
            Space::new().width(Length::Fill),
            button(icon_themed(Icon::X, 14.0, |t: &Theme| tokens(t).text_secondary))
                .padding(4)
                .style(style_ghost)
                .on_press_maybe((!pending).then_some(Message::CancelConfirmation)),
        ]
        .align_y(Alignment::Center),
        Space::new().height(SP_XS),
        container(text(detail).size(13).style(|theme: &Theme| text::Style {
            color: Some(tokens(theme).text_secondary),
        }))
        .id(InteractionRegion::ConfirmationDetails.id()),
        text(if pending {
            lang.tr("rule_trace_apply_running").to_string()
        } else if matches!(action, ConfirmAction::TracerOverride(_)) {
            state
                .editor
                .rule_trace
                .override_failure
                .as_ref()
                .map(|failure| failure.message.clone())
                .unwrap_or_default()
        } else {
            String::new()
        })
        .size(12),
        Space::new().height(SP_SM),
        actions
            .extend([
                container(
                    button(text(cancel_label).size(12).font(FONT_MEDIUM))
                        .padding([7, 14])
                        .style(style_ghost)
                        .on_press_maybe((!pending).then_some(Message::CancelConfirmation))
                )
                .id(InteractionRegion::ConfirmationCancel.id())
                .into(),
                Space::new().width(Length::Fill).into(),
                container(
                    button(text(confirm_label).size(12).font(FONT_MEDIUM))
                        .padding([7, 16])
                        .style(style_danger)
                        .on_press_maybe((!pending).then_some(Message::ConfirmAction))
                )
                .id(InteractionRegion::ConfirmationAccept.id())
                .into(),
            ])
            .align_y(Alignment::Center),
    ]
    .spacing(12);

    modal_backdrop(modal_card(
        container(content)
            .id(InteractionRegion::Confirmation.id())
            .into(),
        420.0,
    ))
}

fn confirmation_copy(action: &ConfirmAction, lang: &Lang<'_>) -> (String, String, String) {
    match action {
        ConfirmAction::RuleStatisticsCleanup => (
            lang.tr("rules_stats_cleanup_title").into_owned(),
            lang.tr("rules_stats_cleanup_description").into_owned(),
            lang.tr("rules_stats_cleanup_confirm").into_owned(),
        ),
        ConfirmAction::TracerOverride(request) => (
            lang.tr("rule_trace_confirm_title").to_string(),
            format!(
                "{}\n{}",
                location_text(
                    &RuleLocation {
                        table: request.rule_table.clone(),
                        index: request.rule_index
                    },
                    lang.0
                ),
                interpolate(
                    &lang.tr("rule_trace_confirm_detail"),
                    &[
                        ("profile", &request.expected_source.profile),
                        ("rule", &request.expected_rule),
                        ("target", &request.new_target),
                    ],
                )
            ),
            lang.tr("rule_trace_confirm_apply").to_string(),
        ),
        ConfirmAction::FactoryReset => (
            lang.tr("modal_confirm_factory_title").to_string(),
            lang.tr("modal_confirm_factory_desc").to_string(),
            lang.tr("modal_confirm_factory_btn").to_string(),
        ),
        ConfirmAction::ClearProfiles => (
            lang.tr("modal_confirm_reset_title").to_string(),
            lang.tr("modal_confirm_reset_desc").to_string(),
            lang.tr("modal_confirm_reset_btn").to_string(),
        ),
        ConfirmAction::DeleteProfile(name) => (
            lang.tr("modal_confirm_del_profile_title").to_string(),
            interpolate(
                &lang.tr("modal_confirm_del_profile_desc"),
                &[("name", name)],
            ),
            lang.tr("modal_delete").to_string(),
        ),
        ConfirmAction::DeleteKernel(version) => (
            lang.tr("modal_confirm_del_kernel_title").to_string(),
            interpolate(
                &lang.tr("modal_confirm_del_kernel_desc"),
                &[("version", version)],
            ),
            lang.tr("modal_delete").to_string(),
        ),
        ConfirmAction::CloseAllConnections => (
            lang.tr("modal_confirm_disconnect_all_title").to_string(),
            lang.tr("modal_confirm_disconnect_all_desc").to_string(),
            lang.tr("modal_confirm_disconnect_all_btn").to_string(),
        ),
    }
}
