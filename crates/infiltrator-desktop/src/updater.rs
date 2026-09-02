//! Client silent atomic self-update engine.
//! Provides release checking, cryptographic integrity validation, staging,
//! atomic binary replacement with rollback, phased canary rollout, and graceful restart.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Target release channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Beta,
    Nightly,
}

impl Default for UpdateChannel {
    fn default() -> Self {
        Self::Stable
    }
}

impl std::fmt::Display for UpdateChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stable => write!(f, "stable"),
            Self::Beta => write!(f, "beta"),
            Self::Nightly => write!(f, "nightly"),
        }
    }
}

impl std::str::FromStr for UpdateChannel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "stable" => Ok(Self::Stable),
            "beta" => Ok(Self::Beta),
            "nightly" => Ok(Self::Nightly),
            other => Err(anyhow!("Unknown update channel: {}", other)),
        }
    }
}

/// Parsed semantic version representation (SemVer 2.0.0 compliant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre_release: Option<String>,
    pub build_metadata: Option<String>,
}

impl SemVer {
    /// Parses a semantic version string (e.g., "1.2.3", "v0.20.0-beta.1+build.42").
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim().trim_start_matches('v');
        if trimmed.is_empty() {
            return None;
        }

        // Split build metadata (+...)
        let mut build_parts = trimmed.splitn(2, '+');
        let version_core = build_parts.next()?;
        let build_metadata = build_parts.next().map(|b| b.to_string());

        // Split pre-release (-...)
        let mut pre_parts = version_core.splitn(2, '-');
        let numeric_core = pre_parts.next()?;
        let pre_release = pre_parts.next().map(|p| p.to_string());

        let mut nums = numeric_core.split('.');
        let major = nums.next()?.parse::<u64>().ok()?;
        let minor = nums.next().map(|n| n.parse::<u64>().ok()).unwrap_or(Some(0))?;
        let patch = nums.next().map(|n| n.parse::<u64>().ok()).unwrap_or(Some(0))?;

        if nums.next().is_some() {
            return None;
        }

        Some(Self {
            major,
            minor,
            patch,
            pre_release,
            build_metadata,
        })
    }

    pub fn is_prerelease(&self) -> bool {
        self.pre_release.is_some()
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref pre) = self.pre_release {
            write!(f, "-{}", pre)?;
        }
        if let Some(ref build) = self.build_metadata {
            write!(f, "+{}", build)?;
        }
        Ok(())
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Major, minor, patch comparison
        match (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }

        // SemVer 2.0.0 rule: normal version is greater than pre-release version
        match (&self.pre_release, &other.pre_release) {
            (None, None) => std::cmp::Ordering::Equal,
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(a), Some(b)) => compare_prerelease(a, b),
        }
    }
}

/// Helper to compare pre-release identifiers per SemVer 2.0.0 rules.
fn compare_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
    let parts_a: Vec<&str> = a.split('.').collect();
    let parts_b: Vec<&str> = b.split('.').collect();

    for (pa, pb) in parts_a.iter().zip(parts_b.iter()) {
        let na = pa.parse::<u64>().ok();
        let nb = pb.parse::<u64>().ok();

        match (na, nb) {
            (Some(num_a), Some(num_b)) => match num_a.cmp(&num_b) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            },
            (Some(_), None) => return std::cmp::Ordering::Less,
            (None, Some(_)) => return std::cmp::Ordering::Greater,
            (None, None) => match pa.cmp(pb) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            },
        }
    }

    parts_a.len().cmp(&parts_b.len())
}

/// Information about a delta / binary patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeltaPatchInfo {
    pub base_sha256: String,
    pub target_sha256: String,
    pub patch_url: String,
    pub patch_sha256: String,
    pub patch_size_bytes: u64,
}

/// Package artifact details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateArtifactInfo {
    pub name: String,
    pub target_triple: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub signature: Option<String>,
}

/// Remote update manifest describing available releases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub channel: UpdateChannel,
    pub release_date: String,
    pub release_notes: String,
    pub min_supported_version: Option<String>,
    pub critical_security_fix: bool,
    pub rollout_percentage: u8,
    pub artifacts: Vec<UpdateArtifactInfo>,
    pub deltas: Vec<DeltaPatchInfo>,
}

