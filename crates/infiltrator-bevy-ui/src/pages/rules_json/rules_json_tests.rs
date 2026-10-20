//! The rules JSON partition's behaviour cases.
//!
//! test-intent: behavior

use super::*;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentRevision, ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileId,
    ProfileSourceId,
};
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};

fn identity(revision: &str) -> ProfileDocumentIdentity {
    ProfileDocumentIdentity::new(
        ProfileId::new("profile-main").unwrap(),
        EngineId::new("mihomo").unwrap(),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("local-main").unwrap(),
        ),
        ProfileDocumentRevision::new(revision).unwrap(),
        ProfileFormatId::new("mihomo-yaml").unwrap(),
        ProfileSchemaVersion::new(1).unwrap(),
    )
}

fn documents() -> Vec<RulesJsonDocumentSnapshot> {
    vec![
        RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            identity: identity("revision-1"),
            json: "{\n  \"a\": 1\n}".to_owned(),
        },
        RulesJsonDocumentSnapshot {
            section: RulesJsonSection::Sniffer,
            identity: identity("revision-1"),
            json: "{}".to_owned(),
        },
    ]
}

#[test]
fn every_shared_section_has_a_label_and_a_save_label() {
    for section in RulesJsonSection::ALL {
        assert!(!Lang("zh-CN").tr(section.i18n_key()).is_empty());
        assert!(!Lang("zh-CN").tr(section.save_i18n_key()).is_empty());
    }
    assert_eq!(
        Lang("zh-CN").tr(RulesJsonSection::Sniffer.i18n_key()),
        "嗅探器 JSON"
    );
    assert_eq!(
        Lang("zh-CN").tr(RulesJsonSection::RuleProviders.save_i18n_key()),
        "保存规则提供者"
    );
}

#[test]
fn adoption_fills_empty_buffers_and_never_clobbers_an_unaccepted_edit() {
    let mut state = RulesJsonState::default();
    let mut model = RulesPageModel::default();
    state.adopt(&documents(), &mut model);
    assert_eq!(state.buffers.len(), RulesJsonSection::ALL.len());
    assert_eq!(
        state.buffers[RulesJsonSection::RuleProviders.index()].full_text(),
        "{\n  \"a\": 1\n}"
    );
    assert_eq!(
        state.buffers[RulesJsonSection::ProxyProviders.index()].full_text(),
        ""
    );

    // An unaccepted edit keeps the user's bytes when the read model moves.
    state.section = RulesJsonSection::RuleProviders;
    state.buffer_mut().set_text("{\n  \"edited\": true\n}");
    model
        .json_buffer_mut(RulesJsonSection::RuleProviders)
        .edit("{\n  \"edited\": true\n}".to_owned());
    state.adopt(
        &[RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            identity: identity("revision-2"),
            json: "{\n  \"server\": 1\n}".to_owned(),
        }],
        &mut model,
    );
    assert_eq!(
        state.buffers[RulesJsonSection::RuleProviders.index()].full_text(),
        "{\n  \"edited\": true\n}"
    );
    assert_eq!(
        state.buffer_identities[RulesJsonSection::RuleProviders.index()],
        Some(identity("revision-1")),
        "an unaccepted edit keeps the revision it was read from"
    );
    assert!(model.json_modified(RulesJsonSection::RuleProviders));
    assert!(
        state
            .status_label(
                "zh-CN",
                model.json_modified(RulesJsonSection::RuleProviders)
            )
            .contains("有未提交改动")
    );

    // A read-back that matches the buffer clears the pending edit.
    state.adopt(
        &[RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            identity: identity("revision-2"),
            json: "{\n  \"edited\": true\n}".to_owned(),
        }],
        &mut model,
    );
    assert!(!model.json_modified(RulesJsonSection::RuleProviders));
    assert_eq!(
        state.buffer_identities[RulesJsonSection::RuleProviders.index()],
        Some(identity("revision-2"))
    );
    assert!(
        !state
            .status_label(
                "zh-CN",
                model.json_modified(RulesJsonSection::RuleProviders)
            )
            .contains("有未提交改动")
    );

    // A clean buffer follows the read model.
    state.adopt(
        &[RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            identity: identity("revision-3"),
            json: "{\n  \"server\": 2\n}".to_owned(),
        }],
        &mut model,
    );
    assert_eq!(
        state.buffers[RulesJsonSection::RuleProviders.index()].full_text(),
        "{\n  \"server\": 2\n}"
    );
    assert!(
        state
            .status_label(
                "zh-CN",
                model.json_modified(RulesJsonSection::RuleProviders)
            )
            .contains("已与读模型一致")
    );
}

#[test]
fn editing_marks_the_shared_draft_modified_and_escape_releases_focus() {
    let mut state = RulesJsonState::default();
    let mut model = RulesPageModel::default();
    state.adopt(&documents(), &mut model);
    state.section = RulesJsonSection::RuleProviders;
    state.focused = true;
    assert!(state.apply_key(&Key::End, &mut model));
    assert!(state.apply_key(&Key::Character("!".into()), &mut model));
    assert!(state.buffer().full_text().contains("!"));
    assert!(model.json_modified(RulesJsonSection::RuleProviders));
    assert!(
        state
            .status_label(
                "zh-CN",
                model.json_modified(RulesJsonSection::RuleProviders)
            )
            .contains("编辑中")
    );
    assert!(state.apply_key(&Key::Escape, &mut model));
    assert!(!state.focused);
    assert!(!state.apply_key(&Key::Character("x".into()), &mut model));
}

#[test]
fn submit_builds_a_revision_bound_typed_update_and_refuses_invalid_buffers() {
    let mut state = RulesJsonState::default();
    let mut model = RulesPageModel::default();
    state.adopt(&documents(), &mut model);
    state.section = RulesJsonSection::Sniffer;
    match state.submit_command() {
        Ok(UiCommand::ApplyProfileConfiguration(request)) => {
            assert_eq!(request.identity, identity("revision-1"));
            assert_eq!(
                request.update,
                ProfileConfigurationUpdate::Sniffer(Default::default())
            );
        }
        other => panic!("expected a typed profile update, got {other:?}"),
    }

    state.section = RulesJsonSection::ProxyProviders;
    assert_eq!(
        state.submit_command(),
        Err(RulesJsonSubmissionFailure::Empty),
        "empty buffer is refused"
    );
    assert!(
        state
            .status_label(
                "zh-CN",
                model.json_modified(RulesJsonSection::ProxyProviders)
            )
            .contains("读模型未发布该文档")
    );

    state.section = RulesJsonSection::RuleProviders;
    state.buffer_mut().set_text("[]");
    assert_eq!(
        state.submit_command(),
        Err(RulesJsonSubmissionFailure::InvalidDocument)
    );
}
