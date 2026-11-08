//! Typed fact cells: the one owner of "a labelled value on a card".
//!
//! A card that shows a label above a value has two failure modes, and neither
//! of them belongs to a page:
//!
//! 1. An unobserved value has to read as "nobody published this yet" instead
//!    of as a zero, an empty string, or a duration the surface invented.
//! 2. Two values in one row have to share a type role, or their baselines and
//!    their weights drift and the row reads as two unrelated fragments.
//!
//! Both decisions are owner facts, so they are typed here and projected. A
//! surface renders the cell it is given; it never guesses a fallback and never
//! picks a second type role for the row.

/// Whether an owner has published a fact's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FactValue {
    /// The owner published this value.
    Observed(String),
    /// Nobody has observed it yet.
    Unobserved,
}

/// The marker a surface paints for a fact nobody has observed.
///
/// It lives here so that no page, no projection and no surface keeps a second
/// copy of "the value we do not have".
pub const UNOBSERVED_FACT_MARKER: &str = "—";

impl FactValue {
    /// The text a surface paints for this value.
    pub fn text(&self) -> &str {
        match self {
            FactValue::Observed(text) => text.as_str(),
            FactValue::Unobserved => UNOBSERVED_FACT_MARKER,
        }
    }

    /// Whether an owner published this value.
    pub fn is_observed(&self) -> bool {
        matches!(self, FactValue::Observed(_))
    }
}

/// Which edge of its column a fact cell anchors to.
///
/// A row whose cells all hug the same edge reads as one table; a row that
/// mixes edges reads as ragged text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactAlignment {
    /// The cell hugs the start of its column.
    Start,
    /// The cell hugs the end of its column.
    End,
}

/// One fact value with the decisions a surface must not invent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactCell {
    /// The value, observed or explicitly unknown.
    pub value: FactValue,
    /// The edge of the column this cell anchors to.
    pub alignment: FactAlignment,
}

/// The hero card's fact row: the runtime length and the network organization.
///
/// The row has exactly two cells by design, so the fields are named instead of
/// being a list: a value can never slide onto the other column's label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardHeroFactRow {
    /// The runtime length, the leading column.
    pub uptime: FactCell,
    /// The network organization, the trailing column.
    pub organization: FactCell,
}

#[cfg(test)]
mod tests {
    use super::{FactAlignment, FactCell, FactValue, UNOBSERVED_FACT_MARKER};

    #[test]
    fn an_unobserved_fact_paints_the_shared_marker() {
        let value = FactValue::Unobserved;
        assert!(!value.is_observed());
        assert_eq!(value.text(), UNOBSERVED_FACT_MARKER);
    }

    #[test]
    fn an_observed_fact_paints_its_own_text() {
        let value = FactValue::Observed("01:23:45".to_string());
        assert!(value.is_observed());
        assert_eq!(value.text(), "01:23:45");
    }

    #[test]
    fn a_cell_carries_its_own_alignment() {
        let leading = FactCell {
            value: FactValue::Unobserved,
            alignment: FactAlignment::Start,
        };
        let trailing = FactCell {
            value: FactValue::Observed("Cloudflare".to_string()),
            alignment: FactAlignment::End,
        };
        assert_eq!(leading.alignment, FactAlignment::Start);
        assert_eq!(trailing.alignment, FactAlignment::End);
    }
}
