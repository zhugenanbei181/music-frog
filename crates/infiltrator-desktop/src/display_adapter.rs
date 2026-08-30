use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DisplayBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub struct DisplayAdapter;

impl DisplayAdapter {
    /// Scales a base dimension using a provided scale factor and rounds correctly.
    pub fn scale_dimension(base: u32, scale_factor: f64) -> u32 {
        if scale_factor < 0.0 {
            return 0;
        }
        (base as f64 * scale_factor).round() as u32
    }

    /// Computes centered window coordinates inside the display bounds.
    pub fn center_window(window_w: u32, window_h: u32, display: &DisplayBounds) -> WindowPosition {
        let x = display.x + (display.width as i32 - window_w as i32) / 2;
        let y = display.y + (display.height as i32 - window_h as i32) / 2;
        WindowPosition {
            x,
            y,
            width: window_w,
            height: window_h,
        }
    }

    /// Clamps the window so that it fits as completely on-screen as possible.
    pub fn constrain_to_screen(window: WindowPosition, display: &DisplayBounds) -> WindowPosition {
        let mut x = window.x;
        let mut y = window.y;

        // Constrain X
        if x < display.x {
            x = display.x;
        } else if x + window.width as i32 > display.x + display.width as i32 {
            x = display.x + display.width as i32 - window.width as i32;
            if x < display.x {
                x = display.x; // fallback if window is wider than screen
            }
        }

        // Constrain Y
        if y < display.y {
            y = display.y;
        } else if y + window.height as i32 > display.y + display.height as i32 {
            y = display.y + display.height as i32 - window.height as i32;
            if y < display.y {
                y = display.y; // fallback if window is taller than screen
            }
        }

        WindowPosition {
            x,
            y,
            width: window.width,
            height: window.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_dimension() {
        assert_eq!(DisplayAdapter::scale_dimension(100, 1.0), 100);
        assert_eq!(DisplayAdapter::scale_dimension(100, 1.25), 125);
        assert_eq!(DisplayAdapter::scale_dimension(100, 1.5), 150);
        assert_eq!(DisplayAdapter::scale_dimension(100, 2.0), 200);
        assert_eq!(DisplayAdapter::scale_dimension(33, 1.5), 50);
    }

    #[test]
    fn test_center_window() {
        let display = DisplayBounds {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
        };

        let pos = DisplayAdapter::center_window(800, 600, &display);
        assert_eq!(pos.x, 560);
        assert_eq!(pos.y, 240);
        assert_eq!(pos.width, 800);
        assert_eq!(pos.height, 600);

        let display_4k = DisplayBounds {
            x: 1920,
            y: 0,
            width: 3840,
            height: 2160,
            scale_factor: 2.0,
        };

        let pos_4k = DisplayAdapter::center_window(1000, 1000, &display_4k);
        assert_eq!(pos_4k.x, 1920 + 1420);
        assert_eq!(pos_4k.y, 580);
    }

    #[test]
    fn test_constrain_to_screen() {
        let display = DisplayBounds {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
        };

        // Window too far left/up
        let win1 = WindowPosition { x: -100, y: -50, width: 800, height: 600 };
        let res1 = DisplayAdapter::constrain_to_screen(win1, &display);
        assert_eq!(res1.x, 0);
        assert_eq!(res1.y, 0);

        // Window too far right/down
        let win2 = WindowPosition { x: 1800, y: 1000, width: 800, height: 600 };
        let res2 = DisplayAdapter::constrain_to_screen(win2, &display);
        assert_eq!(res2.x, 1120); // 1920 - 800
        assert_eq!(res2.y, 480);  // 1080 - 600

        // Oversized window (fallback to top-left)
        let win3 = WindowPosition { x: 100, y: 100, width: 2000, height: 2000 };
        let res3 = DisplayAdapter::constrain_to_screen(win3, &display);
        assert_eq!(res3.x, 0);
        assert_eq!(res3.y, 0);
    }
}
