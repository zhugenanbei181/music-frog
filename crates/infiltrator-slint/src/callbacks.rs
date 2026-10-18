//! Callback registration split by product workflow.

mod navigation;
mod outbound;
mod product;
mod profiles;
mod runtime;
mod settings;

use crate::{AppWindow, SurfaceSnapshot, WorkerRequest};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub(super) fn register_callbacks(
    window: &AppWindow,
    requests: Sender<WorkerRequest>,
    latest: Arc<Mutex<Option<SurfaceSnapshot>>>,
) {
    navigation::register(window, Arc::clone(&latest));
    profiles::register(window, requests.clone(), Arc::clone(&latest));
    runtime::register(window, requests.clone(), Arc::clone(&latest));
    outbound::register(window, requests.clone(), Arc::clone(&latest));
    settings::register(window, requests.clone(), Arc::clone(&latest));
    product::register(window, requests, latest);
}
