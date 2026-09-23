//! Host-declared touch-gesture capability and safe-area insets (DUAL-15-07).
//!
//! Touch recognition itself lives in the widget layer
//! (`infiltrator_bevy_widgets::gesture`) and the shared semantic vocabulary in
//! `infiltrator_contract::shell_gesture`. What a *host* owns is the honest
//! declaration that it can deliver touch at all and what its edge-to-edge safe
//! area is. A host without a touch surface keeps the typed-unsupported default
//! instead of pretending a recognizer exists.

use async_trait::async_trait;
use infiltrator_contract::shell_gesture::{SafeAreaInsets, TouchGestureSupport};

use crate::error::PortError;

/// What a host declares about its touch surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchGestureHostReport {
    /// Whether the host delivers touch and how many pointers it reports.
    pub support: TouchGestureSupport,
    /// The current edge-to-edge insets; [`SafeAreaInsets::ZERO`] when the host
    /// does not expose them.
    pub insets: SafeAreaInsets,
}

impl TouchGestureHostReport {
    /// A host that declared no touch surface.
    pub const fn unsupported(reason: &'static str) -> Self {
        Self {
            support: TouchGestureSupport::Unsupported { reason },
            insets: SafeAreaInsets::ZERO,
        }
    }

    /// A host that hosts the recognizer and reports its insets.
    pub const fn hosted(multi_touch: bool, insets: SafeAreaInsets) -> Self {
        Self {
            support: TouchGestureSupport::Hosted { multi_touch },
            insets,
        }
    }

    /// Whether this host hosts the recognizer.
    pub const fn is_hosted(&self) -> bool {
        self.support.is_hosted()
    }
}

impl Default for TouchGestureHostReport {
    fn default() -> Self {
        Self::unsupported("host did not declare a touch gesture surface")
    }
}

/// Host capability for touch gestures and the edge-to-edge safe area.
#[async_trait]
pub trait TouchGesturePort: Send + Sync {
    /// The host's declared touch support and current safe-area insets.
    ///
    /// The default is the typed-unsupported report: a host must opt in by
    /// overriding this method, and no adapter may fabricate a touch surface.
    async fn touch_gesture_report(&self) -> Result<TouchGestureHostReport, PortError> {
        Ok(TouchGestureHostReport::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;

    struct NoTouchHost;

    impl TouchGesturePort for NoTouchHost {}

    #[test]
    fn a_host_that_does_not_declare_touch_reports_typed_unsupported() {
        let host = NoTouchHost;
        let report = host
            .touch_gesture_report()
            .now_or_never()
            .expect("the default report is ready without awaiting")
            .expect("the default never fails");
        assert!(!report.is_hosted());
        assert_eq!(
            report.support.unsupported_reason(),
            Some("host did not declare a touch gesture surface")
        );
        assert_eq!(report.insets, SafeAreaInsets::ZERO);
    }

    #[test]
    fn a_declared_touch_host_carries_its_insets() {
        let report =
            TouchGestureHostReport::hosted(true, SafeAreaInsets::new(44.0, 0.0, 34.0, 0.0));
        assert!(report.is_hosted());
        assert!(report.support.multi_touch());
        assert_eq!(report.insets.vertical(), 78.0);
    }
}
