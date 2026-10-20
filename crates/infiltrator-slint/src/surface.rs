//! Typed driver for the real Slint surface.
//!
//! The callbacks under [`crate::callbacks`] are the only place where a Slint
//! gesture becomes a typed owner command. Tests and parity scenarios must drive
//! those callbacks instead of re-implementing their mapping, so this module
//! exposes two things:
//!
//! - [`action`]: the typed owner commands the callbacks produce. The callbacks
//!   call them, so there is exactly one implementation per behaviour.
//! - [`harness`]: a headless Slint platform plus a harness that registers the
//!   real callbacks on a real window, drives them, and reports what the surface
//!   dispatched or refused.
//!
//! The surface owns no business rule of its own: every command it emits is the
//! shared `CommandIntent` vocabulary, and every refusal is the shared typed
//! `Failure`.

pub mod action;
pub mod harness;
pub mod workbenches;

use lurker_core_contract::error::Failure;
use std::cell::RefCell;
use std::rc::Rc;

/// Observer that receives the typed failures the surface raises.
pub(crate) type FailureObserver = Rc<dyn Fn(Failure)>;

thread_local! {
    /// Observer for the typed failures raised by the surface on this thread.
    ///
    /// The product never installs one; the headless harness installs it so a
    /// driven gesture can report the exact typed failure the surface produced
    /// instead of only the projected notice text.
    static FAILURE_OBSERVER: RefCell<Option<FailureObserver>> = const { RefCell::new(None) };
}

/// Reports a surface failure to the observer installed on this thread.
pub(crate) fn report_failure(failure: &Failure) {
    let observer = FAILURE_OBSERVER.with(|slot| slot.borrow().clone());
    if let Some(observer) = observer {
        observer(failure.clone());
    }
}

/// Installs the failure observer for this thread and returns the previous one.
pub(crate) fn install_failure_observer(observer: FailureObserver) -> Option<FailureObserver> {
    FAILURE_OBSERVER.with(|slot| slot.borrow_mut().replace(observer))
}
