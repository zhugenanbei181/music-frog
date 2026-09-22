//! DUAL-05-13: host capability for reading a custom CA certificate bundle.
//!
//! The shared application validates PEM structure and fingerprints content,
//! but *reading a path* is a host fact: a mobile or sandboxed host may expose
//! no filesystem at all. Hosts without a reader install nothing, and the
//! application then reports the request as typed unsupported instead of
//! claiming the CA was loaded.

use infiltrator_contract::capability::Capability;

use crate::error::PortError;

/// One CA bundle the host really read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaFile {
    /// The path that was read (echoed back so the caller can match it).
    pub path: String,
    /// The bundle content, exactly as read.
    pub pem: String,
}

/// Read a certificate bundle from a host path.
///
/// Implementations must be honest: a missing file, a permission problem or a
/// size limit is an error, never an empty success.
pub trait CertificateAuthorityPort: Send + Sync {
    fn read_ca_file(&self, path: &str) -> Result<CaFile, PortError>;
}

/// The honest refusal: a host that cannot read CA files.
///
/// Production hosts inject a real reader or nothing at all; this type exists so
/// tests and degraded hosts share one typed unsupported answer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnsupportedCertificateAuthority;

impl CertificateAuthorityPort for UnsupportedCertificateAuthority {
    fn read_ca_file(&self, _path: &str) -> Result<CaFile, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "this host exposes no CA certificate file reader",
        ))
    }
}
