//! Host-selected default adapter types.
//!
//! The actual capability interfaces live in `infiltrator-ports`. This module
//! only keeps target-specific default concrete types during the 0.30
//! composition migration.

#[cfg(not(target_os = "android"))]
pub type DefaultCredentialStore = crate::desktop::KeyringCredentialStore;

#[cfg(target_os = "android")]
pub type DefaultCredentialStore = crate::android::AndroidCredentialStore;

#[cfg(not(target_os = "android"))]
pub type DefaultDataDirProvider = crate::desktop::DesktopDataDirProvider;

#[cfg(target_os = "android")]
pub type DefaultDataDirProvider = crate::android::AndroidDataDirProvider;
