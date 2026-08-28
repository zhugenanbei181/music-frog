use crate::locales::{Lang, Localizer};
use crate::view::components::card;
use crate::{AppState, Message};
use iced::widget::{Space, button, checkbox, column, container, row, text, text_input};
use iced::{Alignment, Element, Font, Length};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };

    let header = text(lang.tr("sync_title")).size(24).font(bold_font);

    let save_settings_btn: Element<'_, Message> = if state.is_saving_app_settings {
        button(text("Saving...").size(12))
            .padding([8, 16])
            .style(button::secondary)
            .into()
    } else {
        button(text("Save Settings").size(12))
            .on_press(Message::SaveAppSettings)
            .padding([8, 16])
            .style(button::secondary)
            .into()
    };

    let sync_form = card(column![
        checkbox(state.webdav_enabled)
            .label("Enable WebDAV auto sync")
            .on_toggle(Message::UpdateWebDavEnabled)
            .size(16),
        Space::new().height(15),
        column![
            text(lang.tr("sync_url")).size(14).font(bold_font),
            text_input("https://dav.example.com", &state.webdav_url)
                .on_input(Message::UpdateWebDavUrl)
                .padding(10)
                .size(14),
        ]
        .spacing(8),
        Space::new().height(15),
        row![
            column![
                text(lang.tr("sync_user")).size(14).font(bold_font),
                text_input(lang.tr("sync_user").as_ref(), &state.webdav_user)
                    .on_input(Message::UpdateWebDavUser)
                    .padding(10)
                    .size(14),
            ]
            .width(Length::FillPortion(1))
            .spacing(8),
            Space::new().width(20),
            column![
                text(lang.tr("sync_pass")).size(14).font(bold_font),
                text_input(lang.tr("sync_pass").as_ref(), &state.webdav_pass)
                    .on_input(Message::UpdateWebDavPass)
                    .padding(10)
                    .size(14)
                    .secure(true),
            ]
            .width(Length::FillPortion(1))
            .spacing(8),
        ],
        Space::new().height(15),
        row![
            column![
                text("Sync Interval (mins)").size(14).font(bold_font),
                text_input("60", &state.webdav_sync_interval_mins)
                    .on_input(Message::UpdateWebDavSyncInterval)
                    .padding(10)
                    .size(14),
            ]
            .width(Length::FillPortion(1))
            .spacing(8),
            Space::new().width(20),
            column![
                text("Startup Behavior").size(14).font(bold_font),
                checkbox(state.webdav_sync_on_startup)
                    .label("Sync on startup")
                    .on_toggle(Message::UpdateWebDavSyncOnStartup)
                    .size(16),
            ]
            .width(Length::FillPortion(1))
            .spacing(8),
        ],
        Space::new().height(15),
        save_settings_btn,
        Space::new().height(25),
        row![
            button(
                container(text(lang.tr("sync_upload")).size(12))
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .padding(10)
            )
            .on_press(Message::SyncUpload)
            .width(Length::FillPortion(1))
            .style(button::secondary),
            Space::new().width(20),
            button(
                container(text(lang.tr("sync_download")).size(12))
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .padding(10)
            )
            .on_press(Message::SyncDownload)
            .width(Length::FillPortion(1))
            .style(button::primary),
        ]
    ]);

    let content = column![
        header,
        Space::new().height(20),
        sync_form,
        Space::new().height(20),
        if state.is_syncing {
            Element::from(text("Syncing...").size(14))
        } else {
            Element::from(Space::new().width(0).height(0))
        }
    ]
    .spacing(10);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
