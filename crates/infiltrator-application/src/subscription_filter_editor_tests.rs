//! test-intent: behavior
use super::*;

use crate::subscription_filter_fixture::{self, FIXTURE_DOCUMENT};
fn observation(
    profile: &str,
    filter: &SubscriptionFilterDraft,
) -> Result<Option<FilterObservation>, Failure> {
    subscription_filter_fixture::observation(profile, FIXTURE_DOCUMENT, filter.clone()).map(Some)
}
fn applied(pending: &PendingFilterChange) -> SubscriptionFilterApplied {
    SubscriptionFilterApplied {
        source: subscription_filter_fixture::observation(
            &pending.source.source.profile,
            FIXTURE_DOCUMENT,
            pending.draft.clone(),
        )
        .unwrap()
        .source,
        report: FilterReport::default(),
    }
}
#[test]
fn all_fields_preserve_drafts_across_unchanged_reads_and_fence_every_changed_field() {
    for field in FilterField::ALL {
        let baseline = SubscriptionFilterDraft::default();
        let mut editor = SubscriptionFilterEditor::new(observation("alpha", &baseline).unwrap());
        assert!(editor.edit(field, "draft {profile}".into()));
        editor.observe(observation("alpha", &baseline));
        assert_eq!(field.value(&editor.draft), "draft {profile}");
        let mut changed = baseline;
        field.set(
            &mut changed,
            match field {
                FilterField::Renames => "remote=>next",
                FilterField::Advanced => "{drop-private-ip: true}",
                _ => "remote",
            }
            .into(),
        );
        editor.observe(observation("alpha", &changed));
        assert!(editor.stale());
        assert!(!editor.edit(field, "lost draft".into()));
        assert_eq!(field.value(&editor.draft), "draft {profile}");
        assert!(editor.begin().is_err());
        assert!(editor.cancel());
        assert_eq!(editor.draft, changed);
        assert!(editor.current());
        assert_eq!(editor.pending, None);
    }
}
#[test]
fn source_switch_read_failure_and_pending_receipts_never_migrate_an_unsent_form() {
    let baseline = SubscriptionFilterDraft::default();
    let mut editor = SubscriptionFilterEditor::new(observation("alpha", &baseline).unwrap());
    editor.edit(FilterField::Include, "user draft".into());
    let pending = editor.begin().unwrap();
    assert!(!editor.cancel());
    assert!(!editor.edit(FilterField::Include, "during write".into()));
    assert!(!editor.finish(pending.token + 1, Ok(applied(&pending))));
    editor.observe(observation("beta", &baseline));
    assert!(editor.finish(pending.token, Ok(applied(&pending))));
    assert_eq!(editor.source_profile(), Some("alpha"));
    assert_eq!(editor.latest_profile(), Some("beta"));
    assert!(editor.stale());
    assert_eq!(editor.draft.include, "user draft");
    assert!(editor.cancel());
    assert_eq!(editor.source_profile(), Some("beta"));
    let failure = Failure::new(ErrorCode::Storage, "read denied", true);
    editor.observe(Err(failure.clone()));
    assert_eq!(editor.read_failure(), Some(&failure));
    assert!(!editor.can_edit());
    assert!(editor.begin().is_err());
}
#[test]
fn validation_failure_and_rejected_write_preserve_form_and_new_requests_have_unique_tokens() {
    let baseline = SubscriptionFilterDraft::default();
    let mut editor = SubscriptionFilterEditor::new(observation("alpha", &baseline).unwrap());
    editor.edit(FilterField::Renames, "broken rename".into());
    assert_eq!(editor.begin().unwrap_err().code, ErrorCode::InvalidInput);
    assert!(editor.pending.is_none());
    editor.edit(FilterField::Renames, "first=>second".into());
    editor.pick(SubscriptionFilterDedup::KeepLast);
    let first = editor.begin().unwrap();
    let failure = Failure::new(ErrorCode::Storage, "write denied", true);
    assert!(editor.finish(first.token, Err(failure.clone())));
    editor.observe(observation("alpha", &baseline));
    assert_eq!(editor.failure, Some(failure));
    assert_eq!(editor.draft, first.draft);
    let second = editor.begin().unwrap();
    assert!(second.token > first.token);
    assert!(!editor.finish(first.token, Ok(applied(&first))));
    assert_eq!(editor.pending, Some(second.clone()));
    assert!(editor.finish(second.token, Ok(applied(&second))));
    assert!(!editor.dirty());
    assert_eq!(editor.selected(), Some(SubscriptionFilterDedup::KeepLast));
    assert!(editor.applied);
}

#[test]
fn context_and_document_changes_cannot_rebind_a_dirty_form_and_foreign_receipts_are_rejected() {
    let baseline = SubscriptionFilterDraft::default();
    let mut editor = SubscriptionFilterEditor::new(observation("alpha", &baseline).unwrap());
    editor.edit(FilterField::Include, "typed".into());
    editor.bind_profile(Some("beta"));
    assert!(!editor.can_edit());
    assert!(editor.begin().is_err());
    assert_eq!(editor.draft.include, "typed");
    editor.observe_profile(
        "beta",
        subscription_filter_fixture::observation("beta", FIXTURE_DOCUMENT, baseline.clone()),
    );
    assert!(editor.stale());
    assert!(editor.cancel());
    assert!(editor.can_edit());
    editor.edit(FilterField::Exclude, "draft".into());
    let changed_document = subscription_filter_fixture::observation(
        "beta",
        "# newer document\nmode: rule\n",
        baseline,
    )
    .unwrap();
    editor.observe_profile("beta", Ok(changed_document.clone()));
    assert!(editor.stale());
    assert_eq!(editor.draft.exclude, "draft");
    assert!(editor.cancel());
    let pending = editor.begin().unwrap();
    assert_eq!(pending.source.source, changed_document.source);
    let mut wrong = applied(&pending);
    wrong.source.profile = "foreign".into();
    assert!(editor.finish(pending.token, Ok(wrong)));
    assert_eq!(
        editor.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    assert!(!editor.applied && editor.report.is_none());
}
