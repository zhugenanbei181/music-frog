//! One localization component for the active profile's published origin.
//!
//! FlClash's `profiles.dart` maps its typed `ProfileType` to `remote`, `local`
//! or `custom`. MusicFrog already has a richer engine-neutral
//! `ProfileDocumentSourceKind`; surfaces consume its stable copy key instead
//! of deriving a source from a URL or a write-protection flag.

use infiltrator_contract::shared_copy::SharedCopyKey;
use infiltrator_contract::surface_snapshot::ProfileSnapshot;
use lurker_core_contract::profile_document_identity::ProfileDocumentSourceKind;
use lurker_ui_presentation::copy::TextCatalog;

/// The profile origin as the shared owner observed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfileSourceObservation {
    Local,
    Subscription,
    Imported,
    Synchronized,
    Unobserved,
}

impl ProfileSourceObservation {
    pub fn from_profile(profile: Option<&ProfileSnapshot>) -> Self {
        let Some(profile) = profile else {
            return Self::Unobserved;
        };
        match &profile.filter_source {
            Ok(workspace) => match workspace.document.source.kind {
                ProfileDocumentSourceKind::Local => Self::Local,
                ProfileDocumentSourceKind::Subscription => Self::Subscription,
                ProfileDocumentSourceKind::Imported => Self::Imported,
                ProfileDocumentSourceKind::Synchronized => Self::Synchronized,
            },
            Err(_) => Self::Unobserved,
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::Local => SharedCopyKey::ProfileSourceLocal.key(),
            Self::Subscription => SharedCopyKey::ProfileSourceSubscription.key(),
            Self::Imported => SharedCopyKey::ProfileSourceImported.key(),
            Self::Synchronized => SharedCopyKey::ProfileSourceSynchronized.key(),
            Self::Unobserved => SharedCopyKey::ProfileSourceUnobserved.key(),
        }
    }

    pub fn copy(self, catalog: &dyn TextCatalog) -> String {
        catalog.text(self.key()).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use infiltrator_contract::surface_snapshot::{
        ProfileSnapshot, ProfilesPageSnapshot, SurfaceSnapshot,
    };
    use infiltrator_shared::locales::Lang;
    use lurker_core_contract::error::Failure;
    use lurker_core_contract::identity::EngineId;
    use lurker_core_contract::page::PageData;
    use lurker_core_contract::profile_document_identity::{
        ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
        ProfileFormatId, ProfileId, ProfileSchemaVersion, ProfileSidecarRevision, ProfileSourceId,
        ProfileWorkspaceIdentity,
    };

    fn snapshot() -> SurfaceSnapshot {
        let mut snapshot = SurfaceSnapshot::unavailable(
            SurfaceKind::SlintDesktop,
            HostKind::Desktop,
            Failure::unsupported("test"),
        );
        let profile = ProfileSnapshot {
            id: "profile-1".to_owned(),
            name: "Alpha".to_owned(),
            url: "https://example.test/sub".to_owned(),
            updated_at: String::new(),
            upload_bytes: None,
            download_bytes: None,
            total_bytes: None,
            is_active: true,
            user_agent: String::new(),
            insecure_skip_verify: false,
            etag: None,
            last_modified: None,
            has_backup: false,
            cron_expression: None,
            auto_update_enabled: false,
            update_interval_hours: None,
            next_update: None,
            auto_reload_core: true,
            filter: Default::default(),
            filter_source: Ok(workspace_identity()),
            write_protection: Default::default(),
        };
        snapshot.pages.profiles = PageData::ready(ProfilesPageSnapshot {
            profiles: vec![profile],
            auto_update_interval_hours: 24,
            updating: false,
            aggregation: None,
            aggregation_templates: Vec::new(),
            aggregation_templates_available: false,
            snapshot_history: None,
            apply_transaction: None,
            named_overrides: Default::default(),
        });
        snapshot
    }

    fn workspace_identity() -> ProfileWorkspaceIdentity {
        ProfileWorkspaceIdentity::new(
            ProfileDocumentIdentity::new(
                ProfileId::new("profile-1").expect("valid profile ID"),
                EngineId::new("mihomo").expect("valid engine ID"),
                ProfileDocumentSourceIdentity::new(
                    ProfileDocumentSourceKind::Subscription,
                    ProfileSourceId::new("source-alpha").expect("valid source ID"),
                ),
                ProfileDocumentRevision::new("revision-1").expect("valid revision"),
                ProfileFormatId::new("mihomo-yaml").expect("valid format ID"),
                ProfileSchemaVersion::new(1).expect("valid schema version"),
            ),
            ProfileSidecarRevision::Absent,
        )
    }

    #[test]
    fn the_workspace_identity_is_the_only_source_truth() {
        let snapshot = snapshot();
        let active = snapshot
            .pages
            .profiles
            .data
            .as_ref()
            .and_then(|page| page.profiles.iter().find(|profile| profile.is_active));
        let source = ProfileSourceObservation::from_profile(active);
        assert_eq!(source, ProfileSourceObservation::Subscription);
        assert_eq!(source.copy(&Lang("zh-CN")), "远程");
        assert_eq!(source.copy(&Lang("en-US")), "Remote");
    }

    #[test]
    fn a_missing_workspace_is_an_explicit_unobserved_source() {
        let source = ProfileSourceObservation::from_profile(None);
        assert_eq!(source, ProfileSourceObservation::Unobserved);
        assert_eq!(source.copy(&Lang("zh-CN")), "来源未知");
    }
}
