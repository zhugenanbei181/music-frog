//! Source-bound document commands and snippet validity.
//! test-intent: behavior
use super::*;
use crate::failure_projection::failure_message;
use crate::profile_document_application::insert_snippet;
use infiltrator_contract::error::FailureReason;

#[tokio::test]
async fn load_and_save_profile_document_round_trips_through_the_shared_guard() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let application = CommandApplication::new().with_profile(profiles.clone());

    application
        .execute(CommandIntent::LoadProfileDocument { profile: None })
        .await
        .expect("load");
    let document = profiles
        .editor_observations()
        .0
        .expect("the shared document is published for the surfaces");
    assert_eq!(document.profile, "main");
    assert_eq!(document.content, THREE_RULES);
    assert!(
        document.is_clean(),
        "a valid stored document has no diagnostic"
    );
    assert!(!document.write_protection.is_protected());
    assert_eq!(document.line_count, THREE_RULES.lines().count());

    // A typed syntax error is rejected by the shared preflight and never
    // reaches the apply transaction.
    let error = application
        .execute(CommandIntent::SaveProfileDocument {
            source: document.source.clone().unwrap(),
            content: "rules: [\n".to_owned(),
            allow_protected: false,
        })
        .await
        .expect_err("invalid yaml is rejected");
    assert!(
        matches!(error.reason, Some(FailureReason::YamlSyntax { .. })),
        "expected a locale-neutral syntax reason: {error:?}"
    );
    assert!(failure_message(&error, "en-US").starts_with("YAML syntax error"));
    assert!(failure_message(&error, "zh-CN").starts_with("YAML 语法错误"));
    assert_eq!(store.content(), THREE_RULES, "the store is untouched");

    // A valid buffer commits against the observed workspace and publishes the actual receipt.
    let saved = "rules:\n  - MATCH,DIRECT\n";
    application
        .execute(CommandIntent::SaveProfileDocument {
            source: document.source.clone().unwrap(),
            content: saved.to_owned(),
            allow_protected: false,
        })
        .await
        .expect("save");
    assert_eq!(store.content(), saved);
    assert_eq!(
        profiles.editor_observations().0.expect("published").content,
        saved
    );
}

// ---- DUAL-09-04: shared snippet insertion ----------------------------------

#[test]
fn insert_snippet_keeps_the_document_parseable_and_refuses_a_broken_splice() {
    let document = "proxies:\n  - name: keep\n    type: ss\nrules:\n  - MATCH,DIRECT\n";

    // A node item appended to the proxies list keeps the document valid, and
    // every untouched byte is preserved.
    let insertion = insert_snippet(document, "ss", 3, 12).expect("a node item may join the list");
    assert!(
        insertion.is_clean(),
        "the splice keeps the document parseable"
    );
    assert!(
        insertion
            .content
            .starts_with("proxies:\n  - name: keep\n    type: ss"),
        "the original bytes stay in place: {}",
        insertion.content
    );
    assert!(
        insertion.content.contains("- name: SS-Node"),
        "the catalogue body is spliced verbatim"
    );

    // Splitting a scalar to make room for a block sequence is not valid YAML:
    // the shared preflight refuses it instead of writing a broken profile.
    let refusal = insert_snippet("mode: rule\n", "select", 1, 6)
        .expect_err("a sequence cannot continue a scalar");
    assert!(
        refusal.message.contains("片段") && refusal.message.contains("无法解析"),
        "the typed failure carries the shared diagnostic: {}",
        refusal.message
    );

    // An unknown id is a programming error, never a silently dropped insert.
    let unknown =
        insert_snippet(document, "nope", 1, 0).expect_err("unknown snippet ids are rejected");
    assert!(unknown.message.contains("nope"));
}
