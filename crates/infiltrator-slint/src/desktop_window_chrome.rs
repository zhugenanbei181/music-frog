//! Slint desktop host application of the shared frameless window contract.

use crate::AppWindow;
use infiltrator_contract::window_chrome::WindowChrome;
use slint::ComponentHandle;

/// The host's chrome choice also publishes the exact band the surface mounts.
pub(crate) fn install(window: &AppWindow) {
    let chrome = WindowChrome::FRAMELESS;
    window.set_frameless(!chrome.os_decorations());
    window.set_desktop_title_bar_units(title_bar_units(chrome));
    let handle_weak = window.as_weak();
    window.on_window_minimize(move || {
        if let Some(window) = handle_weak.upgrade() {
            window.window().set_minimized(true);
        }
    });
    let handle_weak = window.as_weak();
    window.on_window_toggle_maximize(move || {
        if let Some(window) = handle_weak.upgrade() {
            let maximized = !window.window().is_maximized();
            window.window().set_maximized(maximized);
            window.set_is_maximized(maximized);
        }
    });
    window.on_window_close(|| {
        let _ = slint::quit_event_loop();
    });
}

const fn title_bar_units(chrome: WindowChrome) -> i32 {
    chrome.chrome_height_px() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_planned_band_is_the_contract_chrome_height() {
        assert_eq!(
            title_bar_units(WindowChrome::FRAMELESS),
            WindowChrome::FRAMELESS.chrome_height_px() as i32
        );
        assert_eq!(title_bar_units(WindowChrome::SYSTEM), 0);
    }
}
