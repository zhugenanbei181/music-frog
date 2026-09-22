//! DUAL-07-01: outbound port for the host-provided subscription import
//! channels a surface cannot own itself.
//!
//! The Iced surface used to reach into `std::process` / `tokio::fs` for the
//! local-file and clipboard channels, which made the import vocabulary a
//! per-surface implementation detail. This port hoists both behind one
//! host seam: a desktop host reads a real file / clipboard, while a host
//! without a clipboard (or a sandboxed/mobile shell) returns a typed
//! unsupported error rather than pretending it read an empty document.

use async_trait::async_trait;

use crate::error::PortError;

/// Host-provided document sources for local/剪贴板 imports.
#[async_trait]
pub trait SubscriptionImportPort: Send + Sync {
    /// Read a local configuration/subscription file as UTF-8 text.
    async fn read_local_file(&self, path: &str) -> Result<String, PortError>;

    /// Read the current system clipboard as text.
    ///
    /// A host that has no clipboard integration must return
    /// [`PortError::unsupported`] instead of a fabricated empty string.
    async fn read_clipboard(&self) -> Result<String, PortError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::capability::Capability;

    struct DesktopLike;

    #[async_trait]
    impl SubscriptionImportPort for DesktopLike {
        async fn read_local_file(&self, _path: &str) -> Result<String, PortError> {
            Ok("proxies: []".to_string())
        }

        async fn read_clipboard(&self) -> Result<String, PortError> {
            Ok("https://example.com/sub".to_string())
        }
    }

    struct Headless;

    #[async_trait]
    impl SubscriptionImportPort for Headless {
        async fn read_local_file(&self, _path: &str) -> Result<String, PortError> {
            Err(PortError::unsupported(
                Capability::Profiles,
                "this host cannot read local files",
            ))
        }

        async fn read_clipboard(&self) -> Result<String, PortError> {
            Err(PortError::unsupported(
                Capability::Profiles,
                "this host has no clipboard integration",
            ))
        }
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                std::task::Poll::Ready(value) => return value,
                std::task::Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn desktop_like_host_reads_both_channels() {
        let port = DesktopLike;
        assert_eq!(
            block_on(port.read_local_file("a.yaml")).expect("file"),
            "proxies: []"
        );
        assert_eq!(
            block_on(port.read_clipboard()).expect("clipboard"),
            "https://example.com/sub"
        );
    }

    #[test]
    fn headless_host_reports_typed_unsupported() {
        let port = Headless;
        match block_on(port.read_clipboard()).expect_err("unsupported") {
            PortError::Unsupported { capability, .. } => {
                assert_eq!(capability, Capability::Profiles);
            }
            other => panic!("expected typed unsupported, got {other:?}"),
        }
    }
}
