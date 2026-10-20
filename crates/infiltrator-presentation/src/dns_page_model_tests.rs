//! test-intent: behavior
use crate::dns_page_model::{DnsEditMode, DnsOperation, DnsPageModel, DnsPageTab};
use lurker_core_contract::error::{ErrorCode, Failure};

#[test]
fn the_partition_selection_is_one_shared_value() {
    let mut model = DnsPageModel::default();
    assert_eq!(model.tab(), DnsPageTab::Dns);
    assert_eq!(model.tab_index(), 0);

    model.select_tab(DnsPageTab::FakeIp);
    assert_eq!(model.tab(), DnsPageTab::FakeIp);
    assert_eq!(model.tab_index(), 1);

    model.select_tab_at(2);
    assert_eq!(model.tab(), DnsPageTab::Tun);

    model.select_tab_at(99);
    assert_eq!(model.tab(), DnsPageTab::Dns);
}

#[test]
fn the_edit_mode_is_one_shared_value() {
    let mut model = DnsPageModel::default();
    assert_eq!(model.edit_mode(), DnsEditMode::Form);

    model.select_edit_mode(DnsEditMode::Json);
    assert_eq!(model.edit_mode(), DnsEditMode::Json);
}

#[test]
fn every_configuration_write_has_its_own_phase() {
    let mut model = DnsPageModel::default();
    assert!(!model.is_busy());

    let save = model.operation_mut(DnsOperation::SaveDns).begin();
    assert!(model.is_busy());
    assert_eq!(model.running_operations(), vec![DnsOperation::SaveDns]);

    // The Fake-IP write has its own phase and is untouched.
    assert!(!model.operation(DnsOperation::SaveFakeIp).is_running());
    let fake_ip = model.operation_mut(DnsOperation::SaveFakeIp).begin();
    assert_eq!(
        model.running_operations(),
        vec![DnsOperation::SaveDns, DnsOperation::SaveFakeIp]
    );

    let failure = Failure::new(
        ErrorCode::Configuration,
        "the resolver list is invalid",
        false,
    );
    assert!(
        model
            .operation_mut(DnsOperation::SaveDns)
            .fail(save, failure)
    );
    assert!(model.operation(DnsOperation::SaveFakeIp).is_running());

    assert!(
        model
            .operation_mut(DnsOperation::SaveFakeIp)
            .succeed(fake_ip)
    );
    assert!(!model.is_busy());
}

#[test]
fn a_late_answer_moves_only_its_own_phase() {
    let mut model = DnsPageModel::default();
    let stale = model.operation_mut(DnsOperation::SaveTun).begin();
    let current = model.operation_mut(DnsOperation::SaveTun).begin();

    assert!(!model.operation_mut(DnsOperation::SaveTun).succeed(stale));
    assert!(model.operation(DnsOperation::SaveTun).is_running());
    assert_eq!(
        model.operation(DnsOperation::SaveTun).token(),
        Some(current)
    );
}
