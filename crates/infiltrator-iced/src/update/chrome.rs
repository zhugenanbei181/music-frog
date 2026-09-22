//! Frameless-window chrome handlers (DUAL-15-13), split out of `update/ui.rs`
//! so the UI dispatcher keeps its line budget.
//!
//! Every request is a real host window op on the resolved window id: drag and
//! double-click-maximize come from the shared chrome strip, and the window id
//! is unresolved until `Message::WindowIdResolved` arrives, so an early press
//! is a no-op instead of a fabricated window handle.

use crate::state::AppState;
use crate::types::message::Message;
use crate::window_chrome::ChromeRequest;
use iced::Task;

impl AppState {
    /// Handlers for the window-chrome messages. Returns `None` when the
    /// message is not part of this domain.
    pub(crate) fn update_chrome(&mut self, message: Message) -> Option<Task<Message>> {
        let request = ChromeRequest::from_message(&message)?;
        let task = match request {
            // The window runs with `exit_on_close_request(false)`, so the
            // chrome close button reuses the same orderly exit path as the
            // tray quit entry (exit cleanup runs, then the process ends).
            ChromeRequest::Close => Task::done(Message::Exit),
            _ => match self.shell.window_id {
                Some(id) => request.task(id),
                None => Task::none(),
            },
        };
        Some(task)
    }
}