impl UpdateManifest {
    /// Finds the matching artifact for a given target architecture triple.
    pub fn find_artifact_for_target(&self, target_triple: &str) -> Option<&UpdateArtifactInfo> {
        self.artifacts
            .iter()
            .find(|a| a.target_triple.eq_ignore_ascii_case(target_triple.trim()))
    }

    /// Finds an available delta patch matching the client's current base binary hash.
    pub fn find_delta_patch(&self, base_sha256: &str) -> Option<&DeltaPatchInfo> {
        self.deltas
            .iter()
            .find(|d| d.base_sha256.eq_ignore_ascii_case(base_sha256.trim()))
    }
}

/// Result of evaluating update eligibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateEligibility {
    Eligible {
        target_version: String,
        artifact: Option<UpdateArtifactInfo>,
        critical: bool,
    },
    UpToDate {
        current_version: String,
    },
    DowngradeBlocked {
        current_version: String,
        target_version: String,
    },
    BelowMinSupportedVersion {
        current_version: String,
        min_supported_version: String,
    },
    RolloutGated {
        client_bucket: u8,
        rollout_percentage: u8,
    },
    ChannelMismatch {
        current_channel: UpdateChannel,
        manifest_channel: UpdateChannel,
    },
    InvalidVersion {
        reason: String,
    },
}

/// Current state of the self-update lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking,
    Available {
        version: String,
        notes: String,
        size_bytes: u64,
        critical: bool,
    },
    UpToDate,
    Downloading {
        bytes_downloaded: u64,
        total_bytes: u64,
    },
    Downloaded {
        staged_path: PathBuf,
    },
    Verifying,
    Staged {
        staged_path: PathBuf,
    },
    Applied {
        backup_path: PathBuf,
    },
    Rollback {
        reason: String,
    },
    Failed {
        error: String,
    },
}

/// Report after successfully applying an atomic binary update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateExecutionReport {
    pub target_version: String,
    pub backup_path: PathBuf,
    pub updated_binary: PathBuf,
    pub bytes_applied: u64,
}

/// Configuration options for the ClientUpdater instance.
#[derive(Debug, Clone)]
pub struct ClientUpdaterConfig {
    pub current_version: String,
    pub client_uuid: String,
    pub channel: UpdateChannel,
    pub target_binary: PathBuf,
    pub staging_dir: PathBuf,
    pub target_triple: Option<String>,
    pub allow_downgrade: bool,
}

/// Core updater coordinator and atomic execution engine.
pub struct ClientUpdater {
    config: ClientUpdaterConfig,
}

impl ClientUpdater {
    /// Creates a new ClientUpdater with custom configuration.
    pub fn new(config: ClientUpdaterConfig) -> Self {
        Self { config }
    }

    /// Evaluates whether an update in `manifest` is eligible for the current client,
    /// enforcing semver comparison, downgrade barrier, minimum supported version barrier,
    /// release channel compatibility, and phased canary rollout.
    pub fn check_eligibility(
        current_version: &str,
        manifest: &UpdateManifest,
        client_uuid: &str,
        current_channel: Option<UpdateChannel>,
        target_triple: Option<&str>,
    ) -> UpdateEligibility {
        let cur_v = match SemVer::parse(current_version) {
            Some(v) => v,
            None => {
                return UpdateEligibility::InvalidVersion {
                    reason: format!("Cannot parse current version: {}", current_version),
                };
            }
        };

        let target_v = match SemVer::parse(&manifest.version) {
            Some(v) => v,
            None => {
                return UpdateEligibility::InvalidVersion {
                    reason: format!("Cannot parse target version: {}", manifest.version),
                };
            }
        };

        // Channel verification
        if let Some(chan) = current_channel {
            if chan != manifest.channel && !manifest.critical_security_fix {
                return UpdateEligibility::ChannelMismatch {
                    current_channel: chan,
                    manifest_channel: manifest.channel,
                };
            }
        }

        // Downgrade Barrier: Target must be strictly newer than current version
        if target_v <= cur_v {
            if target_v == cur_v {
                return UpdateEligibility::UpToDate {
                    current_version: current_version.to_string(),
                };
            } else {
                return UpdateEligibility::DowngradeBlocked {
                    current_version: current_version.to_string(),
                    target_version: manifest.version.clone(),
                };
            }
        }

        // Minimum supported version barrier (prevents unsupported long jumps)
        if let Some(min_v_str) = &manifest.min_supported_version {
            if let Some(min_v) = SemVer::parse(min_v_str) {
                if cur_v < min_v && !manifest.critical_security_fix {
                    return UpdateEligibility::BelowMinSupportedVersion {
                        current_version: current_version.to_string(),
                        min_supported_version: min_v_str.clone(),
                    };
                }
            }
        }

        // Canary rollout / grayscale gating
        if manifest.rollout_percentage < 100 && !manifest.critical_security_fix {
            let bucket = Self::compute_rollout_bucket(client_uuid);
            if bucket >= manifest.rollout_percentage {
                return UpdateEligibility::RolloutGated {
                    client_bucket: bucket,
                    rollout_percentage: manifest.rollout_percentage,
                };
            }
        }

        let artifact = target_triple
            .and_then(|triple| manifest.find_artifact_for_target(triple))
            .cloned();

        UpdateEligibility::Eligible {
            target_version: manifest.version.clone(),
            artifact,
            critical: manifest.critical_security_fix,
        }
    }

