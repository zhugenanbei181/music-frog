#[cfg(target_os = "android")]
use rustls::crypto::ring::default_provider;
#[cfg(target_os = "android")]
pub(crate) fn ensure_rustls_provider() {
    if default_provider().install_default().is_err() {
        log::debug!("rustls provider already installed");
    }
}

#[cfg(not(target_os = "android"))]
pub(crate) fn ensure_rustls_provider() {}
