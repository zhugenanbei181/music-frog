//! Backup and export/import utilities for all local configurations, profiles, and settings.

use chrono::Utc;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};
use std::num::NonZeroU32;
use std::path::Path;
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::history::SnapshotMeta;
use crate::settings_io::settings_path;

#[cfg(test)]
#[path = "backup_test.rs"]
mod tests;

const ENCRYPTED_MAGIC: &[u8; 16] = b"IFTR_BACKUP_V1\x00\x00";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const PBKDF2_ITERATIONS: u32 = 100_000;
const HEADER_LEN: usize = 16 + SALT_LEN + NONCE_LEN + 4; // 48 bytes

/// Errors that can occur during backup generation, export, import, encryption, or decryption.
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Zip archive error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Password cannot be empty")]
    EmptyPassword,

    #[error("Invalid backup format: {0}")]
    InvalidFormat(String),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: incorrect password or corrupted data")]
    DecryptionFailed,

    #[error("Integrity check failed: expected digest {expected}, computed {actual}")]
    IntegrityMismatch { expected: String, actual: String },
}

pub type Result<T, E = BackupError> = std::result::Result<T, E>;

/// Represents a single configuration profile stored inside a backup bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileBackupItem {
    pub name: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_yaml: Option<String>,
}

impl ProfileBackupItem {
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            options_yaml: None,
        }
    }

    pub fn with_options(
        name: impl Into<String>,
        content: impl Into<String>,
        options_yaml: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            options_yaml,
        }
    }
}

/// Metadata manifest describing bundle version, timestamp, client version, and SHA256 payload digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub version: String,
    pub created_at: String,
    pub client_version: String,
    pub sha256_digest: String,
}

impl BackupManifest {
    pub const CURRENT_BUNDLE_VERSION: &'static str = "1.0.0";

    pub fn new(client_version: impl Into<String>, sha256_digest: impl Into<String>) -> Self {
        Self {
            version: Self::CURRENT_BUNDLE_VERSION.to_string(),
            created_at: Utc::now().to_rfc3339(),
            client_version: client_version.into(),
            sha256_digest: sha256_digest.into(),
        }
    }
}

/// Standalone portable backup bundle capturing profiles, settings, mixin overlay, and manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupBundle {
    pub manifest: BackupManifest,
    pub settings_toml: String,
    pub mixin_yaml: String,
    pub profiles: Vec<ProfileBackupItem>,
}

impl BackupBundle {
    pub fn new(
        profiles: Vec<ProfileBackupItem>,
        settings_toml: String,
        mixin_yaml: String,
    ) -> Self {
        let digest = compute_sha256_digest(&profiles, &settings_toml, &mixin_yaml);
        let manifest = BackupManifest::new(env!("CARGO_PKG_VERSION"), digest);
        Self {
            manifest,
            settings_toml,
            mixin_yaml,
            profiles,
        }
    }

    pub fn with_client_version(
        profiles: Vec<ProfileBackupItem>,
        settings_toml: String,
        mixin_yaml: String,
        client_version: impl Into<String>,
    ) -> Self {
        let digest = compute_sha256_digest(&profiles, &settings_toml, &mixin_yaml);
        let manifest = BackupManifest::new(client_version, digest);
        Self {
            manifest,
            settings_toml,
            mixin_yaml,
            profiles,
        }
    }

    /// Computes the deterministic SHA256 digest over the bundle payload.
    pub fn compute_digest(&self) -> String {
        compute_sha256_digest(&self.profiles, &self.settings_toml, &self.mixin_yaml)
    }

    /// Verifies if the manifest SHA256 digest matches the calculated payload digest.
    pub fn verify_digest(&self) -> bool {
        self.manifest.sha256_digest == self.compute_digest()
    }

    /// Validates the manifest checksum and returns an error if mismatched.
    pub fn validate_checksum(&self) -> Result<()> {
        let actual = self.compute_digest();
        if self.manifest.sha256_digest != actual {
            return Err(BackupError::IntegrityMismatch {
                expected: self.manifest.sha256_digest.clone(),
                actual,
            });
        }
        Ok(())
    }

