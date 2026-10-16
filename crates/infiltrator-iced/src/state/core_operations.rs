//! Typed local phases for core update checks and downloads.

use crate::types::app::CoreDownloadProgress;
use infiltrator_contract::error::InfiltratorError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CoreDownloadRunId(u64);

impl CoreDownloadRunId {
    fn next(self) -> Self {
        Self(
            self.0
                .checked_add(1)
                .expect("core download run identity exhausted"),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CoreUpdateCheckId(u64);

impl CoreUpdateCheckId {
    fn next(self) -> Self {
        Self(
            self.0
                .checked_add(1)
                .expect("core update check identity exhausted"),
        )
    }
}

#[derive(Clone, Debug)]
pub enum CoreDownloadState {
    Idle {
        last_run_id: CoreDownloadRunId,
    },
    Downloading {
        run_id: CoreDownloadRunId,
        progress: CoreDownloadProgress,
        cancellation: Arc<AtomicBool>,
    },
}

impl Default for CoreDownloadState {
    fn default() -> Self {
        Self::Idle {
            last_run_id: CoreDownloadRunId::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum CoreDownloadStart {
    Started {
        run_id: CoreDownloadRunId,
        cancellation: Arc<AtomicBool>,
    },
    AlreadyDownloading,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreDownloadProgressAdmission {
    Applied,
    StaleRun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreDownloadCompletionAdmission {
    Applied,
    StaleRun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreDownloadCancelAdmission {
    Requested(CoreDownloadRunId),
    AlreadyRequested(CoreDownloadRunId),
    NotDownloading,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreDownloadInvalidation {
    Idle,
    Cancelled(CoreDownloadRunId),
}

impl CoreDownloadState {
    pub fn start(&mut self) -> CoreDownloadStart {
        match self {
            Self::Idle { last_run_id } => {
                let run_id = last_run_id.next();
                let cancellation = Arc::new(AtomicBool::new(false));
                *self = Self::Downloading {
                    run_id,
                    progress: CoreDownloadProgress {
                        downloaded: 0,
                        total: None,
                        speed_bytes: 0,
                    },
                    cancellation: Arc::clone(&cancellation),
                };
                CoreDownloadStart::Started {
                    run_id,
                    cancellation,
                }
            }
            Self::Downloading { .. } => CoreDownloadStart::AlreadyDownloading,
        }
    }

    pub fn admit_progress(
        &mut self,
        run_id: CoreDownloadRunId,
        progress: CoreDownloadProgress,
    ) -> CoreDownloadProgressAdmission {
        match self {
            Self::Downloading {
                run_id: current,
                progress: current_progress,
                ..
            } if *current == run_id => {
                *current_progress = progress;
                CoreDownloadProgressAdmission::Applied
            }
            Self::Idle { .. } | Self::Downloading { .. } => CoreDownloadProgressAdmission::StaleRun,
        }
    }

    pub fn complete(&mut self, run_id: CoreDownloadRunId) -> CoreDownloadCompletionAdmission {
        match self {
            Self::Downloading {
                run_id: current, ..
            } if *current == run_id => {
                *self = Self::Idle {
                    last_run_id: run_id,
                };
                CoreDownloadCompletionAdmission::Applied
            }
            Self::Idle { .. } | Self::Downloading { .. } => {
                CoreDownloadCompletionAdmission::StaleRun
            }
        }
    }

    pub fn request_cancel(&self) -> CoreDownloadCancelAdmission {
        match self {
            Self::Downloading {
                run_id,
                cancellation,
                ..
            } => match cancellation.swap(true, Ordering::AcqRel) {
                false => CoreDownloadCancelAdmission::Requested(*run_id),
                true => CoreDownloadCancelAdmission::AlreadyRequested(*run_id),
            },
            Self::Idle { .. } => CoreDownloadCancelAdmission::NotDownloading,
        }
    }

    pub fn invalidate(&mut self) -> CoreDownloadInvalidation {
        match self {
            Self::Idle { last_run_id } => {
                *self = Self::Idle {
                    last_run_id: last_run_id.next(),
                };
                CoreDownloadInvalidation::Idle
            }
            Self::Downloading {
                run_id,
                cancellation,
                ..
            } => {
                let invalidated = *run_id;
                cancellation.store(true, Ordering::Release);
                *self = Self::Idle {
                    last_run_id: invalidated.next(),
                };
                CoreDownloadInvalidation::Cancelled(invalidated)
            }
        }
    }

    pub fn run_id(&self) -> Option<CoreDownloadRunId> {
        match self {
            Self::Idle { .. } => None,
            Self::Downloading { run_id, .. } => Some(*run_id),
        }
    }

    pub fn progress(&self) -> Option<&CoreDownloadProgress> {
        match self {
            Self::Idle { .. } => None,
            Self::Downloading { progress, .. } => Some(progress),
        }
    }

    pub fn progress_fraction(&self) -> f32 {
        self.progress().map_or(0.0, |progress| {
            progress.total.map_or(0.0, |total| {
                if total == 0 {
                    0.0
                } else {
                    (progress.downloaded as f32 / total as f32).clamp(0.0, 1.0)
                }
            })
        })
    }

    pub fn is_downloading(&self) -> bool {
        matches!(self, Self::Downloading { .. })
    }
}

#[derive(Clone, Debug)]
pub enum CoreUpdateCheckState {
    Unchecked {
        last_check_id: CoreUpdateCheckId,
    },
    Checking {
        check_id: CoreUpdateCheckId,
    },
    Available {
        check_id: CoreUpdateCheckId,
        version: String,
    },
    Failed {
        check_id: CoreUpdateCheckId,
        failure: InfiltratorError,
    },
}

impl Default for CoreUpdateCheckState {
    fn default() -> Self {
        Self::Unchecked {
            last_check_id: CoreUpdateCheckId::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreUpdateCheckStart {
    Started(CoreUpdateCheckId),
    AlreadyChecking,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreUpdateCheckCompletionAdmission {
    Applied,
    StaleCheck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreUpdateCheckInvalidation {
    Unchecked,
    Invalidated(CoreUpdateCheckId),
}

impl CoreUpdateCheckState {
    pub fn start(&mut self) -> CoreUpdateCheckStart {
        match self {
            Self::Unchecked { last_check_id } => {
                let check_id = last_check_id.next();
                *self = Self::Checking { check_id };
                CoreUpdateCheckStart::Started(check_id)
            }
            Self::Checking { .. } => CoreUpdateCheckStart::AlreadyChecking,
            Self::Available { check_id, .. } | Self::Failed { check_id, .. } => {
                let next_id = check_id.next();
                *self = Self::Checking { check_id: next_id };
                CoreUpdateCheckStart::Started(next_id)
            }
        }
    }

    pub fn complete(
        &mut self,
        check_id: CoreUpdateCheckId,
        result: Result<String, InfiltratorError>,
    ) -> CoreUpdateCheckCompletionAdmission {
        match self {
            Self::Checking { check_id: current } if *current == check_id => {
                *self = match result {
                    Ok(version) => Self::Available { check_id, version },
                    Err(failure) => Self::Failed { check_id, failure },
                };
                CoreUpdateCheckCompletionAdmission::Applied
            }
            Self::Unchecked { .. }
            | Self::Checking { .. }
            | Self::Available { .. }
            | Self::Failed { .. } => CoreUpdateCheckCompletionAdmission::StaleCheck,
        }
    }

    pub fn invalidate(&mut self) -> CoreUpdateCheckInvalidation {
        match self {
            Self::Unchecked { last_check_id } => {
                *self = Self::Unchecked {
                    last_check_id: last_check_id.next(),
                };
                CoreUpdateCheckInvalidation::Unchecked
            }
            Self::Checking { check_id } => {
                let invalidated = *check_id;
                *self = Self::Unchecked {
                    last_check_id: invalidated.next(),
                };
                CoreUpdateCheckInvalidation::Invalidated(invalidated)
            }
            Self::Available { check_id, .. } | Self::Failed { check_id, .. } => {
                let invalidated = *check_id;
                *self = Self::Unchecked {
                    last_check_id: invalidated.next(),
                };
                CoreUpdateCheckInvalidation::Invalidated(invalidated)
            }
        }
    }

    pub fn is_checking(&self) -> bool {
        matches!(self, Self::Checking { .. })
    }

    pub fn latest_version(&self) -> Option<&str> {
        match self {
            Self::Available { version, .. } => Some(version),
            Self::Unchecked { .. } | Self::Checking { .. } | Self::Failed { .. } => None,
        }
    }

    pub fn failure(&self) -> Option<&InfiltratorError> {
        match self {
            Self::Failed { failure, .. } => Some(failure),
            Self::Unchecked { .. } | Self::Checking { .. } | Self::Available { .. } => None,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/gui/core_operations_tests.rs"]
mod tests;

