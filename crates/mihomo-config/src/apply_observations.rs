//! Actual transaction receipts belong to this host instance and the named profile.
use crate::manager::ConfigManager;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_ports::secure_store::SecureStore;
impl<S: SecureStore> ConfigManager<S> {
    pub fn record_apply_transaction(&self, record: ApplyTransactionSnapshot) {
        self.profile_store.record_apply_transaction(record);
    }
    pub fn apply_transaction(&self, profile: &str) -> Option<ApplyTransactionSnapshot> {
        self.profile_store.apply_transaction(profile)
    }
}
