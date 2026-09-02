//! Filesystem sandbox path validation, portable mode detection, and crash recovery mode.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// The result of validating a path against a sandbox.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PathValidationResult {
    /// The path is valid and within the sandbox.
    Allowed,
    /// The path contains directory traversal components (e.g., `..`).
    DeniedTraversal,
    /// The path points outside the allowed sandbox directory.
    DeniedOutsideSandbox,
    /// The path is fundamentally invalid (e.g., malformed).
    InvalidPath,
}

/// A validator that ensures filesystem paths stay within a permitted root directory.
pub struct SandboxValidator {
    allowed_root: PathBuf,
}

impl SandboxValidator {
    /// Creates a new `SandboxValidator` with the given allowed root directory.
    pub fn new(allowed_root: PathBuf) -> Self {
        Self { allowed_root }
    }

    /// Validates a candidate path to ensure it is safe and resides within the sandbox.
    pub fn validate_path(&self, candidate: &Path) -> PathValidationResult {
        // Reject `..` (ParentDir) components as DeniedTraversal.
        for component in candidate.components() {
            if let Component::ParentDir = component {
                return PathValidationResult::DeniedTraversal;
            }
        }

        // Construct a resolved absolute path.
        let mut resolved = PathBuf::new();
        if candidate.is_absolute() {
            resolved = candidate.to_path_buf();
        } else {
            resolved.push(&self.allowed_root);
            resolved.push(candidate);
        }

        // Normalize the path by removing `.` (CurDir) and resolving any existing `..` (ParentDir).
        let mut normalized = PathBuf::new();
        for component in resolved.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized.pop();
                }
                _ => normalized.push(component),
            }
        }

        // Normalize the allowed root directory as well.
        let mut normalized_root = PathBuf::new();
        for component in self.allowed_root.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized_root.pop();
                }
                _ => normalized_root.push(component),
            }
        }

        // Ensure the normalized path starts with the normalized allowed root.
        if !normalized.starts_with(&normalized_root) {
            return PathValidationResult::DeniedOutsideSandbox;
        }

        PathValidationResult::Allowed
    }

    /// Sanitizes a filename by removing dangerous characters.
    pub fn sanitize_filename(filename: &str) -> String {
        filename
            .chars()
            .filter(|&c| {
                !matches!(
                    c,
                    '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'
                )
            })
            .collect()
    }
}

/// Portable mode detector: verifies if the client is running in zero-registry portable mode.
pub struct PortableModeDetector;

impl PortableModeDetector {
    /// Detects if portable directory (`data/` or `.portable` marker) exists alongside the executable.
    pub fn detect_portable_dir(exe_dir: &Path) -> Option<PathBuf> {
        let data_dir = exe_dir.join("data");
        if data_dir.is_dir() {
            return Some(data_dir);
        }

        let marker = exe_dir.join(".portable");
        if marker.exists() {
            return Some(exe_dir.join("data"));
        }

        None
    }
}

/// Safe Mode watchdog for consecutive crash detection and automatic recovery.
pub struct SafeModeRecovery;

impl SafeModeRecovery {
    const CRASH_COUNTER_FILE: &'static str = ".crash_counter";
    const THRESHOLD: u32 = 2;

    pub fn is_safe_mode_active(home_dir: &Path) -> bool {
        let path = home_dir.join(Self::CRASH_COUNTER_FILE);
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(count) = content.trim().parse::<u32>()
        {
            count >= Self::THRESHOLD
        } else {
            false
        }
    }

    pub fn record_crash(home_dir: &Path) -> u32 {
        let path = home_dir.join(Self::CRASH_COUNTER_FILE);
        let current_count: u32 = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let new_count = current_count.saturating_add(1);
        let _ = std::fs::write(&path, new_count.to_string());
        new_count
    }

    pub fn record_clean_exit(home_dir: &Path) {
        let path = home_dir.join(Self::CRASH_COUNTER_FILE);
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_subpath_relative() {
        let validator = SandboxValidator::new(PathBuf::from("/var/sandbox"));
        assert_eq!(
            validator.validate_path(Path::new("subdir/file.txt")),
            PathValidationResult::Allowed
        );
    }

    #[test]
    fn test_valid_subpath_absolute() {
        let validator = SandboxValidator::new(PathBuf::from("/var/sandbox"));
        assert_eq!(
            validator.validate_path(Path::new("/var/sandbox/subdir/file.txt")),
            PathValidationResult::Allowed
        );
    }

    #[test]
    fn test_denied_traversal() {
        let validator = SandboxValidator::new(PathBuf::from("/var/sandbox"));
        assert_eq!(
            validator.validate_path(Path::new("../external/file.txt")),
            PathValidationResult::DeniedTraversal
        );
        assert_eq!(
            validator.validate_path(Path::new("subdir/../../file.txt")),
            PathValidationResult::DeniedTraversal
        );
    }

    #[test]
    fn test_denied_outside_sandbox() {
        let validator = SandboxValidator::new(PathBuf::from("/var/sandbox"));
        assert_eq!(
            validator.validate_path(Path::new("/etc/passwd")),
            PathValidationResult::DeniedOutsideSandbox
        );
        assert_eq!(
            validator.validate_path(Path::new("/var/sandbox-extra/file.txt")),
            PathValidationResult::DeniedOutsideSandbox
        );
    }

    #[test]
    fn test_sanitize_filename() {
        let dirty = "file/with\\bad:chars*?<>\".txt\0";
        let clean = SandboxValidator::sanitize_filename(dirty);
        assert_eq!(clean, "filewithbadchars.txt");

        let clean_filename = "normal_file.txt";
        assert_eq!(
            SandboxValidator::sanitize_filename(clean_filename),
            clean_filename
        );
    }

    #[test]
    fn test_portable_mode_detector() {
        let temp = tempfile::tempdir().unwrap();
        let exe_dir = temp.path();

        assert_eq!(PortableModeDetector::detect_portable_dir(exe_dir), None);

        let data_dir = exe_dir.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        assert_eq!(
            PortableModeDetector::detect_portable_dir(exe_dir),
            Some(data_dir)
        );
    }

    #[test]
    fn test_safe_mode_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();

        assert!(!SafeModeRecovery::is_safe_mode_active(home));

        let c1 = SafeModeRecovery::record_crash(home);
        assert_eq!(c1, 1);
        assert!(!SafeModeRecovery::is_safe_mode_active(home));

        let c2 = SafeModeRecovery::record_crash(home);
        assert_eq!(c2, 2);
        assert!(SafeModeRecovery::is_safe_mode_active(home));

        SafeModeRecovery::record_clean_exit(home);
        assert!(!SafeModeRecovery::is_safe_mode_active(home));
    }
}
