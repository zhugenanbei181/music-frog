//! Mobile camera texture streaming and native platform view integration slots.

use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;

/// Format of incoming video/camera preview frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CameraPixelFormat {
    #[default]
    Rgba8,
    Nv21,
    Yuv420p,
}

/// Buffer metadata for camera preview texture frames.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CameraTextureFeed {
    pub is_streaming: bool,
    pub frame_width: u32,
    pub frame_height: u32,
    pub format: CameraPixelFormat,
    pub last_qr_payload: Option<String>,
}

impl CameraTextureFeed {
    pub fn start_stream(&mut self, width: u32, height: u32, format: CameraPixelFormat) {
        self.is_streaming = true;
        self.frame_width = width;
        self.frame_height = height;
        self.format = format;
    }

    pub fn stop_stream(&mut self) {
        self.is_streaming = false;
        self.last_qr_payload = None;
    }

    pub fn on_qr_detected(&mut self, payload: impl Into<String>) {
        self.last_qr_payload = Some(payload.into());
    }
}

/// Slot marker component where an external native view (e.g. WebView/Map) is composited.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct NativePlatformViewSlot {
    pub view_type_id: String,
    pub bounds_size: Vec2,
    pub is_visible: bool,
}

impl NativePlatformViewSlot {
    pub fn new(view_type_id: impl Into<String>, size: Vec2) -> Self {
        Self {
            view_type_id: view_type_id.into(),
            bounds_size: size,
            is_visible: true,
        }
    }
}

impl CameraTextureFeed {
    /// Calculate raw frame buffer size in bytes based on pixel format.
    pub fn frame_byte_size(&self) -> usize {
        let pixels = (self.frame_width * self.frame_height) as usize;
        match self.format {
            CameraPixelFormat::Rgba8 => pixels * 4,
            CameraPixelFormat::Nv21 | CameraPixelFormat::Yuv420p => pixels * 3 / 2,
        }
    }

    /// Extract clean subscription URL from detected QR code payload.
    pub fn parse_qr_config_url(&self) -> Option<String> {
        let payload = self.last_qr_payload.as_deref()?;
        if payload.starts_with("clash://install-config?url=") {
            let url = payload.trim_start_matches("clash://install-config?url=");
            Some(url.to_string())
        } else if payload.starts_with("http://") || payload.starts_with("https://") {
            Some(payload.to_string())
        } else {
            None
        }
    }
}

/// RAII lifecycle host managing native OS platform view compositing (Android SurfaceView / iOS CALayer).
///
/// Guarantees clean detach and GPU unbind on drop ("严禁管杀不管埋").
#[derive(Debug, PartialEq)]
pub struct PlatformViewLifecycleHost {
    pub view_id: String,
    pub is_attached: bool,
    pub native_handle_id: Option<u64>,
}

impl PlatformViewLifecycleHost {
    pub fn new(view_id: impl Into<String>) -> Self {
        Self {
            view_id: view_id.into(),
            is_attached: false,
            native_handle_id: None,
        }
    }

    /// Attach native view handle.
    pub fn attach(&mut self, handle_id: u64) {
        self.is_attached = true;
        self.native_handle_id = Some(handle_id);
    }

    /// Explicitly detach and release native view handle.
    pub fn detach(&mut self) -> Option<u64> {
        self.is_attached = false;
        self.native_handle_id.take()
    }
}

impl Drop for PlatformViewLifecycleHost {
    fn drop(&mut self) {
        if self.is_attached {
            self.detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_view_lifecycle_host() {
        let mut host = PlatformViewLifecycleHost::new("native-map");
        assert!(!host.is_attached);

        host.attach(123456);
        assert!(host.is_attached);
        assert_eq!(host.native_handle_id, Some(123456));

        let detached = host.detach();
        assert!(!host.is_attached);
        assert_eq!(detached, Some(123456));
    }
}
