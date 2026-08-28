use crate::locales::{Lang, Localizer};
use crate::view::components::nav_button;
use crate::{AppState, Message, Route};
use iced::widget::{Space, column, container, image, row, text};
use iced::{Alignment, Color, Element, Length, Theme};
use std::sync::OnceLock;

const LOGO_BYTES: &[u8] = include_bytes!("../../../../src-tauri/icons/icon.png");
static LOGO_HANDLE: OnceLock<image::Handle> = OnceLock::new();

pub fn sidebar(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = iced::Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };
    let logo_handle = LOGO_HANDLE
        .get_or_init(|| image::Handle::from_bytes(LOGO_BYTES))
        .clone();

    let logo = container(
        row![
            image::Image::new(logo_handle).width(32).height(32),
            Space::new().width(12),
            column![
                text("MusicFrog").size(16).font(bold_font),
                text("Infiltrator").size(10).style(|_theme: &Theme| {
                    let is_dark = _theme == &Theme::Dark;
                    text::Style {
                        color: Some(if is_dark {
                            Color::from_rgb(0.4, 0.4, 0.4)
                        } else {
                            Color::from_rgb(0.6, 0.6, 0.6)
                        }),
                    }
                }),
            ]
        ]
        .align_y(Alignment::Center),
    )
    .padding(20);

    container(column![
        logo,
        Space::new().height(10),
        nav_button(
            lang.tr("nav_overview").into_owned(),
            Route::Overview,
            &state.current_route
        ),
        nav_button(
            lang.tr("nav_profiles").into_owned(),
            Route::Profiles,
            &state.current_route
        ),
        nav_button(
            lang.tr("nav_proxies").into_owned(),
            Route::Proxies,
            &state.current_route
        ),
        nav_button(
            lang.tr("nav_runtime").into_owned(),
            Route::Runtime,
            &state.current_route
        ),
        nav_button(
            lang.tr("nav_rules").into_owned(),
            Route::Rules,
            &state.current_route
        ),
        nav_button(
            lang.tr("nav_dns").into_owned(),
            Route::Dns,
            &state.current_route
        ),
        nav_button(
            lang.tr("nav_sync").into_owned(),
            Route::Sync,
            &state.current_route
        ),
        nav_button(
            lang.tr("nav_settings").into_owned(),
            Route::Settings,
            &state.current_route
        ),
        Space::new().height(Length::Fill),
        container(
            text(format!("v{}", env!("CARGO_PKG_VERSION")))
                .size(10)
                .style(|_theme: &Theme| {
                    let is_dark = _theme == &Theme::Dark;
                    text::Style {
                        color: Some(if is_dark {
                            Color::from_rgb(0.4, 0.4, 0.4)
                        } else {
                            Color::from_rgb(0.6, 0.6, 0.6)
                        }),
                    }
                })
        )
        .padding(20)
    ])
    .width(220)
    .height(Length::Fill)
    .style(|_theme: &Theme| {
        let is_dark = _theme == &Theme::Dark;
        container::Style {
            background: Some(
                if is_dark {
                    Color::from_rgb(0.08, 0.08, 0.08)
                } else {
                    Color::from_rgb(0.95, 0.95, 0.95)
                }
                .into(),
            ),
            ..Default::default()
        }
    })
    .into()
}