    /// Recalculates and updates the manifest SHA256 digest.
    pub fn update_digest(&mut self) {
        self.manifest.sha256_digest = self.compute_digest();
    }

    /// Serializes this bundle into an unencrypted JSON string.
    pub fn to_json(&self) -> Result<String> {
        export_json_bundle(self)
    }

    /// Deserializes a bundle from a JSON string and verifies its checksum.
    pub fn from_json(json_str: &str) -> Result<Self> {
        import_json_bundle(json_str)
    }

    /// Serializes this bundle into unencrypted ZIP archive bytes.
    pub fn to_zip(&self) -> Result<Vec<u8>> {
        export_zip_bundle(self)
    }

    /// Deserializes a bundle from ZIP archive bytes and verifies its checksum.
    pub fn from_zip(zip_bytes: &[u8]) -> Result<Self> {
        import_zip_bundle(zip_bytes)
    }

    /// Exports this bundle as an encrypted byte buffer protected by `password`.
    pub fn export_encrypted(&self, password: &str) -> Result<Vec<u8>> {
        export_encrypted_bundle(self, password)
    }

    /// Imports and decrypts a bundle from an encrypted byte buffer with `password`.
    pub fn import_encrypted(encrypted_bytes: &[u8], password: &str) -> Result<Self> {
        import_encrypted_bundle(encrypted_bytes, password)
    }
}

/// Deterministically computes the SHA256 checksum over profiles, settings, and mixin configurations.
pub fn compute_sha256_digest(
    profiles: &[ProfileBackupItem],
    settings_toml: &str,
    mixin_yaml: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"INFILTRATOR_BACKUP_DIGEST_V1\n");
    hasher.update(settings_toml.as_bytes());
    hasher.update(b"\n---SETTINGS_END---\n");
    hasher.update(mixin_yaml.as_bytes());
    hasher.update(b"\n---MIXIN_END---\n");

    let mut sorted_profiles: Vec<&ProfileBackupItem> = profiles.iter().collect();
    sorted_profiles.sort_by(|a, b| a.name.cmp(&b.name));

    for item in sorted_profiles {
        hasher.update(item.name.as_bytes());
        hasher.update(b"\n");
        hasher.update(item.content.as_bytes());
        hasher.update(b"\n");
        if let Some(opts) = &item.options_yaml {
            hasher.update(opts.as_bytes());
            hasher.update(b"\n");
        }
        hasher.update(b"---PROFILE_END---\n");
    }

    let result = hasher.finalize();
    let mut hex = String::with_capacity(result.len() * 2);
    for byte in result {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", byte);
    }
    hex
}

/// Export a bundle to an encrypted byte buffer using AES-256-GCM and PBKDF2-HMAC-SHA256 key derivation.
pub fn export_encrypted_bundle(bundle: &BackupBundle, password: &str) -> Result<Vec<u8>> {
    if password.trim().is_empty() {
        return Err(BackupError::EmptyPassword);
    }

    bundle.validate_checksum()?;
    let plaintext = serde_json::to_vec(bundle)?;

    let rng = SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    rng.fill(&mut salt)
        .map_err(|e| BackupError::EncryptionFailed(format!("RNG salt generation error: {e:?}")))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes)
        .map_err(|e| BackupError::EncryptionFailed(format!("RNG nonce generation error: {e:?}")))?;

    let n_iter = NonZeroU32::new(PBKDF2_ITERATIONS)
        .ok_or_else(|| BackupError::EncryptionFailed("Invalid iterations count".into()))?;

    let mut key_bytes = [0u8; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        n_iter,
        &salt,
        password.as_bytes(),
        &mut key_bytes,
    );

    let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
        .map_err(|e| BackupError::EncryptionFailed(format!("Key initialization error: {e:?}")))?;
    let key = LessSafeKey::new(unbound_key);
    let nonce = Nonce::try_assume_unique_for_key(&nonce_bytes)
        .map_err(|e| BackupError::EncryptionFailed(format!("Nonce error: {e:?}")))?;

    let mut in_out = plaintext;
    let mut aad_bytes = Vec::with_capacity(20);
    aad_bytes.extend_from_slice(ENCRYPTED_MAGIC);
    aad_bytes.extend_from_slice(&PBKDF2_ITERATIONS.to_be_bytes());

    key.seal_in_place_append_tag(nonce, Aad::from(&aad_bytes), &mut in_out)
        .map_err(|e| BackupError::EncryptionFailed(format!("AES-GCM encryption error: {e:?}")))?;

    let mut output = Vec::with_capacity(HEADER_LEN + in_out.len());
    output.extend_from_slice(ENCRYPTED_MAGIC);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&PBKDF2_ITERATIONS.to_be_bytes());
    output.extend_from_slice(&in_out);

    Ok(output)
}

