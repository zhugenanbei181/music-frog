//! Full native Hosts row editor with immutable applied facts and correlated write feedback.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent, style_ghost};
use crate::view::scrollbar::auto_hide_scrollable;
use crate::view::theme;
use crate::view::theme::tokens;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Element, Length, Theme};
use infiltrator_presentation::dns_hosts_projection::{feedback, legacy_hint};
use infiltrator_shared::locales::{Lang, Localizer};
pub fn hosts_editor_modal(state: &AppState) -> Element<'_, Message> {
    let editor = &state.editor.dns_hosts_editor;
    let lang = Lang(&state.shell.lang);
    let pending = editor.is_pending();
    let input = row![
        column![
            theme::role_body_text(lang.tr("dns_hosts_address")),
            container(
                text_input("192.168.1.1", editor.address())
                    .on_input_maybe((!pending).then_some(Message::UpdateDnsHostsAddress))
                    .padding(8)
                    .style(form_input_style)
            )
            .id(InteractionRegion::HostsAddress.id())
        ]
        .width(Length::FillPortion(2)),
        column![
            theme::role_body_text(lang.tr("dns_hosts_domain")),
            container(
                text_input("router.lan", editor.domain())
                    .on_input_maybe((!pending).then_some(Message::UpdateDnsHostsDomain))
                    .padding(8)
                    .style(form_input_style)
            )
            .id(InteractionRegion::HostsDomain.id())
        ]
        .width(Length::FillPortion(3)),
    ]
    .spacing(8);
    let mut rows = column![].spacing(4);
    for draft in &editor.rows {
        rows = rows.push(
            row![
                theme::role_body_text(draft.entry.address.clone()).width(Length::FillPortion(2)),
                theme::role_body_text(draft.entry.domain.clone()).width(Length::FillPortion(3)),
                button(theme::role_body_text(lang.tr("dns_hosts_edit")))
                    .style(style_ghost)
                    .on_press_maybe((!pending).then_some(Message::EditDnsHostRow(draft.id))),
                button(theme::role_body_text(lang.tr("dns_hosts_remove")))
                    .style(style_ghost)
                    .on_press_maybe((!pending).then_some(Message::RemoveDnsHostRow(draft.id))),
            ]
            .spacing(6),
        );
    }
    if editor.rows.is_empty() {
        rows = rows.push(theme::role_body_text(lang.tr("dns_hosts_empty")));
    }
    let can_import = !pending
        && !editor.is_editing()
        && editor.address().is_empty()
        && editor.domain().is_empty()
        && !editor.is_importing_legacy()
        && editor
            .applied
            .as_ref()
            .is_some_and(|profile| !profile.legacy_entries.is_empty());
    let mut content = column![
        theme::role_body_text(lang.tr("dns_hosts_title")),
        theme::role_body_text(lang.tr("dns_hosts_desc")),
        input,
        row![
            button(theme::role_body_text(lang.tr(if editor.is_editing() {
                "dns_hosts_commit_row"
            } else {
                "dns_hosts_add"
            })))
            .style(style_ghost)
            .on_press_maybe((!pending).then_some(Message::AddDnsHostRow)),
            button(theme::role_body_text(lang.tr("dns_hosts_cancel_row")))
                .style(style_ghost)
                .on_press_maybe((!pending).then_some(Message::CancelDnsHostRowInput)),
            Space::new().width(Length::Fill),
            button(theme::role_body_text(lang.tr("dns_hosts_import_legacy")))
                .style(style_ghost)
                .on_press_maybe(can_import.then_some(Message::ImportLegacyDnsHosts)),
        ]
        .spacing(8),
        container(auto_hide_scrollable(rows))
            .height(88)
            .id(InteractionRegion::HostsRows.id()),
    ]
    .spacing(8);
    let hint = legacy_hint(editor, &Lang(&state.shell.lang));
    if !hint.is_empty() {
        content = content.push(theme::role_body_text(hint));
    }
    content = content.push(
        theme::role_body_text(feedback(editor, &Lang(&state.shell.lang))).style(|theme: &Theme| {
            text::Style {
                color: Some(tokens(theme).danger),
            }
        }),
    );
    content = content.push(
        row![
            button(theme::role_body_text(lang.tr("dns_hosts_clear_draft")))
                .style(style_ghost)
                .on_press_maybe((!pending).then_some(Message::CancelDnsHostsEditor)),
            Space::new().width(Length::Fill),
            container(
                button(theme::role_body_text(lang.tr(if pending {
                    "dns_hosts_saving"
                } else if editor.failure().is_some() {
                    "dns_hosts_retry"
                } else {
                    "dns_hosts_apply"
                })))
                .style(style_accent)
                .on_press_maybe(editor.can_apply().then_some(Message::SaveDnsHosts))
            )
            .id(InteractionRegion::HostsApply.id()),
        ]
        .spacing(8),
    );
    modal_backdrop(
        container(modal_card(content.into(), 600.0))
            .id(InteractionRegion::HostsEditor.id())
            .into(),
    )
}
