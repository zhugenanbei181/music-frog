//! Host-selected default adapter types.
//!
//! The capability interfaces live in `infiltrator-ports`. These aliases are
//! only composition conveniences for native binaries.

#[cfg(target_os = "android")]
use crate::android::AndroidCredentialStore;
#[cfg(target_os = "android")]
use crate::android::AndroidDataDirProvider;
#[cfg(not(target_os = "android"))]
use crate::desktop::DesktopDataDirProvider;
#[cfg(not(target_os = "android"))]
use crate::desktop::KeyringCredentialStore;
#[cfg(not(target_os = "android"))]
pub type DefaultCredentialStore = KeyringCredentialStore;

#[cfg(target_os = "android")]
pub type DefaultCredentialStore = AndroidCredentialStore;

#[cfg(not(target_os = "android"))]
pub type DefaultDataDirProvider = DesktopDataDirProvider;

#[cfg(target_os = "android")]
pub type DefaultDataDirProvider = AndroidDataDirProvider;
