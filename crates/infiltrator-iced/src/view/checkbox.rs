//! The toolkit's own check box, skinned with the product's tokens.
//!
//! The behaviour belongs to [`iced::widget::checkbox`]: a press on the box or
//! on its label publishes the toggled value. The product keeps only the skin.
use crate::view::theme;
use iced::advanced::text::{IntoFragment, Renderer};
use iced::widget::checkbox;
use iced::{Border, Element, Theme, border};

/// The check box geometry.
///
/// The toolkit draws the box and the tick; the product only picks the size.
const CHECKBOX_SIZE_PX: f32 = 14.0;

/// The product's skin for the toolkit's check box.
///
/// The toolkit owns the box geometry, the tick and the interaction; the
/// product only owns the colours, so the fill, the tick and the hairline live
/// here.
fn checkbox_style(t: &Theme, status: checkbox::Status) -> checkbox::Style {
    let tk = theme::tokens(t);
    let is_checked = match status {
        checkbox::Status::Active { is_checked }
        | checkbox::Status::Hovered { is_checked }
        | checkbox::Status::Disabled { is_checked } => is_checked,
    };
    checkbox::Style {
        background: if is_checked {
            tk.accent.into()
        } else {
            tk.control_bg.into()
        },
        icon_color: tk.on_accent,
        border: Border {
            radius: border::Radius::from(3.0),
            width: theme::HAIRLINE,
            color: if is_checked {
                tk.accent
            } else {
                tk.card_border
            },
        },
        text_color: Some(tk.text_primary),
    }
}

/// The check box control: the toolkit's own check box.
///
/// The label is part of the control, so the click target is the box plus its
/// label — not the whole row the check box may sit in.
pub fn check_box<'a, Message, R>(
    value: bool,
    label: impl IntoFragment<'a>,
    on_change: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message, Theme, R>
where
    Message: 'a + Clone,
    R: Renderer + 'a,
{
    checkbox(value)
        .label(label)
        .size(CHECKBOX_SIZE_PX)
        .text_size(12)
        .style(checkbox_style)
        .on_toggle(move |value| on_change(value))
        .into()
}
