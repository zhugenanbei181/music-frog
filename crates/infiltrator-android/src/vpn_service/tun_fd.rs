//! Ownership helpers for the TUN descriptor borrowed from Kotlin.

use std::fmt;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::io::AsRawFd;
use std::thread::{Builder, JoinHandle};

#[derive(Debug)]
pub(super) struct OwnedTunFd(OwnedFd);

impl OwnedTunFd {
    fn duplicate(raw_fd: RawFd) -> Result<Self, TunFdDuplicateError> {
        if raw_fd < 0 {
            return Err(TunFdDuplicateError::InvalidDescriptor);
        }

        // SAFETY: fcntl accepts any integer descriptor and reports invalid
        // descriptors as an OS error. A successful F_DUPFD_CLOEXEC returns a
        // new descriptor whose ownership is transferred into OwnedFd below.
        let duplicate = unsafe { libc::fcntl(raw_fd, libc::F_DUPFD_CLOEXEC, 0) };
        if duplicate < 0 {
            return Err(TunFdDuplicateError::Duplicate(io::Error::last_os_error()));
        }

        // SAFETY: `duplicate` is a fresh descriptor returned by fcntl above.
        let owned = unsafe { OwnedFd::from_raw_fd(duplicate) };
        Ok(Self(owned))
    }

    pub(super) fn raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[derive(Debug)]
pub(super) enum TunFdDuplicateError {
    InvalidDescriptor,
    Duplicate(io::Error),
}

impl fmt::Display for TunFdDuplicateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDescriptor => formatter.write_str("invalid Android TUN descriptor"),
            Self::Duplicate(error) => {
                write!(
                    formatter,
                    "failed to duplicate Android TUN descriptor: {error}"
                )
            }
        }
    }
}

#[derive(Debug)]
pub(super) enum TunFdPreparationError<Failure> {
    Duplicate(TunFdDuplicateError),
    Prepare(Failure),
}

impl<Failure: fmt::Display> fmt::Display for TunFdPreparationError<Failure> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(error) => fmt::Display::fmt(error, formatter),
            Self::Prepare(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

pub(super) fn prepare_with_tun_fd<T, Failure>(
    borrowed_fd: RawFd,
    prepare: impl FnOnce(RawFd) -> Result<T, Failure>,
) -> Result<(OwnedTunFd, T), TunFdPreparationError<Failure>> {
    let owner = OwnedTunFd::duplicate(borrowed_fd).map_err(TunFdPreparationError::Duplicate)?;
    let prepared = prepare(owner.raw_fd()).map_err(TunFdPreparationError::Prepare)?;
    Ok((owner, prepared))
}

pub(super) fn spawn_with_tun_fd(
    builder: Builder,
    owner: OwnedTunFd,
    worker: impl FnOnce() + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    spawn_owned_tun_fd(owner, worker, move |task| builder.spawn(task))
}

fn spawn_owned_tun_fd(
    owner: OwnedTunFd,
    worker: impl FnOnce() + Send + 'static,
    spawn: impl FnOnce(Box<dyn FnOnce() + Send + 'static>) -> io::Result<JoinHandle<()>>,
) -> io::Result<JoinHandle<()>> {
    let task = Box::new(move || {
        worker();
        drop(owner);
    });
    spawn(task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::fd::AsRawFd;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn is_open(fd: RawFd) -> bool {
        // SAFETY: fcntl checks the descriptor value and does not take ownership.
        unsafe { libc::fcntl(fd, libc::F_GETFD) >= 0 }
    }

    #[test]
    fn preparation_failure_closes_only_the_rust_duplicate() {
        let original = File::open("/dev/null").expect("open test descriptor");
        let original_fd = original.as_raw_fd();
        let mut duplicate_fd = None;

        let result = prepare_with_tun_fd(original_fd, |fd| {
            duplicate_fd = Some(fd);
            Err::<(), _>("tun2proxy builder rejected configuration")
        });

        assert!(matches!(
            result,
            Err(TunFdPreparationError::Prepare(
                "tun2proxy builder rejected configuration"
            ))
        ));
        let duplicate_fd = duplicate_fd.expect("preparation receives Rust-owned duplicate");
        assert!(is_open(original_fd));
        assert!(!is_open(duplicate_fd));
    }

    #[test]
    fn duplicate_failure_is_a_typed_outcome_and_keeps_caller_owner() {
        let original = File::open("/dev/null").expect("open test descriptor");
        let original_fd = original.as_raw_fd();

        let invalid = prepare_with_tun_fd(-1, |_| Ok::<_, String>(()));
        assert!(matches!(
            invalid,
            Err(TunFdPreparationError::Duplicate(
                TunFdDuplicateError::InvalidDescriptor
            ))
        ));

        let result = prepare_with_tun_fd(i32::MAX, |_| Ok::<_, String>(()));

        assert!(matches!(
            result,
            Err(TunFdPreparationError::Duplicate(
                TunFdDuplicateError::Duplicate(_)
            ))
        ));
        assert!(is_open(original_fd));
    }

    #[test]
    fn worker_start_failure_drops_duplicate_without_closing_original() {
        let original = File::open("/dev/null").expect("open test descriptor");
        let original_fd = original.as_raw_fd();
        let (owner, ()) = prepare_with_tun_fd(original_fd, |_| Ok::<_, String>(()))
            .expect("duplicate descriptor for worker");
        let duplicate_fd = owner.raw_fd();
        let result = spawn_owned_tun_fd(
            owner,
            || {},
            |_task| Err(io::Error::other("injected worker spawn failure")),
        );

        assert!(result.is_err());
        assert!(is_open(original_fd));
        assert!(!is_open(duplicate_fd));
    }

    #[test]
    fn worker_holds_duplicate_until_return_then_closes_it() {
        let original = File::open("/dev/null").expect("open test descriptor");
        let original_fd = original.as_raw_fd();
        let (owner, ()) = prepare_with_tun_fd(original_fd, |_| Ok::<_, String>(()))
            .expect("duplicate descriptor for worker");
        let duplicate_fd = owner.raw_fd();
        let worker_saw_open_fd = Arc::new(AtomicBool::new(false));
        let worker_observation = Arc::clone(&worker_saw_open_fd);
        let handle = spawn_with_tun_fd(Builder::new(), owner, move || {
            worker_observation.store(is_open(duplicate_fd), Ordering::Relaxed);
        })
        .expect("start descriptor-owning worker");

        handle.join().expect("worker exits cleanly");

        assert!(worker_saw_open_fd.load(Ordering::Relaxed));
        assert!(is_open(original_fd));
        assert!(!is_open(duplicate_fd));
    }
}
