//! DUAL-05-13 desktop CA reader: real filesystem access behind the port.
//!
//! The shared application never touches the filesystem. This adapter is the
//! desktop host's honest answer to "read this certificate bundle": it reads the
//! file, refuses bundles over a sanity limit, and maps a missing file or an I/O
//! failure to a typed port error instead of an empty success.

use std::fs;

use infiltrator_ports::certificate_authority::{CaFile, CertificateAuthorityPort};
use infiltrator_ports::error::PortError;

/// Upper bound for a CA bundle the studio will read (4 MiB).
pub const MAX_CA_BUNDLE_BYTES: u64 = 4 * 1024 * 1024;

/// Desktop reader over the host filesystem.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DesktopCertificateAuthority;

impl DesktopCertificateAuthority {
    pub fn new() -> Self {
        Self
    }
}

impl CertificateAuthorityPort for DesktopCertificateAuthority {
    fn read_ca_file(&self, path: &str) -> Result<CaFile, PortError> {
        let path = path.trim();
        if path.is_empty() {
            return Err(PortError::NotFound(
                "CA bundle path must not be empty".to_string(),
            ));
        }
        let metadata = fs::metadata(path)
            .map_err(|error| PortError::NotFound(format!("`{path}`: {error}")))?;
        if !metadata.is_file() {
            return Err(PortError::NotFound(format!(
                "`{path}` is not a regular file"
            )));
        }
        if metadata.len() > MAX_CA_BUNDLE_BYTES {
            return Err(PortError::Failed(format!(
                "`{path}` is larger than the {MAX_CA_BUNDLE_BYTES} byte CA bundle limit"
            )));
        }
        let pem = fs::read_to_string(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidData {
                PortError::Failed(format!("`{path}` is not valid UTF-8 text"))
            } else {
                PortError::Io(format!("`{path}`: {error}"))
            }
        })?;
        Ok(CaFile {
            path: path.to_string(),
            pem,
        })
    }
}
