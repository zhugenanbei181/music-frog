//! The one typed draft shape every editable page form composes.
//!
//! A page that edits an owner value needs to answer two questions: what does
//! the user see, and does the draft differ from what the owner confirmed?
//! The old shape answered the second question with a boolean next to two
//! private copies of the owner value, so a surface could say "modified" about
//! a value it had no baseline for.
//!
//! This module owns that answer in one typed value: the draft and its
//! committed baseline live together, and "modified" is a derived fact of the
//! two, never a flag.

/// A draft the user edits next to the committed owner value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditableDraft<T> {
    /// The draft equals the committed owner value.
    Committed(T),
    /// The draft differs from the committed owner value.
    Modified {
        /// The committed owner value the draft started from.
        committed: T,
        /// The edited draft.
        draft: T,
    },
}

impl<T: Default> Default for EditableDraft<T> {
    fn default() -> Self {
        Self::Committed(T::default())
    }
}

impl<T> EditableDraft<T> {
    /// The draft that equals the committed owner value.
    pub fn committed(value: T) -> Self {
        Self::Committed(value)
    }

    /// The draft the user currently sees.
    pub fn value(&self) -> &T {
        match self {
            Self::Committed(value) => value,
            Self::Modified { draft, .. } => draft,
        }
    }

    /// The committed baseline, when the draft has one.
    pub fn baseline(&self) -> Option<&T> {
        match self {
            Self::Committed(value) => Some(value),
            Self::Modified { committed, .. } => Some(committed),
        }
    }

    /// Whether the draft differs from the committed owner value.
    pub fn is_modified(&self) -> bool {
        matches!(self, Self::Modified { .. })
    }
}

impl<T: Clone + PartialEq> EditableDraft<T> {
    /// Replace the draft the user sees.
    ///
    /// A draft that equals the committed baseline is not a modification, so
    /// editing a value back to its original state clears the state instead of
    /// leaving a stale "modified" behind.
    pub fn edit(&mut self, draft: T) {
        match self {
            Self::Committed(committed) => {
                if *committed != draft {
                    let committed = T::clone(committed);
                    *self = Self::Modified { committed, draft };
                }
            }
            Self::Modified {
                committed,
                draft: current,
            } => {
                if *committed == draft {
                    let committed = T::clone(committed);
                    *self = Self::Committed(committed);
                } else {
                    *current = draft;
                }
            }
        }
    }

    /// Change the draft in place.
    ///
    /// The committed baseline is never touched: the change is applied to a
    /// copy of the visible draft, so the baseline stays the owner value the
    /// draft started from.
    pub fn edit_with(&mut self, apply: impl FnOnce(&mut T)) {
        let mut draft = T::clone(self.value());
        apply(&mut draft);
        self.edit(draft);
    }

    /// Adopt a new committed owner value.
    ///
    /// A draft the user has not edited becomes the owner value. A pending
    /// edit stays visible and the owner value becomes its new baseline; an
    /// edit the owner already accepted becomes the committed value again.
    pub fn adopt(&mut self, committed: T) {
        match self {
            Self::Committed(_) => *self = Self::Committed(committed),
            Self::Modified {
                committed: baseline,
                draft,
            } => {
                *baseline = committed;
                if *draft == *baseline {
                    let value = T::clone(draft);
                    *self = Self::Committed(value);
                }
            }
        }
    }

    /// Accept the draft as the new committed value.
    ///
    /// Returns the value the owner must confirm.
    pub fn commit(&mut self) -> T {
        let value = T::clone(self.value());
        *self = Self::Committed(T::clone(&value));
        value
    }

    /// Drop the draft and go back to the committed owner value.
    pub fn discard(&mut self) {
        if let Self::Modified { committed, .. } = self {
            let committed = T::clone(committed);
            *self = Self::Committed(committed);
        }
    }
}

#[cfg(test)]
#[path = "editable_draft_tests.rs"]
mod tests;
