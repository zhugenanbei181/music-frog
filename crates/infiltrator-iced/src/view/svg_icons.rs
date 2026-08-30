//! Embedded monochrome SVG icon set (Lucide-style, 24x24, stroke-width 2,
//! round caps, `stroke="currentColor"`).
//!
//! The raw files live in `assets/icons/*.svg` and are compiled into the
//! binary with `include_bytes!`, so no runtime asset lookup is needed.
//! Recoloring is done through the iced svg color filter — see [`icon`].
//!
//! `text` block: Usage in a page view:
//! `icon(Icon::Search, 16.0, theme::tokens(theme).text_secondary)`.

use std::collections::HashMap;
use std::sync::LazyLock;

use iced::widget::svg;
use iced::{Color, Element};

/// Every icon available in the set. Variants map 1:1 to
/// `assets/icons/<kebab-case>.svg`.
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
}

impl Icon {
    /// Raw (embedded) SVG source for this icon.
    fn bytes(self) -> &'static [u8] {
        match self {
            Icon::LayoutGrid => include_bytes!("../../assets/icons/layout-grid.svg"),
            Icon::FileText => include_bytes!("../../assets/icons/file-text.svg"),
            Icon::Globe => include_bytes!("../../assets/icons/globe.svg"),
            Icon::Activity => include_bytes!("../../assets/icons/activity.svg"),
            Icon::ListChecks => include_bytes!("../../assets/icons/list-checks.svg"),
            Icon::Network => include_bytes!("../../assets/icons/network.svg"),
            Icon::RefreshCw => include_bytes!("../../assets/icons/refresh-cw.svg"),
            Icon::Settings => include_bytes!("../../assets/icons/settings.svg"),
            Icon::Code2 => include_bytes!("../../assets/icons/code-2.svg"),
            Icon::Search => include_bytes!("../../assets/icons/search.svg"),
            Icon::Target => include_bytes!("../../assets/icons/target.svg"),
            Icon::Zap => include_bytes!("../../assets/icons/zap.svg"),
            Icon::Shield => include_bytes!("../../assets/icons/shield.svg"),
            Icon::Wifi => include_bytes!("../../assets/icons/wifi.svg"),
            Icon::ChevronDown => include_bytes!("../../assets/icons/chevron-down.svg"),
            Icon::ChevronRight => include_bytes!("../../assets/icons/chevron-right.svg"),
            Icon::ChevronLeft => include_bytes!("../../assets/icons/chevron-left.svg"),
            Icon::ChevronUp => include_bytes!("../../assets/icons/chevron-up.svg"),
            Icon::X => include_bytes!("../../assets/icons/x.svg"),
            Icon::Plus => include_bytes!("../../assets/icons/plus.svg"),
            Icon::Trash2 => include_bytes!("../../assets/icons/trash-2.svg"),
            Icon::Copy => include_bytes!("../../assets/icons/copy.svg"),
            Icon::Pencil => include_bytes!("../../assets/icons/pencil.svg"),
            Icon::Pin => include_bytes!("../../assets/icons/pin.svg"),
            Icon::ArrowUp => include_bytes!("../../assets/icons/arrow-up.svg"),
            Icon::ArrowDown => include_bytes!("../../assets/icons/arrow-down.svg"),
            Icon::Server => include_bytes!("../../assets/icons/server.svg"),
            Icon::Plug => include_bytes!("../../assets/icons/plug.svg"),
        }
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