/// Import and decrypt a bundle from an encrypted byte buffer using `password`.
pub fn import_encrypted_bundle(encrypted_bytes: &[u8], password: &str) -> Result<BackupBundle> {
    if password.trim().is_empty() {
        return Err(BackupError::EmptyPassword);
    }

    if encrypted_bytes.len() < HEADER_LEN + 16 {
        return Err(BackupError::InvalidFormat("Payload too short".into()));
    }

    if &encrypted_bytes[..16] != ENCRYPTED_MAGIC {
        return Err(BackupError::InvalidFormat(
            "Invalid encrypted header magic".into(),
        ));
    }

    let salt = &encrypted_bytes[16..32];
    let nonce_bytes: [u8; NONCE_LEN] = encrypted_bytes[32..44]
        .try_into()
        .map_err(|_| BackupError::InvalidFormat("Corrupted nonce".into()))?;
    let iterations = u32::from_be_bytes(
        encrypted_bytes[44..48]
            .try_into()
            .map_err(|_| BackupError::InvalidFormat("Corrupted iterations".into()))?,
    );

    if iterations == 0 || iterations > 5_000_000 {
        return Err(BackupError::InvalidFormat("Invalid iteration count".into()));
    }

    let n_iter = NonZeroU32::new(iterations)
        .ok_or_else(|| BackupError::InvalidFormat("Zero iteration count".into()))?;

    let mut key_bytes = [0u8; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        n_iter,
        salt,
        password.as_bytes(),
        &mut key_bytes,
    );

    let unbound_key =
        UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|_| BackupError::DecryptionFailed)?;
    let key = LessSafeKey::new(unbound_key);
    let nonce = Nonce::try_assume_unique_for_key(&nonce_bytes)
        .map_err(|_| BackupError::DecryptionFailed)?;

    let mut ciphertext = encrypted_bytes[HEADER_LEN..].to_vec();
    let mut aad_bytes = Vec::with_capacity(20);
    aad_bytes.extend_from_slice(ENCRYPTED_MAGIC);
    aad_bytes.extend_from_slice(&iterations.to_be_bytes());

    let decrypted_bytes = key
        .open_in_place(nonce, Aad::from(&aad_bytes), &mut ciphertext)
        .map_err(|_| BackupError::DecryptionFailed)?;

    let bundle: BackupBundle = serde_json::from_slice(decrypted_bytes)?;
    bundle.validate_checksum()?;

    Ok(bundle)
}

/// Export a bundle to a formatted JSON string.
pub fn export_json_bundle(bundle: &BackupBundle) -> Result<String> {
    bundle.validate_checksum()?;
    Ok(serde_json::to_string_pretty(bundle)?)
}

/// Import a bundle from a JSON string and verify its checksum.
pub fn import_json_bundle(json_str: &str) -> Result<BackupBundle> {
    let bundle: BackupBundle = serde_json::from_str(json_str)?;
    bundle.validate_checksum()?;
    Ok(bundle)
}

