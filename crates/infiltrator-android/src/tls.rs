#[cfg(target_os = "android")]
pub(crate) fn ensure_rustls_provider() {
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        log::debug!("rustls provider already installed");
    }
}

#[cfg(not(target_os = "android"))]
pub(crate) fn ensure_rustls_provider() {}
