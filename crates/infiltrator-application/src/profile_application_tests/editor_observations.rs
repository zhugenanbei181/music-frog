//! test-intent: behavior
//! Real application facades over one store prove product isolation and coherent source replay.
use super::*;
use crate::profile_document_application::ProfileDocumentApplication;
use crate::profile_editor_observations::EditorFacet;
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_contract::profile_document::ProfileDocumentSnapshot;
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_domain::profile_source::identify_profile_source;

#[tokio::test]
async fn independent_products_cannot_borrow_documents_options_or_foreign_read_tickets_even_over_one_store()
 {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let first = ProfileApplication::new(store.clone());
    let second = ProfileApplication::new(store.clone());
    let document = ProfileDocumentApplication::new(first.clone())
        .load(Some(" main "))
        .await
        .unwrap();
    let options = ProfileOptionsApplication::new(first.clone())
        .load(Some("main"))
        .await
        .unwrap();
    assert_eq!(
        first.clone().editor_observations(),
        (Some(document.clone()), Some(options.clone()))
    );
    assert_eq!(second.editor_observations(), (None, None));
    let read = first.begin_editor_read("main", EditorFacet::Document);
    second.begin_editor_read("main", EditorFacet::Document);
    assert!(!second.observe_document(&read, document));
    assert_eq!(second.editor_observations(), (None, None));
    let own = ProfileDocumentApplication::new(second.clone())
        .load(None)
        .await
        .unwrap();
    assert_eq!(second.editor_observations(), (Some(own), None));
    assert!(first.editor_observations().1.is_some());
}

#[tokio::test]
async fn changed_workspace_source_retires_the_other_facet_and_old_read_cannot_replace_a_newer_source()
 {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let old = ProfileDocumentApplication::new(profiles.clone())
        .load(None)
        .await
        .unwrap();
    let old_read = profiles.begin_editor_read("main", EditorFacet::Document);
    store.save("main", "mode: global\n").await.unwrap();
    let options = ProfileOptionsApplication::new(profiles.clone())
        .load(None)
        .await
        .unwrap();
    assert_eq!(
        profiles.editor_observations(),
        (None, Some(options.clone()))
    );
    assert!(!profiles.observe_document(&old_read, old.clone()));
    assert_eq!(
        profiles.editor_observations(),
        (None, Some(options.clone()))
    );
    assert_eq!(
        profiles.editor_observations_for(old.source.as_ref().unwrap()),
        (None, None)
    );
    let current = ProfileDocumentApplication::new(profiles.clone())
        .load(None)
        .await
        .unwrap();
    assert_eq!(current.content, "mode: global\n");
    assert_eq!(current.source.as_ref(), Some(&options.source));
    assert_eq!(
        profiles.editor_observations(),
        (Some(current), Some(options))
    );
}

#[test]
fn profile_switch_same_facet_old_reads_and_corrupted_document_bytes_are_rejected_without_changing_new_facts()
 {
    let profiles = ProfileApplication::new(Arc::new(FakeStore::default()));
    let old = profiles.begin_editor_read("first", EditorFacet::Document);
    let current = profiles.begin_editor_read("second", EditorFacet::Options);
    let source = identify_profile_source("second".into(), "mode: rule\n", None);
    let options = ProfileOptionsSnapshot::new(source.clone(), "{}\n", Default::default());
    assert!(profiles.observe_options(&current, options.clone()));
    let mut old_doc =
        ProfileDocumentSnapshot::new("first", "mode: rule\n", ProfileWriteProtection::Editable);
    old_doc.source = Some(identify_profile_source(
        "first".into(),
        "mode: rule\n",
        None,
    ));
    assert!(!profiles.observe_document(&old, old_doc));
    let old_read = profiles.begin_editor_read("second", EditorFacet::Document);
    let new_read = profiles.begin_editor_read("second", EditorFacet::Document);
    let mut document =
        ProfileDocumentSnapshot::new("second", "mode: rule\n", ProfileWriteProtection::Editable);
    document.source = Some(source);
    assert!(!profiles.observe_document(&old_read, document.clone()));
    let mut corrupt = document.clone();
    corrupt.content = "mode: global\n".into();
    assert!(!profiles.observe_document(&new_read, corrupt));
    assert_eq!(
        profiles.editor_observations(),
        (None, Some(options.clone()))
    );
    assert!(profiles.observe_document(&new_read, document.clone()));
    assert_eq!(
        profiles.editor_observations(),
        (Some(document), Some(options))
    );
}

