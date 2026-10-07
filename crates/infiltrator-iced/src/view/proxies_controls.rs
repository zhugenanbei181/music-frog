//! Native proxy filters, all sort options and editable probe settings.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{icon_button, toggle_switch};
use crate::view::svg_icons;
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, tokens};
use iced::advanced::widget::Id;
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

/// Sort options already understood by `Message::UpdateProxyDelaySort`.
pub(super) const SORT_KEYS: [&str; 4] = ["delay_asc", "delay_desc", "name_asc", "name_desc"];
pub(super) const SORT_LABEL_KEYS: [&str; 4] = [
    "runtime_delay_sort_delay_asc",
    "runtime_delay_sort_delay_desc",
    "runtime_delay_sort_name_asc",
    "runtime_delay_sort_name_desc",
];

pub fn controls<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let search_box = row![
        svg_icons::icon_themed(Icon::Search, 14.0, |t: &Theme| tokens(t).text_tertiary),
        text_input(
            lang.tr("proxies_search_placeholder").as_ref(),
            &state.runtime.proxy_filter,
        )
        .on_input(Message::FilterProxies)
        .size(12)
        .padding([5, 9])
        .width(Length::Fixed(180.0))
        .style(delay_input_style),
        if state.runtime.proxy_filter.is_empty() {
            Element::from(Space::new().width(0))
        } else {
            icon_button(Icon::X, 12.0, Message::FilterProxies(String::new()))
        },
    ]
    .spacing(theme::SP_XS)
    .align_y(Alignment::Center);

    let search_box = container(search_box).id(search_region_id());
    let alive_toggle = row![
        text(lang.tr("proxies_filter_alive"))
            .size(11)
            .font(theme::FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().width(theme::SP_XS),
        toggle_switch(state.runtime.filter_alive_only, Message::ToggleFilterAlive),
    ]
    .spacing(theme::SP_XS)
    .align_y(Alignment::Center);

    let mut sort_segment = row![].spacing(2);
    for (index, key) in SORT_KEYS.iter().enumerate() {
        let active = state.runtime.proxy_delay_sort == *key;
        let label = text(lang.tr(SORT_LABEL_KEYS[index]))
            .size(11)
            .font(if active {
                theme::FONT_SEMIBOLD
            } else {
                theme::FONT_MEDIUM
            })
            .style(move |t: &Theme| text::Style {
                color: Some(if active {
                    tokens(t).text_primary
                } else {
                    tokens(t).text_secondary
                }),
            });

        let mut segment = button(container(label).padding([5, 10])).padding(0).style(
            move |t: &Theme, _status| {
                let tk = tokens(t);
                if active {
                    button::Style {
                        background: Some(tk.card_bg.into()),
                        border: Border {
                            radius: border::Radius::from(theme::R_CONTROL),
                            ..Default::default()
                        },
                        shadow: tk.card_shadow,
                        text_color: tk.text_primary,
                        ..Default::default()
                    }
                } else {
                    button::Style {
                        background: None,
                        border: Border {
                            radius: border::Radius::from(theme::R_CONTROL),
                            ..Default::default()
                        },
                        text_color: tk.text_secondary,
                        ..Default::default()
                    }
                }
            },
        );

        if !active {
            segment = segment.on_press(Message::UpdateProxyDelaySort((*key).to_string()));
        }

        sort_segment = sort_segment.push(segment);
    }

    let sort_segment = container(sort_segment)
        .padding(2)
        .style(|t: &Theme| container::Style {
            background: Some(tokens(t).control_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CONTROL),
                width: theme::HAIRLINE,
                color: tokens(t).card_border,
            },
            ..Default::default()
        });

    let filters: Element<'a, Message> = if state.shell.viewport.tier.is_narrow() {
        column![
            row![search_box, Space::new().width(Length::Fill), alive_toggle]
                .spacing(theme::SP_SM)
                .align_y(Alignment::Center),
            sort_segment,
        ]
        .spacing(theme::SP_SM)
        .into()
    } else {
        row![
            search_box,
            alive_toggle,
            Space::new().width(Length::Fill),
            sort_segment
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    };
    let status = if let Some(failure) = &state.runtime.proxy_search.failure {
        row![
            text(interpolate(
                lang.tr("proxies_search_failed").as_ref(),
                &[("reason", failure.message.as_str())]
            ))
            .size(12),
            button(text(lang.tr("proxies_search_retry")))
                .on_press(Message::FilterProxies(state.runtime.proxy_filter.clone()))
        ]
        .spacing(theme::SP_SM)
        .into()
    } else if let Some(failure) = &state.runtime.proxy_preferences.failure {
        Element::from(
            text(interpolate(
                lang.tr("proxies_preferences_failed").as_ref(),
                &[("reason", failure.message.as_str())],
            ))
            .size(12),
        )
    } else if !state.runtime.proxy_filter.trim().is_empty()
        && state
            .runtime
            .proxy_groups
            .iter()
            .all(|group| group.proxies.is_empty())
    {
        Element::from(text(lang.tr("proxies_search_empty")).size(12))
    } else {
        Element::from(text(lang.tr("proxies_search_hint")).size(12))
    };
    column![filters, status, delay_test_group(state, lang)]
        .spacing(theme::SP_SM)
        .width(Length::Fill)
        .into()
}

/// Compact bordered control group for the delay-test endpoint.
fn delay_test_group<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let options = state.runtime.probe_options_editor.applied.as_ref();
    let caption = options
        .map(|options| format!("{} · {} ms", options.test_url, options.timeout_ms))
        .unwrap_or_else(|| lang.tr("proxy_probe_settings_unavailable").into_owned());
    container(
        row![
            button(text(lang.tr("proxy_probe_settings_title")).size(12))
                .on_press(Message::OpenProxyProbeOptions),
            text(caption).size(11).width(Length::Fill),
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center),
    )
    .padding(theme::SP_SM)
    .style(delay_group_surface)
    .into()
}

/// Hairline-bordered control-group surface (tokens, control radius).
fn delay_group_surface(t: &Theme) -> container::Style {
    let tk = tokens(t);
    container::Style {
        background: Some(tk.control_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            width: theme::HAIRLINE,
            color: tk.card_border,
        },
        ..Default::default()
    }
}

/// Token-driven text-input style matching the runtime page's inputs.
fn delay_input_style(t: &Theme, status: text_input::Status) -> text_input::Style {
    let tk = tokens(t);
    let (border_color, border_width) = match status {
        text_input::Status::Focused { .. } => (tk.accent, 1.5),
        _ => (tk.card_border, 1.0),
    };
    text_input::Style {
        background: tk.card_bg.into(),
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            width: border_width,
            color: border_color,
        },
        icon: tk.text_tertiary,
        placeholder: tk.text_tertiary,
        value: tk.text_primary,
        selection: Color {
            a: 0.25,
            ..tk.accent
        },
    }
}

pub fn search_region_id() -> Id {
    Id::new("proxy-search-input")
}
