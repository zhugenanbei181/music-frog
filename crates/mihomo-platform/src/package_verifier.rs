use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PackageType {
    WindowsMsi,
    WindowsNsisExe,
    LinuxAppImage,
    LinuxDeb,
    MacOsDmg,
    AndroidApk,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SignatureStatus {
    Valid(String),
    Unsigned,
    Invalid(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PackageArtifactInfo {
    pub name: String,
    pub pkg_type: PackageType,
    pub sha256: String,
    pub size_bytes: u64,
    pub signature_status: SignatureStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ReleaseManifest {
    pub version: String,
    pub artifacts: Vec<PackageArtifactInfo>,
}

pub struct PackageManifestVerifier;

impl PackageManifestVerifier {
    /// Verifies if the SHA256 checksum of the given data matches the expected hex string.
    pub fn verify_sha256(data: &[u8], expected_hex: &str) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        
        let mut computed_hex = String::with_capacity(64);
        for byte in result {
            // Using standard format for hex
            computed_hex.push_str(&format!("{:02x}", byte));
        }
        
        computed_hex.eq_ignore_ascii_case(expected_hex)
    }

    /// Generates a JSON release manifest string from a version and list of artifacts.
    pub fn generate_release_manifest(version: &str, artifacts: &[PackageArtifactInfo]) -> String {
        let manifest = ReleaseManifest {
            version: version.to_string(),
            artifacts: artifacts.to_vec(),
        };
        serde_json::to_string_pretty(&manifest).unwrap_or_default()
    }

    /// Parses a JSON release manifest string into a version string and list of artifacts.
    pub fn parse_release_manifest(manifest_json: &str) -> Result<(String, Vec<PackageArtifactInfo>)> {
        let manifest: ReleaseManifest = serde_json::from_str(manifest_json)
            .context("Failed to parse release manifest JSON")?;
        Ok((manifest.version, manifest.artifacts))
    }

    /// Infers the package type based on the file extension.
    pub fn infer_package_type_from_filename(filename: &str) -> Option<PackageType> {
        let lower = filename.to_lowercase();
        if lower.ends_with(".msi") {
            Some(PackageType::WindowsMsi)
        } else if lower.ends_with(".exe") {
            Some(PackageType::WindowsNsisExe)
        } else if lower.ends_with(".appimage") {
            Some(PackageType::LinuxAppImage)
        } else if lower.ends_with(".deb") {
            Some(PackageType::LinuxDeb)
        } else if lower.ends_with(".dmg") {
            Some(PackageType::MacOsDmg)
        } else if lower.ends_with(".apk") {
            Some(PackageType::AndroidApk)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_sha256() {
        let data = b"hello world";
        // sha256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        
        // Exact match
        assert!(PackageManifestVerifier::verify_sha256(data, expected));
        
        // Case insensitive match
        assert!(PackageManifestVerifier::verify_sha256(data, &expected.to_uppercase()));
        
        // Invalid match
        assert!(!PackageManifestVerifier::verify_sha256(data, "invalid"));
    }

    #[test]
    fn test_infer_package_type() {
        assert_eq!(PackageManifestVerifier::infer_package_type_from_filename("app_installer.msi"), Some(PackageType::WindowsMsi));
        assert_eq!(PackageManifestVerifier::infer_package_type_from_filename("setup.exe"), Some(PackageType::WindowsNsisExe));
        assert_eq!(PackageManifestVerifier::infer_package_type_from_filename("MusicFrog.AppImage"), Some(PackageType::LinuxAppImage));
        assert_eq!(PackageManifestVerifier::infer_package_type_from_filename("package.deb"), Some(PackageType::LinuxDeb));
        assert_eq!(PackageManifestVerifier::infer_package_type_from_filename("installer.dmg"), Some(PackageType::MacOsDmg));
        assert_eq!(PackageManifestVerifier::infer_package_type_from_filename("app.apk"), Some(PackageType::AndroidApk));
        assert_eq!(PackageManifestVerifier::infer_package_type_from_filename("source.tar.gz"), None);
    }

    #[test]
    fn test_manifest_round_trip() {
        let version = "1.0.0-alpha.1";
        let artifacts = vec![
            PackageArtifactInfo {
                name: "MusicFrog-1.0.0.msi".to_string(),
                pkg_type: PackageType::WindowsMsi,
                sha256: "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9".to_string(),
                size_bytes: 4096000,
                signature_status: SignatureStatus::Valid("TrustedCert".to_string()),
            },
            PackageArtifactInfo {
                name: "MusicFrog.AppImage".to_string(),
                pkg_type: PackageType::LinuxAppImage,
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
                size_bytes: 2048000,
                signature_status: SignatureStatus::Unsigned,
            },
            PackageArtifactInfo {
                name: "MusicFrog-mac.dmg".to_string(),
                pkg_type: PackageType::MacOsDmg,
                sha256: "deadbeef".to_string(),
                size_bytes: 1024000,
                signature_status: SignatureStatus::Invalid("ExpiredCert".to_string()),
            },
        ];

        // Generate JSON
        let manifest_json = PackageManifestVerifier::generate_release_manifest(version, &artifacts);
        
        // Ensure it's valid JSON format
        assert!(manifest_json.contains("\"version\": \"1.0.0-alpha.1\""));
        
        // Parse back
        let (parsed_version, parsed_artifacts) = PackageManifestVerifier::parse_release_manifest(&manifest_json).expect("Failed to parse manifest");
        
        // Validate matching data
        assert_eq!(parsed_version, version);
        assert_eq!(parsed_artifacts, artifacts);
    }
}
