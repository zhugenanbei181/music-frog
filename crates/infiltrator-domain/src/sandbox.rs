//! Pure path-sandbox validation used by file-oriented application flows.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PathValidationResult {
    Allowed,
    DeniedTraversal,
    DeniedOutsideSandbox,
    InvalidPath,
}

pub struct SandboxValidator {
    allowed_root: PathBuf,
}

impl SandboxValidator {
    pub fn new(allowed_root: PathBuf) -> Self {
        Self { allowed_root }
    }

    pub fn validate_path(&self, candidate: &Path) -> PathValidationResult {
        if candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return PathValidationResult::DeniedTraversal;
        }

        let mut resolved = PathBuf::new();
        if candidate.is_absolute() {
            resolved.push(candidate);
        } else {
            resolved.push(&self.allowed_root);
            resolved.push(candidate);
        }

        let normalized = normalize_path(&resolved);
        let normalized_root = normalize_path(&self.allowed_root);
        if !normalized.starts_with(&normalized_root) {
            return PathValidationResult::DeniedOutsideSandbox;
        }
        PathValidationResult::Allowed
    }

    pub fn sanitize_filename(filename: &str) -> String {
        filename
            .chars()
            .filter(|character| {
                !matches!(
                    character,
                    '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'
                )
            })
            .collect()
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_only_paths_inside_the_root() {
        let validator = SandboxValidator::new(PathBuf::from("/var/sandbox"));
        assert_eq!(
            validator.validate_path(Path::new("subdir/file.txt")),
            PathValidationResult::Allowed
        );
        assert_eq!(
            validator.validate_path(Path::new("../external/file.txt")),
            PathValidationResult::DeniedTraversal
        );
        assert_eq!(
            validator.validate_path(Path::new("/etc/passwd")),
            PathValidationResult::DeniedOutsideSandbox
        );
    }

    #[test]
    fn sanitizes_filename_separators() {
        assert_eq!(
            SandboxValidator::sanitize_filename("file/with\\bad:chars*?.txt\0"),
            "filewithbadchars.txt"
        );
    }
}
