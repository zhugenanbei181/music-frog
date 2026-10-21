//! test-intent: behavior
//! Shared helpers that drive the Iced surface's own task pipeline.
//!
//! A surface command answers asynchronously: the update pipeline returns a
//! task, and the message that task produces is the fact the surface renders.
//! These helpers drive that pipeline to the end, so a behaviour test observes
//! the real answer instead of a local copy of it.
use crate::state::AppState;
use crate::types::message::Message;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::overview_layout_application::OverviewLayoutApplication;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::runtime::Builder;

use crate::test_mounts::command_harness::HostlessProcess;

/// The update pipeline a drained message is fed back into.
pub type SurfaceUpdate = fn(&mut AppState, Message) -> Task<Message>;

/// Drive a surface task to the end, feeding every message it produces back
/// into the given update pipeline.
pub fn drain_surface_task(state: &mut AppState, task: Task<Message>, update: SurfaceUpdate) {
    let mut pending = VecDeque::from([task]);
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            while let Some(task) = pending.pop_front() {
                let Some(mut stream) = into_stream(task) else {
                    continue;
                };
                while let Some(action) = stream.next().await {
                    if let Action::Output(message) = action {
                        pending.push_back(update(state, message));
                    }
                }
            }
        });
}

/// A product owner that holds the Overview card order.
///
/// The card order is the owner's fact: a surface test that moves a card
/// needs an owner that answers the shared move intent with the order it now
/// holds.
pub fn overview_layout_application() -> CoreApplication {
    let application = CoreApplication::new(
        Arc::new(HostlessProcess),
        Arc::new(HostlessProcess),
        infiltrator_composition::tokio_application_runtime().unwrap(),
    );
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_overview_layout(OverviewLayoutApplication::new()),
    ));
    application
}
