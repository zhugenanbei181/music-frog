use crate::view::theme;
use crate::view::theme::font_for;
use infiltrator_contract::design_resources::TextRole;
label, TextRole::Label).font(if is_active {
        theme::font_for(TextRole::LabelStrong)
    } else {
        theme::font_for(TextRole::Label)
    });
    let content = container(
        row![
            indicator,
            glyph,
            label_text,
            Space::new().width(Length::Fill),
            match count {
                Some(badge_value) => quantity_badge(badge_value),
                None => Space::new().width(0).into(),
            },
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([8, 12]);
    button(content)
        .width(Length::Fill)
        .style(move |t: &Theme, status| {
            let tk = theme::tokens(t);
            let mut style = button::Style {
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            };
            if is_active {
                style.background = Some(tk.accent_soft.into());
                style.text_color = tk.accent;
            } else {
                style.background = match status {
                    button::Status::Hovered => Some(tk.hover.into()),
                    button::Status::Pressed => Some(tk.pressed.into()),
                    _ => None,
                };
                style.text_color = match status {
                    button::Status::Hovered | button::Status::Pressed => tk.text_primary,
                    _ => tk.sidebar_text_muted,
                };
            }
            style
        })
        .on_press(Message::Navigate(route))
        .into()
}
/// Compact icon button for the responsive rail sidebar.
pub fn nav_rail_icon<'a>(route: Route, current_route: &Route) -> Element<'a, Message> {
    let is_active = route == *current_route;
    let glyph = icon_themed(nav_icon(route), 18.0, move |t: &Theme| {
        let tk = theme::tokens(t);
        if is_active {
            tk.accent
        } else {
            tk.sidebar_text_muted
        }
    });
    button(glyph)
        .padding([8, 8])
        .style(move |t: &Theme, status| {
            let tk = theme::tokens(t);
            let bg = if is_active {
                Some(tk.control_wash.into())
            } else if matches!(status, button::Status::Hovered) {
                Some(tk.control_bg.into())
            } else {
                None
            };
            button::Style {
                background: bg,
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: if is_active { theme::HAIRLINE } else { 0.0 },
                    color: if is_active {
                        tk.highlight_border
                    } else {
                        Color::TRANSPARENT
                    },
                },
                ..Default::default()
            }
        })
        .on_press(Message::Navigate(route))
        .into()
}
fn quantity_badge<'a>(value: ShellQuantityBadge) -> Element<'a, Message> {
    match value {
        ShellQuantityBadge::Current(value) => badge(value, BadgeKind::Neutral),
        ShellQuantityBadge::Stale(value) => badge(value, BadgeKind::Warning),
        ShellQuantityBadge::Unobserved => Space::new().width(0).into(),
    }
}