    /// Convenience wrapper checking if an update is eligible.
    pub fn is_update_eligible(
        current_version: &str,
        manifest: &UpdateManifest,
        client_uuid: &str,
    ) -> bool {
        matches!(
            Self::check_eligibility(current_version, manifest, client_uuid, None, None),
            UpdateEligibility::Eligible { .. }
        )
    }

    /// Evaluates the instance against a manifest.
    pub fn evaluate_manifest(&self, manifest: &UpdateManifest) -> UpdateEligibility {
        if self.config.allow_downgrade {
            let artifact = self
                .config
                .target_triple
                .as_deref()
                .and_then(|t| manifest.find_artifact_for_target(t))
                .cloned();
            return UpdateEligibility::Eligible {
                target_version: manifest.version.clone(),
                artifact,
                critical: manifest.critical_security_fix,
            };
        }

        Self::check_eligibility(
            &self.config.current_version,
            manifest,
            &self.config.client_uuid,
            Some(self.config.channel),
            self.config.target_triple.as_deref(),
        )
    }

    /// Computes deterministic rollout bucket 0..99 from client UUID.
    pub fn compute_rollout_bucket(client_uuid: &str) -> u8 {
        let mut hasher = Sha256::new();
        hasher.update(b"infiltrator-rollout-v1:");
        hasher.update(client_uuid.as_bytes());
        let hash = hasher.finalize();
        let val = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        (val % 100) as u8
    }

    /// Checks if a client is included in a rollout percentage.
    pub fn is_in_rollout(client_uuid: &str, percentage: u8) -> bool {
        if percentage >= 100 {
            return true;
        }
        if percentage == 0 {
            return false;
        }
        Self::compute_rollout_bucket(client_uuid) < percentage
    }

