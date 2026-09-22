//! Shared desktop window-chrome contract (DUAL-15-13).
//!
//! A frameless desktop window is a *host* capability: the OS decoration has to
//! be replaced by real draggable chrome, or the window becomes immovable. Both
//! surfaces therefore describe the same chrome shape here and report what
//! their host can actually do:
//!
//! * [`WindowChrome`] is the shared shape — decoration style, drag-strip
//!   height, double-click behaviour and where the shadow comes from;
//! * [`WindowChromeSupport`] is the per-host report — a host either implements
//!   the drag/minimize/maximize path, or states the typed reason it keeps the
//!   system decorations instead of shipping an unmovable frameless window.
//!
//! Native shadow is deliberately *not* claimed as programmable: neither Iced
//! nor Bevy exposes a shadow knob, so the contract records whether the style
//! keeps the platform's default shadow or loses it — it never promises a
//! custom shadow.

/// Height of the draggable chrome strip a frameless window must mount.
pub const CHROME_DRAG_STRIP_HEIGHT_PX: u32 = 38;

/// The pointer interval inside which two presses on the chrome strip count as
/// a double click (and therefore toggle maximize).
pub const CHROME_DOUBLE_CLICK_INTERVAL_MS: u64 = 400;

/// Which decoration style the window runs with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowChromeStyle {
    /// No OS decorations: the surface must mount its own drag strip and
    /// minimize/maximize/close controls.
    Frameless,
    /// The OS keeps its title bar and buttons; no custom chrome is mounted.
    SystemDecorated,
}

/// Where the window shadow comes from on this style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeShadow {
    /// The platform draws its default shadow (macOS borderless windows and
    /// every decorated window keep it).
    PlatformDefault,
    /// The frameless surface has no platform shadow (the common X11/Wayland
    /// and Windows borderless case) and none is requested on its behalf.
    Absent,
}

/// The shared chrome shape both surfaces build their window from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowChrome {
    pub style: WindowChromeStyle,
    /// Draggable strip height in logical pixels (`0` for system decorations).
    pub drag_strip_height_px: u32,
    /// Whether a double click on the strip toggles maximize.
    pub double_click_maximizes: bool,
    pub native_shadow: NativeShadow,
}

impl WindowChrome {
    /// The modern frameless desktop shell: custom strip, double-click maximize,
    /// no programmatic shadow.
    pub const FRAMELESS: Self = Self {
        style: WindowChromeStyle::Frameless,
        drag_strip_height_px: CHROME_DRAG_STRIP_HEIGHT_PX,
        double_click_maximizes: true,
        native_shadow: NativeShadow::Absent,
    };

    /// The conservative fallback: keep exactly what the OS provides.
    pub const SYSTEM: Self = Self {
        style: WindowChromeStyle::SystemDecorated,
        drag_strip_height_px: 0,
        double_click_maximizes: true,
        native_shadow: NativeShadow::PlatformDefault,
    };

    /// Whether the host window should ask the OS for decorations.
    pub const fn os_decorations(self) -> bool {
        matches!(self.style, WindowChromeStyle::SystemDecorated)
    }

    /// Whether the surface must mount its own strip + window controls.
    pub const fn needs_custom_controls(self) -> bool {
        matches!(self.style, WindowChromeStyle::Frameless)
    }

    /// The strip height a mounted chrome bar must reserve (`0` when the OS
    /// title bar still exists).
    pub const fn chrome_height_px(self) -> u32 {
        self.drag_strip_height_px
    }
}

/// What one host can honestly do with a frameless window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowChromeSupport {
    /// The host runs the frameless style with a real OS drag path.
    Hosted {
        /// `Window drag` (press-and-hold on the strip) is wired to the OS.
        drag: bool,
        minimize: bool,
        maximize: bool,
        native_shadow: NativeShadow,
    },
    /// The host cannot host frameless chrome; it keeps the system decorations
    /// and reports the typed boundary instead of mounting an immovable window.
    Unsupported { reason: &'static str },
}

impl WindowChromeSupport {
    /// Whether this host implements the frameless chrome path.
    pub const fn is_hosted(self) -> bool {
        matches!(self, Self::Hosted { .. })
    }

    /// Whether the OS drag path is wired (`false` on unsupported hosts).
    pub const fn drag(self) -> bool {
        match self {
            Self::Hosted { drag, .. } => drag,
            Self::Unsupported { .. } => false,
        }
    }

    /// Whether the host can toggle maximize (`false` when unsupported).
    pub const fn maximize(self) -> bool {
        match self {
            Self::Hosted { maximize, .. } => maximize,
            Self::Unsupported { .. } => false,
        }
    }

    /// The typed boundary of a host that keeps system decorations.
    pub const fn unsupported_reason(self) -> Option<&'static str> {
        match self {
            Self::Hosted { .. } => None,
            Self::Unsupported { reason } => Some(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frameless_style_owns_a_real_drag_strip() {
        let chrome = WindowChrome::FRAMELESS;
        assert!(!chrome.os_decorations());
        assert!(chrome.needs_custom_controls());
        assert_eq!(chrome.chrome_height_px(), CHROME_DRAG_STRIP_HEIGHT_PX);
        assert!(chrome.double_click_maximizes);
        assert_eq!(chrome.native_shadow, NativeShadow::Absent);
    }

    #[test]
    fn the_system_style_keeps_os_decorations_and_shadow() {
        let chrome = WindowChrome::SYSTEM;
        assert!(chrome.os_decorations());
        assert!(!chrome.needs_custom_controls());
        assert_eq!(chrome.chrome_height_px(), 0);
        assert_eq!(chrome.native_shadow, NativeShadow::PlatformDefault);
    }

    #[test]
    fn a_host_without_a_drag_path_reports_the_typed_boundary() {
        let hosted = WindowChromeSupport::Hosted {
            drag: true,
            minimize: true,
            maximize: true,
            native_shadow: NativeShadow::Absent,
        };
        assert!(hosted.is_hosted());
        assert!(hosted.drag());
        assert!(hosted.maximize());
        assert_eq!(hosted.unsupported_reason(), None);

        let unsupported = WindowChromeSupport::Unsupported {
            reason: "host-keeps-system-decorations",
        };
        assert!(!unsupported.is_hosted());
        assert!(!unsupported.drag());
        assert!(!unsupported.maximize());
        assert_eq!(
            unsupported.unsupported_reason(),
            Some("host-keeps-system-decorations")
        );
    }
}
