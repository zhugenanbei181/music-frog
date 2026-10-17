//! Product-side bridge to the neutral text-catalog port.
//!
//! Every projection resolves canonical copy through
//! [`lurker_ui_presentation::copy::TextCatalog`] and interpolates placeholders
//! with the neutral single-pass algorithm. The product catalog itself (the
//! `Lang` adapter and all copy data) stays in `infiltrator-shared`; this crate
//! owns no copy and selects no language.

use lurker_ui_presentation::copy::{TextCatalog, interpolate_with};

/// Resolve `key` through `catalog` and interpolate `values` once.
pub(crate) fn localize(catalog: &dyn TextCatalog, key: &str, values: &[(&str, String)]) -> String {
    let template = catalog.text(key);
    interpolate_with(template.as_ref(), |name| {
        values
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, value)| value.as_str())
    })
}
