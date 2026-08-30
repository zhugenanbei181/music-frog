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
    ///
    /// Removes characters `\`, `/`, `:`, `*`, `?`, `"`, `<`, `>`, `|`, and null bytes `\0`.
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
        // Direct relative traversal
        assert_eq!(
            validator.validate_path(Path::new("../external/file.txt")),
            PathValidationResult::DeniedTraversal
        );
        // Traversal hidden inside
        assert_eq!(
            validator.validate_path(Path::new("subdir/../../file.txt")),
            PathValidationResult::DeniedTraversal
        );
    }

    #[test]
    fn test_denied_outside_sandbox() {
        let validator = SandboxValidator::new(PathBuf::from("/var/sandbox"));
        // Completely different absolute path
        assert_eq!(
            validator.validate_path(Path::new("/etc/passwd")),
            PathValidationResult::DeniedOutsideSandbox
        );
        // Path that shares a prefix but isn't a sub-directory
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
}
