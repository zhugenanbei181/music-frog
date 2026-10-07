//! Real runtime identities constrain a complete group-order transaction.
use super::CommandApplication;
use crate::proxy_group_order_editor::validate_order;
use infiltrator_contract::error::{ErrorCode, Failure};
use std::collections::BTreeSet;
impl CommandApplication {
    pub(super) async fn apply_group_order(&self, group_names: Vec<String>) -> Result<(), Failure> {
        validate_order(&group_names)?;
        if let Some(runtime) = &self.runtime {
            let proxies = runtime.get_proxies().await.map_err(Failure::from)?;
            let actual: BTreeSet<_> = proxies
                .iter()
                .filter(|(_, proxy)| proxy.is_group())
                .map(|(name, _)| name.as_str())
                .collect();
            let requested: BTreeSet<_> = group_names.iter().map(String::as_str).collect();
            if actual != requested {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "group identities changed; refresh before applying the order",
                    true,
                ));
            }
        }
        self.proxy_preferences()?.reorder_groups(group_names)
    }
}
