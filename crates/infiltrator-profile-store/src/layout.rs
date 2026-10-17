//! Product-pinned on-disk layout.
//!
//! Every name here is a historical compatibility promise: raised by the product
//! owner, asserted by the test below, and independent of the neutral kernel's
//! defaults. Renaming any field is a migration, not a refactor.

use lurker_profile_store::layout::ProfileStoreLayout;
use std::sync::OnceLock;

static PRODUCT_LAYOUT: OnceLock<ProfileStoreLayout> = OnceLock::new();

/// The single product layout used by every `EngineProfileStore` in this build.
pub fn product_layout() -> &'static ProfileStoreLayout {
    PRODUCT_LAYOUT.get_or_init(|| ProfileStoreLayout {
        engine_dir_name: "engines".to_owned(),
        profile_state_file_name: "profile-state.toml".to_owned(),
        profile_state_backup_extension: "toml.namespace-backup".to_owned(),
        options_dir_name: "options".to_owned(),
        options_extension: "yaml".to_owned(),
        document_backup_suffix: ".bak".to_owned(),
        profile_write_lock_prefix: ".profile-write-".to_owned(),
        profile_write_lock_suffix: ".lock".to_owned(),
        active_profile_reference_file_name: "active-profile-reference.json".to_owned(),
        active_profile_reference_lock_file_name: "active-profile-reference.lock".to_owned(),
        active_core_owner_state_file_name: "active-core-owner.json".to_owned(),
        active_core_owner_lock_file_name: "active-core-owner.lock".to_owned(),
        active_core_owner_state_lock_file_name: "active-core-owner-state.lock".to_owned(),
        reset_intent_file_name: ".active-profile-reset-intent.json".to_owned(),
        reset_complete_file_name: ".active-profile-reset-complete.json".to_owned(),
        reset_default_backup_file_name: ".active-profile-reset-default.previous".to_owned(),
        reset_default_absent_file_name: ".active-profile-reset-default.was-absent".to_owned(),
        aggregation_templates_file_name: ".aggregation-templates.yaml".to_owned(),
        named_override_catalog_file_name: ".named-overrides.yaml".to_owned(),
    })
}

/// Clone of the pinned layout for owners that must hold it by value.
pub fn product_layout_owned() -> ProfileStoreLayout {
    product_layout().clone()
}

#[cfg(test)]
mod tests {
    use super::product_layout;
    use std::path::Path;

    #[test]
    fn historical_on_disk_literals_are_pinned() {
        let layout = product_layout();
        assert_eq!(layout.engine_dir_name, "engines");
        assert_eq!(layout.profile_state_file_name, "profile-state.toml");
        assert_eq!(
            layout.profile_state_backup_extension,
            "toml.namespace-backup"
        );
        assert_eq!(layout.options_dir_name, "options");
        assert_eq!(layout.options_extension, "yaml");
        assert_eq!(layout.document_backup_suffix, ".bak");
        assert_eq!(layout.profile_write_lock_prefix, ".profile-write-");
        assert_eq!(layout.profile_write_lock_suffix, ".lock");
        assert_eq!(
            layout.active_profile_reference_file_name,
            "active-profile-reference.json"
        );
        assert_eq!(
            layout.active_profile_reference_lock_file_name,
            "active-profile-reference.lock"
        );
        assert_eq!(
            layout.active_core_owner_state_file_name,
            "active-core-owner.json"
        );
        assert_eq!(
            layout.active_core_owner_lock_file_name,
            "active-core-owner.lock"
        );
        assert_eq!(
            layout.active_core_owner_state_lock_file_name,
            "active-core-owner-state.lock"
        );
        assert_eq!(
            layout.reset_intent_file_name,
            ".active-profile-reset-intent.json"
        );
        assert_eq!(
            layout.reset_complete_file_name,
            ".active-profile-reset-complete.json"
        );
        assert_eq!(
            layout.reset_default_backup_file_name,
            ".active-profile-reset-default.previous"
        );
        assert_eq!(
            layout.reset_default_absent_file_name,
            ".active-profile-reset-default.was-absent"
        );
        assert_eq!(
            layout.aggregation_templates_file_name,
            ".aggregation-templates.yaml"
        );
        assert_eq!(
            layout.named_override_catalog_file_name,
            ".named-overrides.yaml"
        );
    }

    #[test]
    fn historical_paths_are_unchanged() {
        let layout = product_layout();
        assert_eq!(
            layout.engine_config_dir(Path::new("/config"), "mihomo"),
            Path::new("/config/engines/mihomo")
        );
        assert_eq!(
            layout.profile_state_file(Path::new("/config/engines/mihomo")),
            Path::new("/config/engines/mihomo/profile-state.toml")
        );
        assert_eq!(
            layout.options_path(Path::new("/config/engines/mihomo"), "default"),
            Path::new("/config/engines/mihomo/options/default.yaml")
        );
        assert_eq!(
            layout.profile_write_lock_name("mihomo"),
            ".profile-write-mihomo.lock"
        );
        assert_eq!(
            layout.document_backup_path(Path::new("/config/engines/mihomo/default.yaml")),
            Path::new("/config/engines/mihomo/default.yaml.bak")
        );
    }
}
