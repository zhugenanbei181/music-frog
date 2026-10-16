use super::*;

#[test]
fn download_transitions_admit_only_the_current_run_and_cancellation() {
    let mut state = CoreDownloadState::default();
    let run_id = match state.start() {
        CoreDownloadStart::Started { run_id, .. } => run_id,
        CoreDownloadStart::AlreadyDownloading => panic!("idle download must start"),
    };
    assert!(matches!(
        state.start(),
        CoreDownloadStart::AlreadyDownloading
    ));
    assert_eq!(
        state.admit_progress(
            CoreDownloadRunId::default(),
            CoreDownloadProgress {
                downloaded: 900,
                total: Some(1000),
                speed_bytes: 50,
            },
        ),
        CoreDownloadProgressAdmission::StaleRun
    );
    assert_eq!(
        state.admit_progress(
            run_id,
            CoreDownloadProgress {
                downloaded: 700,
                total: Some(1000),
                speed_bytes: 50,
            },
        ),
        CoreDownloadProgressAdmission::Applied
    );
    assert_eq!(state.progress_fraction(), 0.7);
    assert!(matches!(
        state.request_cancel(),
        CoreDownloadCancelAdmission::Requested(_)
    ));
    assert!(matches!(
        state.request_cancel(),
        CoreDownloadCancelAdmission::AlreadyRequested(_)
    ));
    assert_eq!(
        state.complete(run_id),
        CoreDownloadCompletionAdmission::Applied
    );
    assert_eq!(
        state.complete(run_id),
        CoreDownloadCompletionAdmission::StaleRun
    );
    assert_eq!(state.progress(), None);
}

#[test]
fn invalidated_download_and_update_replies_cannot_reopen_old_phases() {
    let mut download = CoreDownloadState::default();
    let run_id = match download.start() {
        CoreDownloadStart::Started { run_id, .. } => run_id,
        CoreDownloadStart::AlreadyDownloading => panic!("idle download must start"),
    };
    assert_eq!(
        download.invalidate(),
        CoreDownloadInvalidation::Cancelled(run_id)
    );
    assert_eq!(
        download.complete(run_id),
        CoreDownloadCompletionAdmission::StaleRun
    );

    let mut update = CoreUpdateCheckState::default();
    let check_id = match update.start() {
        CoreUpdateCheckStart::Started(check_id) => check_id,
        CoreUpdateCheckStart::AlreadyChecking => panic!("idle update check must start"),
    };
    assert!(matches!(
        update.start(),
        CoreUpdateCheckStart::AlreadyChecking
    ));
    assert_eq!(
        update.invalidate(),
        CoreUpdateCheckInvalidation::Invalidated(check_id)
    );
    assert_eq!(
        update.complete(check_id, Ok("stale-version".to_owned())),
        CoreUpdateCheckCompletionAdmission::StaleCheck
    );
    let retry_id = match update.start() {
        CoreUpdateCheckStart::Started(check_id) => check_id,
        CoreUpdateCheckStart::AlreadyChecking => panic!("invalidated check must restart"),
    };
    let failure = InfiltratorError::Internal("offline".to_owned());
    assert_eq!(
        update.complete(retry_id, Err(failure.clone())),
        CoreUpdateCheckCompletionAdmission::Applied
    );
    assert!(matches!(
        update.failure(),
        Some(InfiltratorError::Internal(_))
    ));
    assert_eq!(update.latest_version(), None);
    assert!(matches!(update.start(), CoreUpdateCheckStart::Started(_)));
}
