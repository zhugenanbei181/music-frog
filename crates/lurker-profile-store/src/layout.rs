//! Product-independent on-disk layout for one durable profile store.
//!
//! The kernel never embeds a product path literal. Every historical name is
//! supplied by [`ProfileStoreLayout`]; products pin their own values (and
//! assert them) so an upgrade can never silently relocate existing files.

use std::path::{Path, PathBuf};

/// Directory and file names owned by one durable profile store.
///
/// Field names describe the *role* of a path, never a product decision: a
/// product may rename any of them without touching the kernel, while the
/// defaults reproduce the historical layout of the first product adopter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileStoreLayout {
    /// Directory holding one sub-directory per engine namespace.
    pub engine_dir_name: String,
    /// Engine-scoped profile-state document (metadata, catalog order, schema).
    pub profile_state_file_name: String,
    /// Extension applied to the profile-state document's backup copy.
    pub profile_state_backup_extension: String,
    /// Engine-scoped directory holding opaque per-profile sidecars.
    pub options_dir_name: String,
    /// Extension of the per-profile sidecar document.
    pub options_extension: String,
    /// Suffix of the transient pre-save document copy.
    pub document_backup_suffix: String,
    /// Prefix of the cross-process engine write lock, before the engine name.
    pub profile_write_lock_prefix: String,
    /// Suffix of the cross-process engine write lock.
    pub profile_write_lock_suffix: String,
    /// Product-wide active-profile reference record.
    pub active_profile_reference_file_name: String,
    /// Root serialization lock for reference and owner transitions.
    pub active_profile_reference_lock_file_name: String,
    /// Durable active-Core owner proof record.
    pub active_core_owner_state_file_name: String,
    /// Admission lock held by the live Core owner process.
    pub active_core_owner_lock_file_name: String,
    /// Owner-state lock taken inside the root lock.
    pub active_core_owner_state_lock_file_name: String,
    /// Pending reset intent record.
    pub reset_intent_file_name: String,
    /// Durable reset completion marker.
    pub reset_complete_file_name: String,
    /// Saved previous `[default]` state during a reset.
    pub reset_default_backup_file_name: String,
    /// Marker recording that `[default]` was absent before a reset.
    pub reset_default_absent_file_name: String,
    /// Aggregation-template sidecar inside the options directory.
    pub aggregation_templates_file_name: String,
    /// Named-override catalog sidecar inside the options directory.
    pub named_override_catalog_file_name: String,
}

impl Default for ProfileStoreLayout {
    fn default() -> Self {
        Self {
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
        }
    }
}

impl ProfileStoreLayout {
    /// Engine-scoped namespace directory under the product config root.
    pub fn engine_config_dir(&self, config_root: &Path, engine_id: &str) -> PathBuf {
        config_root.join(&self.engine_dir_name).join(engine_id)
    }

    /// Profile-state document for one engine namespace.
    pub fn profile_state_file(&self, config_dir: &Path) -> PathBuf {
        config_dir.join(&self.profile_state_file_name)
    }

    /// Native profile document for one profile inside an engine namespace.
    ///
    /// The extension belongs to the injected document codec, so it is a caller
    /// argument rather than a layout field.
    pub fn profile_document_path(
        &self,
        config_dir: &Path,
        profile: &str,
        extension: &str,
    ) -> PathBuf {
        config_dir.join(format!("{profile}.{extension}"))
    }

    /// Durable backup of the profile-state document.
    pub fn profile_state_backup_path(&self, profile_state_file: &Path) -> PathBuf {
        profile_state_file.with_extension(&self.profile_state_backup_extension)
    }

    /// Directory holding opaque per-profile sidecars.
    pub fn options_dir(&self, config_dir: &Path) -> PathBuf {
        config_dir.join(&self.options_dir_name)
    }

    /// Sidecar document for one profile.
    pub fn options_path(&self, config_dir: &Path, profile: &str) -> PathBuf {
        self.options_dir(config_dir)
            .join(format!("{profile}.{}", self.options_extension))
    }

    /// Transient pre-save copy of one document.
    pub fn document_backup_path(&self, document: &Path) -> PathBuf {
        let name = document
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile.document");
        document.with_file_name(format!("{name}{}", self.document_backup_suffix))
    }

    /// Cross-process write lock shared by every writer of one engine namespace.
    pub fn profile_write_lock_name(&self, engine_name: &str) -> String {
        format!(
            "{}{engine_name}{}",
            self.profile_write_lock_prefix, self.profile_write_lock_suffix
        )
    }

    /// Aggregation-template sidecar path for one engine namespace.
    pub fn aggregation_templates_path(&self, config_dir: &Path) -> PathBuf {
        self.options_dir(config_dir)
            .join(&self.aggregation_templates_file_name)
    }

    /// Named-override catalog sidecar path for one engine namespace.
    pub fn named_override_catalog_path(&self, config_dir: &Path) -> PathBuf {
        self.options_dir(config_dir)
            .join(&self.named_override_catalog_file_name)
    }
}

#[cfg(test)]
mod tests {
    use super::ProfileStoreLayout;
    use std::path::Path;

    #[test]
    fn default_layout_paths_are_role_based_and_relative() {
        let layout = ProfileStoreLayout::default();
        let root = Path::new("/config");
        let config_dir = layout.engine_config_dir(root, "mihomo");
        assert_eq!(config_dir, Path::new("/config/engines/mihomo"));
        assert_eq!(
            layout.profile_state_file(&config_dir),
            Path::new("/config/engines/mihomo/profile-state.toml")
        );
        assert_eq!(
            layout.options_path(&config_dir, "primary"),
            Path::new("/config/engines/mihomo/options/primary.yaml")
        );
        assert_eq!(
            layout.document_backup_path(Path::new("/config/engines/mihomo/primary.yaml")),
            Path::new("/config/engines/mihomo/primary.yaml.bak")
        );
        assert_eq!(
            layout.profile_write_lock_name("mihomo"),
            ".profile-write-mihomo.lock"
        );
    }
}
