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

    /// Build a scan request for the current frame, or `None` when nothing is
    /// streaming or the frame geometry is degenerate.
    pub fn scan_request(&self, request_id: u64, texture_id: u64) -> Option<CameraScanRequest> {
        if !self.is_streaming {
            return None;
        }
        let texture =
            CameraScanTexture::new(texture_id, self.frame_width, self.frame_height, self.format)
                .ok()?;
        Some(CameraScanRequest {
            request_id,
            texture,
        })
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

/// Opaque GPU texture descriptor handed to the camera scanner.
///
/// It carries only integer identity and geometry — never an `Image` asset
/// handle or ECS entity — so the scanner seam stays clear of renderer
/// internals. A zero-area frame is rejected as [`CameraScanReject::InvalidTexture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraScanTexture {
    texture_id: u64,
    width: u32,
    height: u32,
    format: CameraPixelFormat,
}

impl CameraScanTexture {
    pub fn new(
        texture_id: u64,
        width: u32,
        height: u32,
        format: CameraPixelFormat,
    ) -> Result<Self, CameraScanReject> {
        if width == 0 || height == 0 {
            return Err(CameraScanReject::InvalidTexture);
        }
        Ok(Self {
            texture_id,
            width,
            height,
            format,
        })
    }

    pub fn texture_id(&self) -> u64 {
        self.texture_id
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> CameraPixelFormat {
        self.format
    }
}

/// Typed request to scan a code out of one GPU camera frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraScanRequest {
    pub request_id: u64,
    pub texture: CameraScanTexture,
}

/// Closed reason a scan session ended without a payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraScanReject {
    PermissionDenied,
    UserCancelled,
    InvalidTexture,
    DecodeFailed,
}

/// Terminal, typed response of a camera scan session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CameraScanResponse {
    Granted(String),
    Denied,
    Cancelled,
    Invalid(CameraScanReject),
}

/// State of the camera scan handoff state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CameraScanState {
    #[default]
    Idle,
    AwaitingPermission,
    Scanning,
    Denied,
    Cancelled,
    Invalid,
    Detected,
}

impl CameraScanState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Denied | Self::Cancelled | Self::Invalid | Self::Detected
        )
    }
}

/// Typed rejection of a transition the current scan state forbids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraScanTransitionError {
    pub from: CameraScanState,
}

/// Camera scan handoff state machine: request → permission → scan → response.
///
/// It owns no camera or GPU dependency; a host drives the transitions with the
/// permission result and the decoded payload, and reads a typed response back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraScanSession {
    request_id: Option<u64>,
    state: CameraScanState,
    response: Option<CameraScanResponse>,
}

impl CameraScanSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> CameraScanState {
        self.state
    }

    pub fn request_id(&self) -> Option<u64> {
        self.request_id
    }

    pub fn response(&self) -> Option<&CameraScanResponse> {
        self.response.as_ref()
    }

    /// Start a session. Only legal from [`CameraScanState::Idle`].
    pub fn request(
        &mut self,
        request_id: u64,
        texture: CameraScanTexture,
    ) -> Result<CameraScanRequest, CameraScanTransitionError> {
        self.require(CameraScanState::Idle)?;
        self.request_id = Some(request_id);
        self.state = CameraScanState::AwaitingPermission;
        Ok(CameraScanRequest {
            request_id,
            texture,
        })
    }

    /// Permission granted: `AwaitingPermission` → `Scanning`.
    pub fn grant_permission(&mut self) -> Result<(), CameraScanTransitionError> {
        self.require(CameraScanState::AwaitingPermission)?;
        self.state = CameraScanState::Scanning;
        Ok(())
    }

    /// Permission denied: `AwaitingPermission` → `Denied`.
    pub fn deny_permission(&mut self) -> Result<(), CameraScanTransitionError> {
        self.require(CameraScanState::AwaitingPermission)?;
        self.state = CameraScanState::Denied;
        self.response = Some(CameraScanResponse::Denied);
        Ok(())
    }

    /// User cancelled from any non-terminal state → `Cancelled`.
    pub fn cancel(&mut self) -> Result<(), CameraScanTransitionError> {
        if self.state.is_terminal() {
            return Err(CameraScanTransitionError { from: self.state });
        }
        self.state = CameraScanState::Cancelled;
        self.response = Some(CameraScanResponse::Cancelled);
        Ok(())
    }

    /// Scan failed: `Scanning` → `Invalid`.
    pub fn invalidate(
        &mut self,
        reason: CameraScanReject,
    ) -> Result<(), CameraScanTransitionError> {
        self.require(CameraScanState::Scanning)?;
        self.state = CameraScanState::Invalid;
        self.response = Some(CameraScanResponse::Invalid(reason));
        Ok(())
    }

    /// A code was decoded: `Scanning` → `Detected`.
    pub fn detect(&mut self, payload: impl Into<String>) -> Result<(), CameraScanTransitionError> {
        self.require(CameraScanState::Scanning)?;
        self.state = CameraScanState::Detected;
        self.response = Some(CameraScanResponse::Granted(payload.into()));
        Ok(())
    }

    /// Return to `Idle` from a terminal state, clearing the request and response.
    pub fn reset(&mut self) -> Result<(), CameraScanTransitionError> {
        if !self.state.is_terminal() {
            return Err(CameraScanTransitionError { from: self.state });
        }
        self.request_id = None;
        self.state = CameraScanState::Idle;
        self.response = None;
        Ok(())
    }

    fn require(&self, expected: CameraScanState) -> Result<(), CameraScanTransitionError> {
        if self.state == expected {
            Ok(())
        } else {
            Err(CameraScanTransitionError { from: self.state })
        }
    }
}