/// Export a bundle into a standard unencrypted ZIP archive byte vector.
pub fn export_zip_bundle(bundle: &BackupBundle) -> Result<Vec<u8>> {
    bundle.validate_checksum()?;

    let mut buffer = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut buffer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. Manifest
    zip.start_file("manifest.json", options)?;
    let manifest_json = serde_json::to_string_pretty(&bundle.manifest)?;
    zip.write_all(manifest_json.as_bytes())?;

    // 2. Settings
    zip.start_file("settings.toml", options)?;
    zip.write_all(bundle.settings_toml.as_bytes())?;

    // 3. Mixin
    zip.start_file("mixin.yaml", options)?;
    zip.write_all(bundle.mixin_yaml.as_bytes())?;

    // 4. Profiles
    for profile in &bundle.profiles {
        let profile_file = format!("profiles/{}.yaml", profile.name);
        zip.start_file(profile_file, options)?;
        zip.write_all(profile.content.as_bytes())?;

        if let Some(opts) = &profile.options_yaml {
            let options_file = format!("profiles/{}.options.yaml", profile.name);
            zip.start_file(options_file, options)?;
            zip.write_all(opts.as_bytes())?;
        }
    }

    zip.finish()?;
    Ok(buffer.into_inner())
}

/// Import a bundle from ZIP archive bytes, verifying structure and checksum.
pub fn import_zip_bundle(zip_bytes: &[u8]) -> Result<BackupBundle> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor)?;

    let mut manifest_opt: Option<BackupManifest> = None;
    let mut settings_toml = String::new();
    let mut mixin_yaml = String::new();
    let mut profile_contents: HashMap<String, String> = HashMap::new();
    let mut profile_options: HashMap<String, String> = HashMap::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if file.is_dir() {
            continue;
        }

        // Security check: prevent zip slip
        if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
            return Err(BackupError::InvalidFormat(format!(
                "Suspicious file path in zip: {name}"
            )));
        }

        let mut content = String::new();
        file.read_to_string(&mut content)?;

        if name == "manifest.json" {
            manifest_opt = Some(serde_json::from_str(&content)?);
        } else if name == "settings.toml" {
            settings_toml = content;
        } else if name == "mixin.yaml" {
            mixin_yaml = content;
        } else if let Some(subpath) = name.strip_prefix("profiles/") {
            if let Some(profile_name) = subpath.strip_suffix(".options.yaml") {
                profile_options.insert(profile_name.to_string(), content);
            } else if let Some(profile_name) = subpath
                .strip_suffix(".yaml")
                .or_else(|| subpath.strip_suffix(".yml"))
            {
                profile_contents.insert(profile_name.to_string(), content);
            }
        }
    }

    let manifest = manifest_opt
        .ok_or_else(|| BackupError::InvalidFormat("Zip archive missing manifest.json".into()))?;

    let mut profiles = Vec::new();
    for (name, content) in profile_contents {
        let options_yaml = profile_options.remove(&name);
        profiles.push(ProfileBackupItem {
            name,
            content,
            options_yaml,
        });
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));

    let bundle = BackupBundle {
        manifest,
        settings_toml,
        mixin_yaml,
        profiles,
    };

    bundle.validate_checksum()?;
    Ok(bundle)
}

/// Intelligent snapshot pruning: retains newest snapshots, deduplicates identical SHA256 hashes,
/// and trims older snapshots exceeding `max_retain` per profile.
/// Returns the list of snapshot paths that should be deleted.
pub fn prune_snapshots(snapshots: &[SnapshotMeta], max_retain: usize) -> Vec<String> {
    if snapshots.is_empty() {
        return Vec::new();
    }

    let mut grouped: HashMap<&str, Vec<&SnapshotMeta>> = HashMap::new();
    for snap in snapshots {
        grouped.entry(&snap.profile).or_default().push(snap);
    }

    let mut pruned_paths = Vec::new();

    for (_profile, mut profile_snaps) in grouped {
        profile_snaps.sort_by_key(|s| std::cmp::Reverse(s.timestamp));

        let mut seen_hashes = HashSet::new();
        let mut retained_count = 0;

        for snap in profile_snaps {
            let path_str = snap.path.to_string_lossy().to_string();

            if max_retain == 0
                || retained_count >= max_retain
                || !seen_hashes.insert(snap.sha256.clone())
            {
                pruned_paths.push(path_str);
            } else {
                retained_count += 1;
            }
        }
    }

    pruned_paths
}

