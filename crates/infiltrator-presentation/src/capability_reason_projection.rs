//! One localized view of a typed capability reason.
//!
//! The capability snapshot publishes stable keys, not user-facing prose. This
//! projector owns the only mapping from those keys to the product catalog;
//! opaque owner-authored reasons remain visible verbatim.

use infiltrator_contract::capability::CapabilityReason;
use lurker_ui_presentation::copy::TextCatalog;

pub fn reason_copy(reason: &str, catalog: &dyn TextCatalog) -> String {
    let key = CapabilityReason::ALL
        .iter()
        .find(|candidate| candidate.key() == reason)
        .map(|candidate| candidate.key());
    match key {
        Some(key) => catalog.text(key).into_owned(),
        None => reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::reason_copy;
    use infiltrator_contract::capability::CapabilityReason;
    use infiltrator_shared::locales::Lang;

    #[test]
    fn a_registered_reason_key_resolves_through_both_product_catalogs() {
        let reason = CapabilityReason::NotRegistered.key();
        assert_eq!(reason_copy(reason, &Lang("zh-CN")), "当前宿主未登记此能力");
        assert_eq!(
            reason_copy(reason, &Lang("en-US")),
            "This capability is not registered for the current host"
        );
    }

    #[test]
    fn an_owner_authored_reason_is_never_rewritten() {
        assert_eq!(
            reason_copy("adapter refused the tunnel", &Lang("zh-CN")),
            "adapter refused the tunnel"
        );
    }
}
