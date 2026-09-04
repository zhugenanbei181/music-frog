//! Native Desktop integration: frameless window hit testing, tray speed badges, and window geometry.

use bevy::ecs::resource::Resource;
use bevy::math::Vec2;

/// Hit test zone for frameless client-side window decoration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowHitZone {
    TitlebarDrag,
    MinimizeButton,
    MaximizeButton,
    CloseButton,
    ResizeBorderNorth,
    ResizeBorderSouth,
    ResizeBorderEast,
    ResizeBorderWest,
    Content,
}

/// Parameters defining frameless window geometry and draggable regions.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FramelessWindowConfig {
    pub titlebar_height_px: f32,
    pub resize_border_px: f32,
    pub window_size: Vec2,
}

impl Default for FramelessWindowConfig {
    fn default() -> Self {
        Self {
            titlebar_height_px: 36.0,
            resize_border_px: 6.0,
            window_size: Vec2::new(1180.0, 760.0),
        }
    }
}

impl FramelessWindowConfig {
    /// Perform non-blocking hit-testing for cursor coordinates relative to window top-left.
    pub fn hit_test(&self, cursor_pos: Vec2) -> WindowHitZone {
        let b = self.resize_border_px;
        let w = self.window_size.x;
        let h = self.window_size.y;

        if cursor_pos.y < b {
            return WindowHitZone::ResizeBorderNorth;
        }
        if cursor_pos.y > h - b {
            return WindowHitZone::ResizeBorderSouth;
        }
        if cursor_pos.x < b {
            return WindowHitZone::ResizeBorderWest;
        }
        if cursor_pos.x > w - b {
            return WindowHitZone::ResizeBorderEast;
        }

        if cursor_pos.y <= self.titlebar_height_px {
            // Window control buttons in upper right corner (120px span)
            if cursor_pos.x > w - 40.0 {
                return WindowHitZone::CloseButton;
            } else if cursor_pos.x > w - 80.0 {
                return WindowHitZone::MaximizeButton;
            } else if cursor_pos.x > w - 120.0 {
                return WindowHitZone::MinimizeButton;
            }
            return WindowHitZone::TitlebarDrag;
        }

        WindowHitZone::Content
    }
}

/// System tray badge display state.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TrayBadgeState {
    pub upload_rate_str: String,
    pub download_rate_str: String,
    pub is_running: bool,
}

impl TrayBadgeState {
    pub fn update(
        &mut self,
        is_running: bool,
        up_str: impl Into<String>,
        down_str: impl Into<String>,
    ) {
        self.is_running = is_running;
        self.upload_rate_str = up_str.into();
        self.download_rate_str = down_str.into();
    }
}

/// Types of content payloads supported by the clipboard pipeline.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipboardPayload {
    PlainText(String),
    NodeUri(String),
    RedactedDiagnostic(String),
}

impl ClipboardPayload {
    /// Sanitize and redact sensitive query tokens or secrets from text before copying to clipboard.
    pub fn sanitize_text(input: &str) -> String {
        let mut sanitized = input.to_string();
        // Redact ?token=... or &token=...
        if let Some(pos) = sanitized.find("token=") {
            let end = sanitized[pos..]
                .find('&')
                .map(|e| pos + e)
                .unwrap_or(sanitized.len());
            sanitized.replace_range(pos + 6..end, "REDACTED");
        }
        // Redact secret: ...
        if let Some(pos) = sanitized.find("secret:") {
            let end = sanitized[pos..]
                .find('\n')
                .map(|e| pos + e)
                .unwrap_or(sanitized.len());
            sanitized.replace_range(pos + 7..end, " REDACTED");
        }
        sanitized
    }
}
