//! test-intent: behavior
use crate::profiles_page_model::{ProfileFilterTarget, ProfilesOperation, ProfilesPageModel};
use lurker_core_contract::error::{ErrorCode, Failure};

#[derive(Clone, Debug)]
struct Profile {
    name: String,
    kind: String,
}

impl Profile {
    fn new(name: &str, kind: &str) -> Self {
        Self {
            name: name.to_owned(),
            kind: kind.to_owned(),
        }
    }
}

impl ProfileFilterTarget for Profile {
    fn profile_name(&self) -> &str {
        &self.name
    }

    fn profile_kind(&self) -> &str {
        &self.kind
    }
}

fn profiles() -> Vec<Profile> {
    vec![
        Profile::new("Home", "local"),
        Profile::new("Work", "subscription"),
        Profile::new("Backup", "local"),
    ]
}

#[test]
fn the_operation_list_is_one_shared_inventory() {
    assert_eq!(ProfilesOperation::ALL.len(), 10);
    for (index, operation) in ProfilesOperation::ALL.iter().enumerate() {
        assert_eq!(operation.index(), index);
    }
}

#[test]
fn every_operation_has_its_own_phase_and_one_operation_never_moves_another() {
    let mut model = ProfilesPageModel::default();
    assert!(!model.is_busy());

    let import = model.operation_mut(ProfilesOperation::Import).begin();
    let sync = model.operation_mut(ProfilesOperation::SyncWebDav).begin();
    assert!(model.is_busy());
    assert_eq!(
        model.running_operations(),
        vec![ProfilesOperation::Import, ProfilesOperation::SyncWebDav]
    );

    let failure = Failure::new(ErrorCode::Network, "the host refused the import", true);
    assert!(
        model
            .operation_mut(ProfilesOperation::Import)
            .fail(import, failure)
    );
    assert_eq!(
        model
            .operation_failure(ProfilesOperation::Import)
            .map(|failure| failure.message.as_str()),
        Some("the host refused the import")
    );
    assert!(model.operation(ProfilesOperation::SyncWebDav).is_running());
    assert!(
        model
            .operation_failure(ProfilesOperation::SyncWebDav)
            .is_none()
    );

    assert!(
        model
            .operation_mut(ProfilesOperation::SyncWebDav)
            .succeed(sync)
    );
    assert!(!model.is_busy());
}

#[test]
fn a_second_import_supersedes_the_first_one() {
    let mut model = ProfilesPageModel::default();
    let stale = model.operation_mut(ProfilesOperation::Import).begin();
    let current = model.operation_mut(ProfilesOperation::Import).begin();

    assert!(
        !model
            .operation_mut(ProfilesOperation::Import)
            .succeed(stale)
    );
    assert!(model.operation(ProfilesOperation::Import).is_running());
    assert_eq!(
        model.operation(ProfilesOperation::Import).token(),
        Some(current)
    );
}

#[test]
fn the_list_filter_hides_the_profiles_it_does_not_match() {
    let mut model = ProfilesPageModel::default();
    let profiles = profiles();

    assert_eq!(model.visible_profiles(&profiles).len(), 3);
    assert_eq!(model.hidden_profile_count(&profiles), 0);

    model.edit_filter("  ");
    assert_eq!(model.visible_profiles(&profiles).len(), 3);

    model.edit_filter("work");
    let visible = model.visible_profiles(&profiles);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].profile_name(), "Work");
    assert_eq!(model.hidden_profile_count(&profiles), 2);

    model.edit_filter("local");
    assert_eq!(model.visible_profiles(&profiles).len(), 2);

    model.edit_filter("nothing");
    assert!(model.visible_profiles(&profiles).is_empty());
    assert_eq!(model.hidden_profile_count(&profiles), 3);
}
