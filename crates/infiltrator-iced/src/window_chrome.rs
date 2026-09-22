//! Iced host application of the shared window-chrome contract (DUAL-15-13).
//!
//! The desktop shell runs frameless: the OS title bar is replaced by the
//! mounted [`crate::view::chrome`] strip, whose press/double-click route to
//! the real winit drag path (`iced::window::drag`) and to the minimize /
//! maximize / close ops. The shape (strip height, double-click rule, shadow
//! availability) is owned by `infiltrator_contract::window_chrome`; this
//! module is the honest host report behind it.

use iced::window;
use infiltrator_contract::window_chrome::{NativeShadow, WindowChrome, WindowChromeSupport};

use crate::types::message::Message;

/// The host window op a chrome message asks for.
///
/// The mapping lives here (not inside the update chain) so a test can prove
/// every chrome message routes to one real op, and so the strip's intent is
/// readable without an iced runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeRequest {
    Drag,
    ToggleMaximize,
    Minimize,
    Close,
}

impl ChromeRequest {
    /// The request carried by a chrome message (`None` for other messages).
    pub const fn from_message(message: &Message) -> Option<Self> {
        match message {
            Message::WindowChromeDragRequested => Some(Self::Drag),
            Message::WindowChromeToggleMaximize => Some(Self::ToggleMaximize),
            Message::WindowChromeMinimize => Some(Self::Minimize),
            Message::WindowChromeClose => Some(Self::Close),
            _ => None,
        }
    }

    /// The real iced task for this request on a resolved window.
    pub fn task(self, id: window::Id) -> iced::Task<Message> {
        match self {
            Self::Drag => iced::window::drag(id),
            Self::ToggleMaximize => iced::window::toggle_maximize(id),
            Self::Minimize => iced::window::minimize(id, true),
            // Closing goes through the orderly exit path in the update chain.
            Self::Close => iced::Task::none(),
        }
    }
}

/// The chrome shape this host runs: the modern frameless desktop shell.
pub fn chrome() -> WindowChrome {
    WindowChrome::FRAMELESS
}

/// What this host can actually do with the shared chrome shape.
///
/// `native_shadow` stays `Absent`: iced exposes no shadow knob, so the
/// frameless window does not get one requested on its behalf — the platform
/// decides, and the contract records that honestly.
pub fn support() -> WindowChromeSupport {
    WindowChromeSupport::Hosted {
        drag: true,
        minimize: true,
        maximize: true,
        native_shadow: NativeShadow::Absent,
    }
}

/// The iced window settings for the shared chrome shape. `decorations` comes
/// from the contract, so the OS title bar and the mounted strip can never
/// both appear (or both vanish).
pub fn window_settings(size: (f32, f32), min_size: (f32, f32)) -> window::Settings {
    window::Settings {
        size: size.into(),
        min_size: Some(min_size.into()),
        decorations: chrome().os_decorations(),
        exit_on_close_request: false,
        ..Default::default()
    }
}
