//! Behavior checks for the native `-t` dry-run boundary.
//! test-intent: behavior

use super::MihomoNativeConfigCheck;
use crate::process::command::{MihomoCommandPort, NativeConfigCheck, NativeConfigCheckOutcome};
use async_trait::async_trait;
use lurker_core_contract::error::ErrorCode;
use lurker_core_ports::error::PortError;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::fs::read;

struct RecordingCommand {
    check: NativeConfigCheck,
    detail: &'static str,
    observed: Mutex<Vec<(PathBuf, Vec<u8>, PathBuf)>>,
}

impl RecordingCommand {
    fn new(check: NativeConfigCheck, detail: &'static str) -> Self {
        Self {
            check,
            detail,
            observed: Mutex::new(Vec::new()),
        }
    }

    fn observed(&self) -> Vec<(PathBuf, Vec<u8>, PathBuf)> {
        self.observed.lock().expect("observed commands").clone()
    }
}

#[async_trait]
impl MihomoCommandPort for RecordingCommand {
    async fn check_config(
        &self,
        binary_path: &Path,
        config_path: &Path,
        config_dir: &Path,
    ) -> Result<NativeConfigCheckOutcome, PortError> {
        let staged = read(config_path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        self.observed.lock().expect("observed commands").push((
            binary_path.to_path_buf(),
            staged,
            config_dir.to_path_buf(),
        ));
        Ok(NativeConfigCheckOutcome {
            check: self.check,
            detail: self.detail.to_owned(),
        })
    }
}

fn native_files(config_dir: &Path) -> Vec<PathBuf> {
    read_dir(config_dir)
        .expect("read config dir")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".mihomo-profile-validation-"))
        })
        .collect()
}

#[tokio::test]
async fn accepted_native_check_stages_the_exact_candidate_beside_the_profile() {
    let config_dir = tempfile::tempdir().expect("isolated config dir");
    let command = Arc::new(RecordingCommand::new(NativeConfigCheck::Accepted, ""));
    let binary_path = config_dir.path().join("mihomo");
    let check = MihomoNativeConfigCheck::new(
        binary_path.clone(),
        config_dir.path().to_path_buf(),
        command.clone(),
    );

    let document = b"mode: rule\nrules:\n  - MATCH,DIRECT\n";
    check.validate(document).await.expect("accepted candidate");

    let observed = command.observed();
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].0, binary_path);
    assert_eq!(observed[0].1, document);
    assert_eq!(observed[0].2, config_dir.path());
    assert!(
        native_files(config_dir.path()).is_empty(),
        "the staged candidate must be removed"
    );
}

#[tokio::test]
async fn rejected_native_check_preserves_the_binary_diagnostic() {
    let config_dir = tempfile::tempdir().expect("isolated config dir");
    let command = Arc::new(RecordingCommand::new(
        NativeConfigCheck::Rejected,
        "parse error at line 2",
    ));
    let check = MihomoNativeConfigCheck::new(
        config_dir.path().join("mihomo"),
        config_dir.path().to_path_buf(),
        command,
    );

    match check.validate(b"mode: : rule\n").await {
        Err(PortError::Rejected(failure)) => {
            assert_eq!(failure.code, ErrorCode::Configuration);
            assert!(failure.message.contains("parse error at line 2"));
        }
        other => panic!("expected a typed configuration rejection, got {other:?}"),
    }
    assert!(native_files(config_dir.path()).is_empty());
}
