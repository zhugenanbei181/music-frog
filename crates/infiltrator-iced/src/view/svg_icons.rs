//! Embedded monochrome SVG icon set (Lucide-style, 24x24, stroke-width 2,
//! round caps, `stroke="currentColor"`).
//!
//! Raw SVGs are owned by the shared visual asset catalog and compiled into the
//! binary, so no runtime asset lookup is needed.
//! Recoloring is done through the iced svg color filter — see [`icon`].
//!
//! `text` block: Usage in a page view:
//! `icon(Icon::Search, 16.0, theme::tokens(theme).text_secondary)`.

use iced::widget::svg;
use iced::{Color, Element};
use infiltrator_shared::assets::iced_icon_svg;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Every icon available in the set. Variants map 1:1 to a shared catalog ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    LayoutGrid,
    FileText,
    Globe,
    Activity,
    ListChecks,
    Network,
    RefreshCw,
    Settings,
    Code2,
    Search,
    Target,
    Zap,
    Shield,
    Wifi,
    ChevronDown,
    ChevronRight,
    ChevronLeft,
    ChevronUp,
    X,
    Plus,
    Trash2,
    Copy,
    Pencil,
    Pin,
    ArrowUp,
    ArrowDown,
    Server,
    Plug,
    Minus,
    Square,
}

impl Icon {
    /// Raw (embedded) SVG source for this icon.
    fn bytes(self) -> &'static [u8] {
        let name = match self {
            Icon::LayoutGrid => "layout-grid",
            Icon::FileText => "file-text",
            Icon::Globe => "globe",
            Icon::Activity => "activity",
            Icon::ListChecks => "list-checks",
            Icon::Network => "network",
            Icon::RefreshCw => "refresh-cw",
            Icon::Settings => "settings",
            Icon::Code2 => "code-2",
            Icon::Search => "search",
            Icon::Target => "target",
            Icon::Zap => "zap",
            Icon::Shield => "shield",
            Icon::Wifi => "wifi",
            Icon::ChevronDown => "chevron-down",
            Icon::ChevronRight => "chevron-right",
            Icon::ChevronLeft => "chevron-left",
            Icon::ChevronUp => "chevron-up",
            Icon::X => "x",
            Icon::Plus => "plus",
            Icon::Trash2 => "trash-2",
            Icon::Copy => "copy",
            Icon::Pencil => "pencil",
            Icon::Pin => "pin",
            Icon::ArrowUp => "arrow-up",
            Icon::ArrowDown => "arrow-down",
            Icon::Server => "server",
            Icon::Plug => "plug",
            Icon::Minus => "minus",
            Icon::Square => "square",
        };
        iced_icon_svg(name).expect("registered Iced icon must exist in the shared asset catalog")
    }
}

/// Parsed-SVG handles, built once per icon.
static HANDLES: LazyLock<HashMap<Icon, svg::Handle>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for icon in [
        Icon::LayoutGrid,
        Icon::FileText,
        Icon::Globe,
        Icon::Activity,
        Icon::ListChecks,
        Icon::Network,
        Icon::RefreshCw,
        Icon::Settings,
        Icon::Code2,
        Icon::Search,
        Icon::Target,
        Icon::Zap,
        Icon::Shield,
        Icon::Wifi,
        Icon::ChevronDown,
        Icon::ChevronRight,
        Icon::ChevronLeft,
        Icon::ChevronUp,
        Icon::X,
        Icon::Plus,
        Icon::Trash2,
        Icon::Copy,
        Icon::Pencil,
        Icon::Pin,
        Icon::ArrowUp,
        Icon::ArrowDown,
        Icon::Server,
        Icon::Plug,
        Icon::Minus,
        Icon::Square,
    ] {
        map.insert(icon, svg::Handle::from_memory(icon.bytes()));
    }
    map
});

/// The cached [`svg::Handle`] for an icon (for advanced/custom widgets).
pub fn icon_handle(icon: Icon) -> svg::Handle {
    HANDLES[&icon].clone()
}

/// A tinted monochrome icon element.
///
/// The `color` filter repaints the whole glyph, so any `Color` works in both
/// themes. Typical sizes: 14–16 for inline text-adjacent icons, 18–20 for
/// nav rows, 32–40 for empty states.
pub fn icon<'a, Message>(name: Icon, size: f32, color: Color) -> Element<'a, Message> {
    svg(icon_handle(name))
        .width(size)
        .height(size)
        .style(move |_theme, _status| svg::Style { color: Some(color) })
        .into()
}

/// Theme-aware variant of [`icon`]: the tint is resolved from the active
/// theme on every draw, e.g. `icon_themed(Icon::Globe, 16.0, |t| theme::tokens(t).accent)`.
pub fn icon_themed<'a, Message>(
    name: Icon,
    size: f32,
    color: impl Fn(&iced::Theme) -> Color + 'a,
) -> Element<'a, Message> {
    svg(icon_handle(name))
        .width(size)
        .height(size)
        .style(move |theme, _status| svg::Style {
            color: Some(color(theme)),
        })
        .into()
}