    /// Computes hex-encoded SHA256 of byte slice.
    pub fn compute_sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut hex = String::with_capacity(result.len() * 2);
        for byte in result {
            use std::fmt::Write;
            let _ = write!(hex, "{:02x}", byte);
        }
        hex
    }

    /// Computes hex-encoded SHA256 of a file streamingly (avoids excessive RAM usage).
    pub fn compute_file_sha256(path: &Path) -> Result<String> {
        let mut file = fs::File::open(path)
            .with_context(|| format!("Failed to open file for SHA-256 hash: {}", path.display()))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        let result = hasher.finalize();
        let mut hex = String::with_capacity(result.len() * 2);
        for byte in result {
            use std::fmt::Write;
            let _ = write!(hex, "{:02x}", byte);
        }
        Ok(hex)
    }

    /// Verifies SHA256 checksum of payload bytes.
    pub fn verify_sha256(data: &[u8], expected_hex: &str) -> bool {
        let computed = Self::compute_sha256(data);
        computed.eq_ignore_ascii_case(expected_hex.trim())
    }

    /// Verifies SHA256 checksum of a file on disk.
    pub fn verify_file_sha256(path: &Path, expected_hex: &str) -> Result<bool> {
        let computed = Self::compute_file_sha256(path)?;
        Ok(computed.eq_ignore_ascii_case(expected_hex.trim()))
    }

    /// Stages a downloaded update payload to disk after SHA256 integrity validation.
    pub fn stage_payload(
        data: &[u8],
        staging_dir: &Path,
        filename: &str,
        expected_sha256: &str,
    ) -> Result<PathBuf> {
        if !Self::verify_sha256(data, expected_sha256) {
            return Err(anyhow!(
                "SHA-256 integrity verification failed for staged payload"
            ));
        }

        fs::create_dir_all(staging_dir)
            .with_context(|| format!("Failed to create staging dir: {}", staging_dir.display()))?;

        let staged_file = staging_dir.join(filename);
        fs::write(&staged_file, data)
            .with_context(|| format!("Failed to write staged file: {}", staged_file.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&staged_file, fs::Permissions::from_mode(0o755));
        }

        Ok(staged_file)
    }

    /// Performs an atomic update of `target_binary` using `staged_binary`.
    /// On Windows: Utilizes `.old` rename mechanism to swap executing binaries safely.
    /// On POSIX: Utilizes atomic `renameat` semantics via a local staging copy with fallback.
    /// Creates a `.bak` backup file before replacing, which can be restored on rollback.
    pub fn apply_atomic_update(target_binary: &Path, staged_binary: &Path) -> Result<PathBuf> {
        if !staged_binary.exists() {
            return Err(anyhow!(
                "Staged binary does not exist at {}",
                staged_binary.display()
            ));
        }

        let parent = target_binary
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let file_stem = target_binary
            .file_name()
            .ok_or_else(|| anyhow!("Invalid target binary filename: {}", target_binary.display()))?
            .to_string_lossy();

        let backup_path = parent.join(format!("{}.bak", file_stem));
        let old_path = parent.join(format!("{}.old", file_stem));
        let temp_staged = parent.join(format!("{}.tmp.{}", file_stem, std::process::id()));

        // Clean up any stale .old or .bak files in target directory
        let _ = fs::remove_file(&old_path);
        let _ = fs::remove_file(&backup_path);
        let _ = fs::remove_file(&temp_staged);

        if target_binary.exists() {
            // Create reliable backup before replacing
            fs::copy(target_binary, &backup_path).with_context(|| {
                format!(
                    "Failed to create backup from {} to {}",
                    target_binary.display(),
                    backup_path.display()
                )
            })?;
        }

        #[cfg(windows)]
        {
            // Windows allows renaming open executing files, but forbids direct overwriting.
            // 1. Rename active target -> .old
            // 2. Move staged binary -> target
            // 3. Rollback on failure
            if target_binary.exists() {
                fs::rename(target_binary, &old_path).with_context(|| {
                    format!(
                        "Failed to rename active Windows binary {} to .old",
                        target_binary.display()
                    )
                })?;
            }

            if let Err(e) = fs::rename(staged_binary, target_binary) {
                // Try copy + remove if cross-device rename fails
                let copy_res = fs::copy(staged_binary, target_binary)
                    .map(|_| ())
                    .map_err(|ce| anyhow!("Rename failed ({e}) and copy failed: {ce}"));

                if let Err(ce) = copy_res {
                    // Critical rollback: restore target from .old if it was renamed
                    if old_path.exists() {
                        let _ = fs::rename(&old_path, target_binary);
                    }
                    return Err(anyhow!("Failed to replace target binary on Windows: {}", ce));
                }
                let _ = fs::remove_file(staged_binary);
            }
        }

        #[cfg(not(windows))]
        {
            // POSIX atomic replacement:
            // Ensure staged file is in the same directory (file system) to guarantee atomic renameat.
            let prep_result = (|| -> Result<()> {
                fs::copy(staged_binary, &temp_staged)?;
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&temp_staged, fs::Permissions::from_mode(0o755));
                fs::rename(&temp_staged, target_binary)?;
                let _ = fs::remove_file(staged_binary);
                Ok(())
            })();

            if let Err(err) = prep_result {
                let _ = fs::remove_file(&temp_staged);
                // Attempt automatic rollback if target was corrupted
                if backup_path.exists() && (!target_binary.exists() || fs::metadata(target_binary).map(|m| m.len()).unwrap_or(0) == 0) {
                    let _ = fs::copy(&backup_path, target_binary);
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(target_binary, fs::Permissions::from_mode(0o755));
                }
                return Err(err.context(format!(
                    "Failed POSIX atomic rename for binary at {}",
                    target_binary.display()
                )));
            }

            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(target_binary, fs::Permissions::from_mode(0o755));
        }

        Ok(backup_path)
    }

    /// Rolls back an applied update by restoring the backup binary.
    pub fn rollback(target_binary: &Path, backup_path: &Path) -> Result<()> {
        if !backup_path.exists() {
            return Err(anyhow!(
                "Rollback failed: Backup file does not exist at {}",
                backup_path.display()
            ));
        }

        let parent = target_binary
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let file_stem = target_binary
            .file_name()
            .ok_or_else(|| anyhow!("Invalid target binary filename"))?
            .to_string_lossy();

        #[cfg(windows)]
        {
            let temp_bad = parent.join(format!("{}.bad", file_stem));
            let _ = fs::remove_file(&temp_bad);
            if target_binary.exists() {
                let _ = fs::rename(target_binary, &temp_bad);
            }
            fs::copy(backup_path, target_binary)
                .with_context(|| format!("Failed to restore backup binary on Windows"))?;
            let _ = fs::remove_file(&temp_bad);
        }

        #[cfg(not(windows))]
        {
            let temp_restore = parent.join(format!("{}.restore.{}", file_stem, std::process::id()));
            fs::copy(backup_path, &temp_restore)?;
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&temp_restore, fs::Permissions::from_mode(0o755));
            fs::rename(&temp_restore, target_binary)
                .with_context(|| format!("Failed to atomically restore backup binary on POSIX"))?;
            let _ = fs::set_permissions(target_binary, fs::Permissions::from_mode(0o755));
        }

        Ok(())
    }

    /// Cleans up leftover `.old`, `.bak`, or temporary artifacts from previous updates.
    pub fn cleanup_old_artifacts(target_binary: &Path) -> Result<()> {
        let parent = target_binary
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let file_stem = match target_binary.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return Ok(()),
        };

        let old_file = parent.join(format!("{}.old", file_stem));
        let bak_file = parent.join(format!("{}.bak", file_stem));
        let bad_file = parent.join(format!("{}.bad", file_stem));

        let _ = fs::remove_file(old_file);
        let _ = fs::remove_file(bak_file);
        let _ = fs::remove_file(bad_file);
        Ok(())
    }

    /// Cleans up temporary staging directory and its files.
    pub fn cleanup_staging(staging_dir: &Path) -> Result<()> {
        if staging_dir.exists() {
            fs::remove_dir_all(staging_dir).with_context(|| {
                format!("Failed to clean staging directory: {}", staging_dir.display())
            })?;
        }
        Ok(())
    }

    /// High-level method: Stages and atomically applies binary payload from memory.
    pub fn apply_update_payload(
        &self,
        payload: &[u8],
        target_version: &str,
        expected_sha256: &str,
    ) -> Result<UpdateExecutionReport> {
        let filename = self
            .config
            .target_binary
            .file_name()
            .ok_or_else(|| anyhow!("Invalid target binary"))?
            .to_string_lossy();

        let staged = Self::stage_payload(
            payload,
            &self.config.staging_dir,
            &filename,
            expected_sha256,
        )?;

        let backup_path = Self::apply_atomic_update(&self.config.target_binary, &staged)?;

        let _ = Self::cleanup_staging(&self.config.staging_dir);

        Ok(UpdateExecutionReport {
            target_version: target_version.to_string(),
            backup_path,
            updated_binary: self.config.target_binary.clone(),
            bytes_applied: payload.len() as u64,
        })
    }
}

/// Helper to parse semver string (e.g., "0.20.1", "v1.2.3").
pub fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    SemVer::parse(version).map(|v| (v.major, v.minor, v.patch))
}

#[cfg(test)]
#[path = "updater_test.rs"]
mod updater_test;
