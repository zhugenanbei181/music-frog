//! Full native Hosts row editor with immutable applied facts and correlated write feedback.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent, style_ghost};
use crate::view::theme::{FONT_SEMIBOLD, MONO, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length, Theme};
use infiltrator_application::dns_hosts_projection::{feedback, legacy_hint};
use infiltrator_shared::locales::{Lang, Localizer};
pub fn hosts_editor_modal(state: &AppState) -> Element<'_, Message> {
    let editor = &state.editor.dns_hosts_editor;
    let lang = Lang(&state.shell.lang);
    let pending = editor.pending.is_some();
    let input = row![
        column![
            text(lang.tr("dns_hosts_address")).size(11),
            container(
                text_input("192.168.1.1", &editor.address)
                    .on_input_maybe((!pending).then_some(Message::UpdateDnsHostsAddress))
                    .padding(8)
                    .size(12)
                    .style(form_input_style)
            )
            .id(InteractionRegion::HostsAddress.id())
        ]
        .width(Length::FillPortion(2)),
        column![
            text(lang.tr("dns_hosts_domain")).size(11),
            container(
                text_input("router.lan", &editor.domain)
                    .on_input_maybe((!pending).then_some(Message::UpdateDnsHostsDomain))
                    .padding(8)
                    .size(12)
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
                text(draft.entry.address.clone())
                    .size(11)
                    .font(MONO)
                    .width(Length::FillPortion(2)),
                text(draft.entry.domain.clone())
                    .size(11)
                    .width(Length::FillPortion(3)),
                button(text(lang.tr("dns_hosts_edit")).size(11))
                    .style(style_ghost)
                    .on_press_maybe((!pending).then_some(Message::EditDnsHostRow(draft.id))),
                button(text(lang.tr("dns_hosts_remove")).size(11))
                    .style(style_ghost)
                    .on_press_maybe((!pending).then_some(Message::RemoveDnsHostRow(draft.id))),
            ]
            .spacing(6),
        );
    }
    if editor.rows.is_empty() {
        rows = rows.push(text(lang.tr("dns_hosts_empty")).size(11));
    }
    let can_import = !pending
        && editor.editing.is_none()
        && editor.address.is_empty()
        && editor.domain.is_empty()
        && !editor.importing_legacy
        && editor
            .applied
            .as_ref()
            .is_some_and(|profile| !profile.legacy_entries.is_empty());
    let mut content = column![
        text(lang.tr("dns_hosts_title"))
            .size(16)
            .font(FONT_SEMIBOLD),
        text(lang.tr("dns_hosts_desc")).size(11),
        input,
        row![
            button(
                text(lang.tr(if editor.editing.is_some() {
                    "dns_hosts_commit_row"
                } else {
                    "dns_hosts_add"
                }))
                .size(11)
            )
            .style(style_ghost)
            .on_press_maybe((!pending).then_some(Message::AddDnsHostRow)),
            button(text(lang.tr("dns_hosts_cancel_row")).size(11))
                .style(style_ghost)
                .on_press_maybe((!pending).then_some(Message::CancelDnsHostRowInput)),
            Space::new().width(Length::Fill),
            button(text(lang.tr("dns_hosts_import_legacy")).size(11))
                .style(style_ghost)
                .on_press_maybe(can_import.then_some(Message::ImportLegacyDnsHosts)),
        ]
        .spacing(8),
        container(scrollable(rows))
            .height(88)
            .id(InteractionRegion::HostsRows.id()),
    ]
    .spacing(8);
    let hint = legacy_hint(editor, &state.shell.lang);
    if !hint.is_empty() {
        content = content.push(text(hint).size(11));
    }
    content = content.push(text(feedback(editor, &state.shell.lang)).size(11).style(
        |theme: &Theme| text::Style {
            color: Some(tokens(theme).danger),
        },
    ));
    content = content.push(
        row![
            button(text(lang.tr("dns_hosts_clear_draft")).size(11))
                .style(style_ghost)
                .on_press_maybe((!pending).then_some(Message::CancelDnsHostsEditor)),
            Space::new().width(Length::Fill),
            container(
                button(
                    text(lang.tr(if pending {
                        "dns_hosts_saving"
                    } else if editor.failure.is_some() {
                        "dns_hosts_retry"
                    } else {
                        "dns_hosts_apply"
                    }))
                    .size(11)
                )
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
