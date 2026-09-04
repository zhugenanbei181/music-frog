//! Host-selected default adapter types.
//!
//! The capability interfaces live in `infiltrator-ports`. These aliases are
//! only composition conveniences for native binaries.

#[cfg(not(target_os = "android"))]
pub type DefaultCredentialStore = crate::desktop::KeyringCredentialStore;

#[cfg(target_os = "android")]
pub type DefaultCredentialStore = crate::android::AndroidCredentialStore;

#[cfg(not(target_os = "android"))]
pub type DefaultDataDirProvider = crate::desktop::DesktopDataDirProvider;

#[cfg(target_os = "android")]
pub type DefaultDataDirProvider = crate::android::AndroidDataDirProvider;
