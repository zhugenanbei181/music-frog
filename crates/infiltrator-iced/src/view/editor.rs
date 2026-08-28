use crate::locales::{Lang, Localizer};
use crate::view::icons;
use crate::{AppState, Message};
use iced::widget::{button, column, container, row, text, text_editor};
use iced::{Alignment, Element, Font, Length};
use std::path::PathBuf;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };

    let filename = state
        .editor_path
        .as_ref()
        .and_then(|p: &PathBuf| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled");

    let header = row![
        text(filename).size(20).font(bold_font),
        iced::widget::Space::new().width(Length::Fill),
        button(
            row![
                text(icons::SAVE).size(12),
                text(lang.tr("btn_save")).size(12)
            ]
            .spacing(8)
        )
        .on_press(Message::SaveProfile)
        .padding([6, 12])
        .style(button::primary),
        iced::widget::Space::new().width(10),
        button(
            row![
                text(icons::CANCEL).size(12),
                text(lang.tr("btn_cancel")).size(12)
            ]
            .spacing(8)
        )
        .on_press(Message::Navigate(crate::types::Route::Profiles))
        .padding([6, 12])
        .style(button::secondary),
    ]
    .align_y(Alignment::Center);

    let editor = text_editor(&state.editor_content)
        .on_action(Message::EditorAction)
        .padding(10)
        .height(Length::Fill);

    let content = column![header, iced::widget::Space::new().height(10), editor].spacing(10);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(20)
        .into()
}