/// Export all active configs and settings into a single JSON backup bundle string.
pub async fn export_all_configs_bundle(base_dir: &Path) -> anyhow::Result<String> {
    let bundle = export_bundle_from_dir(base_dir).await?;
    Ok(serde_json::to_string_pretty(&bundle)?)
}

/// Reads local configurations, profiles, and options from `base_dir` into a `BackupBundle`.
pub async fn export_bundle_from_dir(base_dir: &Path) -> Result<BackupBundle> {
    let settings_p =
        settings_path(base_dir).map_err(|e| BackupError::InvalidFormat(e.to_string()))?;
    let settings_toml = if tokio::fs::try_exists(&settings_p).await.unwrap_or(false) {
        tokio::fs::read_to_string(&settings_p).await?
    } else {
        String::new()
    };

    let mixin_p = base_dir.join("mixin.yaml");
    let mixin_yaml = if tokio::fs::try_exists(&mixin_p).await.unwrap_or(false) {
        tokio::fs::read_to_string(&mixin_p).await?
    } else {
        String::new()
    };

    let configs_dir = base_dir.join("configs");
    let options_dir = base_dir.join("options");
    let mut profiles = Vec::new();

    if tokio::fs::try_exists(&configs_dir).await.unwrap_or(false) {
        let mut entries = tokio::fs::read_dir(&configs_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let is_yaml =
                path.is_file() && path.extension().is_some_and(|e| e == "yaml" || e == "yml");
            if is_yaml && let Ok(content) = tokio::fs::read_to_string(&path).await {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let opt_path = options_dir.join(format!("{stem}.yaml"));
                let options_yaml = if tokio::fs::try_exists(&opt_path).await.unwrap_or(false) {
                    tokio::fs::read_to_string(&opt_path).await.ok()
                } else {
                    None
                };

                profiles.push(ProfileBackupItem {
                    name: stem,
                    content,
                    options_yaml,
                });
            }
        }
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(BackupBundle::new(profiles, settings_toml, mixin_yaml))
}

/// Restores a `BackupBundle` into `base_dir`.
pub async fn restore_bundle_to_dir(
    bundle: &BackupBundle,
    base_dir: &Path,
    overwrite: bool,
) -> Result<()> {
    bundle.validate_checksum()?;

    let settings_p =
        settings_path(base_dir).map_err(|e| BackupError::InvalidFormat(e.to_string()))?;
    if let Some(parent) = settings_p.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if overwrite || !tokio::fs::try_exists(&settings_p).await.unwrap_or(false) {
        tokio::fs::write(&settings_p, &bundle.settings_toml).await?;
    }

    let mixin_p = base_dir.join("mixin.yaml");
    if !bundle.mixin_yaml.is_empty()
        && (overwrite || !tokio::fs::try_exists(&mixin_p).await.unwrap_or(false))
    {
        tokio::fs::write(&mixin_p, &bundle.mixin_yaml).await?;
    }

    let configs_dir = base_dir.join("configs");
    let options_dir = base_dir.join("options");
    tokio::fs::create_dir_all(&configs_dir).await?;

    for profile in &bundle.profiles {
        let p_path = configs_dir.join(format!("{}.yaml", profile.name));
        if overwrite || !tokio::fs::try_exists(&p_path).await.unwrap_or(false) {
            tokio::fs::write(&p_path, &profile.content).await?;
        }

        if let Some(opts) = &profile.options_yaml {
            tokio::fs::create_dir_all(&options_dir).await?;
            let opt_path = options_dir.join(format!("{}.yaml", profile.name));
            if overwrite || !tokio::fs::try_exists(&opt_path).await.unwrap_or(false) {
                tokio::fs::write(&opt_path, opts).await?;
            }
        }
    }

    Ok(())
}
