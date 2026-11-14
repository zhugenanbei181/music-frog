//! Typed intents from the proxies page's per-group controls.
//!
//! The page owns three controls that belong to one family: a group's own
//! filter, and the control that brings that group's selected node into view.
//! They are one typed enum instead of three `Message` variants so the message
//! bus stays within its line budget and the intent family has one owner.

/// One intent from the proxies page's per-group controls.
#[derive(Clone, Debug)]
pub enum ProxyPageIntent {
    /// Open or close the filter one proxy group owns.
    ToggleGroupFilter(String),
    /// Replace the query of the filter one proxy group owns.
    FilterGroup {
        /// The group whose own filter changed.
        group: String,
        /// The raw text the group's filter box now holds.
        text: String,
    },
    /// Bring the node one group selected into view.
    ///
    /// The intent names the group only: the node it selected is the owner's
    /// fact, so the surface resolves it when the command is handled instead of
    /// carrying a second copy of it.
    LocateGroupNode(String),
}