/// Lifecycle state of a native platform view region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlatformViewState {
    #[default]
    Unmounted,
    Mounted,
    Paused,
}

/// A completed native platform view transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformViewTransition {
    Mounted,
    Paused,
    Resumed,
    Unmounted,
}

/// Typed rejection of a platform view transition the current state forbids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformViewTransitionError {
    AlreadyMounted,
    NotMounted,
    AlreadyPaused,
    NotPaused,
}

/// RAII lifecycle host managing native OS platform view compositing (Android SurfaceView / iOS CALayer).
///
/// Guarantees clean detach and GPU unbind on drop ("严禁管杀不管埋").
#[derive(Debug, PartialEq)]
pub struct PlatformViewLifecycleHost {
    pub view_id: String,
    pub is_attached: bool,
    pub native_handle_id: Option<u64>,
    state: PlatformViewState,
}

impl PlatformViewLifecycleHost {
    pub fn new(view_id: impl Into<String>) -> Self {
        Self {
            view_id: view_id.into(),
            is_attached: false,
            native_handle_id: None,
            state: PlatformViewState::Unmounted,
        }
    }

    pub fn state(&self) -> PlatformViewState {
        self.state
    }

    /// Attach native view handle.
    pub fn attach(&mut self, handle_id: u64) {
        self.is_attached = true;
        self.native_handle_id = Some(handle_id);
        self.state = PlatformViewState::Mounted;
    }

    /// Explicitly detach and release native view handle.
    pub fn detach(&mut self) -> Option<u64> {
        self.is_attached = false;
        self.state = PlatformViewState::Unmounted;
        self.native_handle_id.take()
    }

    /// Mount a native view region. Illegal unless currently unmounted.
    pub fn mount(
        &mut self,
        handle_id: u64,
    ) -> Result<PlatformViewTransition, PlatformViewTransitionError> {
        match self.state {
            PlatformViewState::Unmounted => {
                self.attach(handle_id);
                Ok(PlatformViewTransition::Mounted)
            }
            PlatformViewState::Mounted | PlatformViewState::Paused => {
                Err(PlatformViewTransitionError::AlreadyMounted)
            }
        }
    }

    /// Pause a mounted view region without releasing its native handle.
    pub fn pause(&mut self) -> Result<PlatformViewTransition, PlatformViewTransitionError> {
        match self.state {
            PlatformViewState::Mounted => {
                self.state = PlatformViewState::Paused;
                Ok(PlatformViewTransition::Paused)
            }
            PlatformViewState::Paused => Err(PlatformViewTransitionError::AlreadyPaused),
            PlatformViewState::Unmounted => Err(PlatformViewTransitionError::NotMounted),
        }
    }

    /// Resume a paused view region.
    pub fn resume(&mut self) -> Result<PlatformViewTransition, PlatformViewTransitionError> {
        match self.state {
            PlatformViewState::Paused => {
                self.state = PlatformViewState::Mounted;
                Ok(PlatformViewTransition::Resumed)
            }
            PlatformViewState::Mounted => Err(PlatformViewTransitionError::NotPaused),
            PlatformViewState::Unmounted => Err(PlatformViewTransitionError::NotMounted),
        }
    }

    /// Unmount the view region, releasing the native handle exactly once.
    pub fn unmount(&mut self) -> Result<PlatformViewTransition, PlatformViewTransitionError> {
        match self.state {
            PlatformViewState::Unmounted => Err(PlatformViewTransitionError::NotMounted),
            PlatformViewState::Mounted | PlatformViewState::Paused => {
                self.detach();
                Ok(PlatformViewTransition::Unmounted)
            }
        }
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
