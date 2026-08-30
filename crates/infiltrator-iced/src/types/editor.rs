//! Editor lazy-load flag shared by the rules JSON editors and the
//! DNS / Fake-IP / TUN config editors.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorLazyState {
    #[default]
    Unloaded,
    Loaded,
}