#[tokio::test]
async fn mixin_save_commits_a_coherent_workspace_preserves_filter_and_repeated_apply_does_not_duplicate_rules()
 {
    use infiltrator_domain::mixin::{MixinConfig, RuleMixin};
    use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions};
    use infiltrator_ports::runtime_gateway::ManagedRuntime;
    let content =
        "# retained header\nmode: rule\nrules:\n  - DOMAIN,old.example,DIRECT\n  - MATCH,DIRECT\n";
    let store = Arc::new(FakeStore::with_profile("main", content, true));
    let filter = FilterSpec {
        include_keywords: vec!["retained filter".into()],
        ..Default::default()
    };
    store.options.lock().unwrap().insert(
        "main".into(),
        ProfileOptions {
            mixin: MixinConfig {
                rules: Some(RuleMixin {
                    prepend: vec!["DOMAIN,old.example,DIRECT".into()],
                    ..Default::default()
                }),
                ..Default::default()
            },
            filter: Some(filter.clone()),
        },
    );
    let profiles = ProfileApplication::new(store.clone());
    let before = profiles.load_workspace("main").await.unwrap();
    let application = ProfileOptionsApplication::new(profiles.clone());
    let proposal = "mode: global\nrules:\n  prepend:\n    - DOMAIN,new.example,DIRECT\n";
    let result = application
        .save_mixin(None::<Arc<dyn ManagedRuntime>>, &before.source, proposal)
        .await
        .unwrap();
    let committed = profiles.load_workspace("main").await.unwrap();
    assert_eq!(result.options.source, committed.source);
    assert_ne!(result.options.source, before.source);
    assert_eq!(committed.options.filter, Some(filter.clone()));
    assert_eq!(committed.options.mixin.mode.as_deref(), Some("global"));
    assert!(committed.content.contains("# retained header"));
    assert!(!committed.content.contains("DOMAIN,old.example,DIRECT"));
    assert_eq!(
        committed
            .content
            .matches("DOMAIN,new.example,DIRECT")
            .count(),
        1
    );
    let repeated = application
        .save_mixin(None::<Arc<dyn ManagedRuntime>>, &committed.source, proposal)
        .await
        .unwrap();
    let after = profiles.load_workspace("main").await.unwrap();
    assert_eq!(after.content, committed.content);
    assert_eq!(after.options.filter, Some(filter));
    assert_eq!(repeated.options.source, after.source);
    assert_eq!(profiles.editor_observations().1, Some(repeated.options));
}

#[tokio::test]
async fn failed_mixin_workspace_read_preserves_both_files_and_the_prior_product_observation() {
    use infiltrator_ports::runtime_gateway::ManagedRuntime;
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let application = ProfileOptionsApplication::new(profiles.clone());
    let observed = application.load(None).await.unwrap();
    let before = profiles.load_workspace("main").await.unwrap();
    *store.options_read_error.lock().unwrap() =
        Some(PortError::PermissionDenied("denied sidecar".into()));
    let failure = application
        .save_mixin(
            None::<Arc<dyn ManagedRuntime>>,
            &before.source,
            "mode: global\n",
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::Permission);
    assert_eq!(store.load("main").await.unwrap(), before.content);
    assert_eq!(profiles.editor_observations().1, Some(observed));
    *store.options_read_error.lock().unwrap() = None;
    assert_eq!(profiles.load_workspace("main").await.unwrap(), before);
}

#[tokio::test]
async fn actual_profile_deletion_retires_observations_and_inflight_reads_without_affecting_another_product()
 {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let first = ProfileApplication::new(store.clone());
    let second = ProfileApplication::new(store.clone());
    let document = ProfileDocumentApplication::new(first.clone())
        .load(Some("main"))
        .await
        .unwrap();
    let old_read = first.begin_editor_read("main", EditorFacet::Document);
    ProfileDocumentApplication::new(second.clone())
        .load(Some("main"))
        .await
        .unwrap();
    first.delete_profile("main").await.unwrap();
    assert_eq!(first.editor_observations(), (None, None));
    assert!(!first.observe_document(&old_read, document.clone()));
    assert!(
        second.editor_observations().0.is_some(),
        "another product's historical observation is not a mutable shared cache"
    );
    let failure = ProfileDocumentApplication::new(second.clone())
        .load(Some("main"))
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::Storage);
    assert_eq!(
        store.load("main").await.unwrap_err(),
        PortError::NotFound("main".into())
    );
}
