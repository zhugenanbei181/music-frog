//! Behavior cases for rules workspace.
//! test-intent: behavior

use super::*;
use infiltrator_contract::command::{CommandIntent, CommandKind};
use infiltrator_contract::profile_configuration::{
    ProfileConfigurationRequest, ProfileConfigurationUpdate, SnifferConfiguration,
};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileId, ProfileSourceId,
};
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};

#[test]
fn test_rules_workspace_partitions_delegate_to_shared_vocabulary() {
    use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
    use infiltrator_shared::locales::{Lang, Localizer};

    let (mut state, _) = AppState::new();
    // The default partition is the shared default, not a surface literal.
    assert_eq!(state.editor.rules_tab, RulesTab::default());
    assert_eq!(state.editor.rules_json_tab, RulesJsonSection::default());
    assert_eq!(RulesTab::default(), RulesTab::List);
    assert_eq!(RulesJsonSection::default(), RulesJsonSection::RuleProviders);

    // Every shared partition is selectable through the shared enum and keeps
    // its shared identity.
    for tab in RulesTab::ALL {
        let _ = state.update(Message::SetRulesTab(tab));
        assert_eq!(state.editor.rules_tab, tab);
        assert_eq!(RulesTab::from_index(tab.index()), tab);
    }
    for section in RulesJsonSection::ALL {
        let _ = state.update(Message::SetRulesJsonTab(section));
        assert_eq!(state.editor.rules_json_tab, section);
        assert_eq!(RulesJsonSection::from_index(section.index()), section);
    }

    // Every shared label key resolves in both language tables: a partition
    // cannot exist with a label the Iced surface cannot render.
    for lang in [Lang("zh-CN"), Lang("en")] {
        for tab in RulesTab::ALL {
            assert_ne!(lang.tr(tab.i18n_key()).as_ref(), tab.i18n_key(), "{tab:?}");
        }
        for section in RulesJsonSection::ALL {
            assert_ne!(
                lang.tr(section.i18n_key()).as_ref(),
                section.i18n_key(),
                "{section:?}"
            );
            assert_ne!(
                lang.tr(section.save_i18n_key()).as_ref(),
                section.save_i18n_key(),
                "{section:?}"
            );
        }
    }

    // DUAL-11-14: the Geo database entry point both surfaces expose is the
    // shared command vocabulary (Iced drives the same gateway call the
    // application performs for the Bevy intent).
    assert_eq!(
        CommandIntent::UpgradeGeoDatabases.kind(),
        CommandKind::Runtime
    );
    let identity = ProfileDocumentIdentity::new(
        ProfileId::new("main").unwrap(),
        EngineId::new("mihomo").unwrap(),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("local-main").unwrap(),
        ),
        ProfileDocumentRevision::new("revision-1").unwrap(),
        ProfileFormatId::new("mihomo-yaml").unwrap(),
        ProfileSchemaVersion::new(1).unwrap(),
    );
    assert_eq!(
        CommandIntent::ApplyProfileConfiguration {
            request: Box::new(ProfileConfigurationRequest {
                identity,
                update: ProfileConfigurationUpdate::Sniffer(SnifferConfiguration::default()),
            }),
        }
        .kind(),
        CommandKind::Profile
    );
}
