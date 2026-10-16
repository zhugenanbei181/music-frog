use super::*;

pub(super) fn readback_matches_operation(
    operation: &AndroidOwnerOperation,
    readback: &AndroidOwnerOperationReadback,
) -> bool {
    matches!(
        (operation, readback),
        (
            AndroidOwnerOperation::ProfilesList,
            AndroidOwnerOperationReadback::ProfilesList(_)
        ) | (
            AndroidOwnerOperation::ProfileDetail { .. },
            AndroidOwnerOperationReadback::ProfileDetail(_)
        ) | (
            AndroidOwnerOperation::ProfileSelect { .. },
            AndroidOwnerOperationReadback::ProfileSelection(_)
        ) | (
            AndroidOwnerOperation::ProfileCreate { .. }
                | AndroidOwnerOperation::ProfileUpdate { .. }
                | AndroidOwnerOperation::ProfileSave { .. }
                | AndroidOwnerOperation::ProfileDelete { .. }
                | AndroidOwnerOperation::ProfileSubscriptionSave { .. }
                | AndroidOwnerOperation::ProfileSubscriptionClear { .. },
            AndroidOwnerOperationReadback::ProfileMutation(_)
        ) | (
            AndroidOwnerOperation::CoreVersionManagement { .. },
            AndroidOwnerOperationReadback::CoreVersionManagement(_)
        ) | (
            AndroidOwnerOperation::DnsSettings | AndroidOwnerOperation::DnsSettingsSave { .. },
            AndroidOwnerOperationReadback::DnsSettings(_)
        ) | (
            AndroidOwnerOperation::TunSettings | AndroidOwnerOperation::TunSettingsSave { .. },
            AndroidOwnerOperationReadback::TunSettings(_)
        ) | (
            AndroidOwnerOperation::FakeIpSettings
                | AndroidOwnerOperation::FakeIpSettingsSave { .. },
            AndroidOwnerOperationReadback::FakeIpSettings(_)
        ) | (
            AndroidOwnerOperation::FakeIpCacheClear,
            AndroidOwnerOperationReadback::FakeIpCacheClear(_)
        ) | (
            AndroidOwnerOperation::RulesList | AndroidOwnerOperation::RulesSave { .. },
            AndroidOwnerOperationReadback::Rules(_)
        ) | (
            AndroidOwnerOperation::RuleProviders | AndroidOwnerOperation::RuleProvidersSave { .. },
            AndroidOwnerOperationReadback::RuleProviders(_)
        ) | (
            AndroidOwnerOperation::AppRoutingSnapshot
                | AndroidOwnerOperation::AppRoutingModeSet { .. }
                | AndroidOwnerOperation::AppRoutingPackagesSet { .. },
            AndroidOwnerOperationReadback::AppRouting(_)
        ) | (
            AndroidOwnerOperation::WebDavSettings
                | AndroidOwnerOperation::WebDavSettingsSave { .. },
            AndroidOwnerOperationReadback::WebDavSettings(_)
        ) | (
            AndroidOwnerOperation::WebDavTest { .. },
            AndroidOwnerOperationReadback::WebDavTest(_)
        ) | (
            AndroidOwnerOperation::WebDavSyncNow,
            AndroidOwnerOperationReadback::WebDavSyncNow(_)
        )
    )
}

pub(super) fn snapshot_matches_operation(
    operation: &AndroidOwnerOperation,
    snapshot: Option<&AndroidOwnerDataSnapshot>,
) -> bool {
    if matches!(operation, AndroidOwnerOperation::FakeIpCacheClear) {
        return snapshot.is_none();
    }
    if matches!(
        operation,
        AndroidOwnerOperation::CoreVersionManagement { .. }
    ) && snapshot.is_none()
    {
        return true;
    }
    matches!(
        (operation, snapshot),
        (
            AndroidOwnerOperation::ProfilesList
                | AndroidOwnerOperation::ProfileDetail { .. }
                | AndroidOwnerOperation::ProfileCreate { .. }
                | AndroidOwnerOperation::ProfileSelect { .. }
                | AndroidOwnerOperation::ProfileUpdate { .. }
                | AndroidOwnerOperation::ProfileSave { .. }
                | AndroidOwnerOperation::ProfileDelete { .. }
                | AndroidOwnerOperation::ProfileSubscriptionSave { .. }
                | AndroidOwnerOperation::ProfileSubscriptionClear { .. },
            Some(AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::Profiles,
                data: AndroidOwnerDataSnapshotData::Profiles(_),
            })
        ) | (
            AndroidOwnerOperation::CoreVersionManagement { .. },
            Some(AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::CoreVersions,
                data: AndroidOwnerDataSnapshotData::CoreVersions(_),
            })
        ) | (
            AndroidOwnerOperation::DnsSettings | AndroidOwnerOperation::DnsSettingsSave { .. },
            Some(AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::DnsSettings,
                data: AndroidOwnerDataSnapshotData::DnsSettings(_),
            })
        ) | (
            AndroidOwnerOperation::TunSettings | AndroidOwnerOperation::TunSettingsSave { .. },
            Some(AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::TunSettings,
                data: AndroidOwnerDataSnapshotData::TunSettings(_),
            })
        ) | (
            AndroidOwnerOperation::FakeIpSettings
                | AndroidOwnerOperation::FakeIpSettingsSave { .. },
            Some(AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::FakeIpSettings,
                data: AndroidOwnerDataSnapshotData::FakeIpSettings(_),
            })
        ) | (
            AndroidOwnerOperation::RulesList | AndroidOwnerOperation::RulesSave { .. },
            Some(AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::Rules,
                data: AndroidOwnerDataSnapshotData::Rules(_),
            })
        ) | (
            AndroidOwnerOperation::RuleProviders | AndroidOwnerOperation::RuleProvidersSave { .. },
            Some(AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::RuleProviders,
                data: AndroidOwnerDataSnapshotData::RuleProviders(_),
            })
        ) | (
            AndroidOwnerOperation::AppRoutingSnapshot
                | AndroidOwnerOperation::AppRoutingModeSet { .. }
                | AndroidOwnerOperation::AppRoutingPackagesSet { .. },
            Some(AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::AppRouting,
                data: AndroidOwnerDataSnapshotData::AppRouting(_),
            })
        ) | (
            AndroidOwnerOperation::WebDavSettings
                | AndroidOwnerOperation::WebDavSettingsSave { .. }
                | AndroidOwnerOperation::WebDavTest { .. }
                | AndroidOwnerOperation::WebDavSyncNow,
            Some(AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::WebDav,
                data: AndroidOwnerDataSnapshotData::WebDavSettings(_),
            })
        )
    )
}

pub(super) fn snapshot_is_successful(
    operation: &AndroidOwnerOperation,
    snapshot: Option<&AndroidOwnerDataSnapshot>,
) -> bool {
    match (operation, snapshot.map(|snapshot| &snapshot.data)) {
        (AndroidOwnerOperation::FakeIpCacheClear, None) => true,
        (
            AndroidOwnerOperation::CoreVersionManagement { .. },
            Some(AndroidOwnerDataSnapshotData::CoreVersions(result)),
        ) => result.status().code == FfiErrorCode::Ok,
        (_, Some(AndroidOwnerDataSnapshotData::Profiles(result))) => {
            result.status.code == FfiErrorCode::Ok
        }
        (_, Some(AndroidOwnerDataSnapshotData::DnsSettings(result))) => {
            result.status.code == FfiErrorCode::Ok
        }
        (_, Some(AndroidOwnerDataSnapshotData::TunSettings(result))) => {
            result.status.code == FfiErrorCode::Ok && result.settings.is_some()
        }
        (_, Some(AndroidOwnerDataSnapshotData::FakeIpSettings(result))) => {
            result.status.code == FfiErrorCode::Ok
        }
        (_, Some(AndroidOwnerDataSnapshotData::Rules(result))) => {
            result.status.code == FfiErrorCode::Ok
        }
        (_, Some(AndroidOwnerDataSnapshotData::RuleProviders(result))) => {
            result.status.code == FfiErrorCode::Ok
        }
        (_, Some(AndroidOwnerDataSnapshotData::AppRouting(result))) => {
            result.status.code == FfiErrorCode::Ok && result.snapshot.is_some()
        }
        (_, Some(AndroidOwnerDataSnapshotData::WebDavSettings(result))) => {
            result.status.code == FfiErrorCode::Ok && result.settings.is_some()
        }
        _ => false,
    }
}

pub(super) fn readback_matches_snapshot(
    operation: &AndroidOwnerOperation,
    reply: &AndroidOwnerOperationReply,
) -> bool {
    match (
        operation,
        &reply.readback,
        reply.snapshot.as_ref().map(|s| &s.data),
    ) {
        (
            AndroidOwnerOperation::ProfilesList,
            AndroidOwnerOperationReadback::ProfilesList(readback),
            Some(AndroidOwnerDataSnapshotData::Profiles(snapshot)),
        ) => readback == snapshot,
        (
            AndroidOwnerOperation::ProfileDetail { .. },
            AndroidOwnerOperationReadback::ProfileDetail(_),
            Some(AndroidOwnerDataSnapshotData::Profiles(_)),
        ) => true,
        (
            AndroidOwnerOperation::ProfileSelect { target },
            AndroidOwnerOperationReadback::ProfileSelection(readback),
            Some(AndroidOwnerDataSnapshotData::Profiles(snapshot)),
        ) => match &readback.active_reference {
            super::AndroidActiveProfileReferenceObservation::Selected {
                target: observed, ..
            } => {
                readback.operation_status.code == FfiErrorCode::Ok
                    && observed == target
                    && snapshot.status.code == FfiErrorCode::Ok
            }
            super::AndroidActiveProfileReferenceObservation::Unselected { .. }
            | super::AndroidActiveProfileReferenceObservation::Unavailable { .. } => false,
        },
        (
            AndroidOwnerOperation::ProfileCreate { .. }
            | AndroidOwnerOperation::ProfileUpdate { .. }
            | AndroidOwnerOperation::ProfileSave { .. }
            | AndroidOwnerOperation::ProfileDelete { .. }
            | AndroidOwnerOperation::ProfileSubscriptionSave { .. }
            | AndroidOwnerOperation::ProfileSubscriptionClear { .. },
            AndroidOwnerOperationReadback::ProfileMutation(status),
            Some(AndroidOwnerDataSnapshotData::Profiles(_)),
        ) => status.code == FfiErrorCode::Ok,
        (
            AndroidOwnerOperation::CoreVersionManagement { .. },
            AndroidOwnerOperationReadback::CoreVersionManagement(readback),
            Some(AndroidOwnerDataSnapshotData::CoreVersions(snapshot)),
        ) => readback == snapshot,
        (
            AndroidOwnerOperation::CoreVersionManagement { .. },
            AndroidOwnerOperationReadback::CoreVersionManagement(readback),
            None,
        ) => readback.status().code != FfiErrorCode::Ok,
        (
            AndroidOwnerOperation::DnsSettings | AndroidOwnerOperation::DnsSettingsSave { .. },
            AndroidOwnerOperationReadback::DnsSettings(readback),
            Some(AndroidOwnerDataSnapshotData::DnsSettings(snapshot)),
        ) => readback == snapshot,
        (
            AndroidOwnerOperation::TunSettings | AndroidOwnerOperation::TunSettingsSave { .. },
            AndroidOwnerOperationReadback::TunSettings(readback),
            Some(AndroidOwnerDataSnapshotData::TunSettings(snapshot)),
        ) => readback == snapshot,
        (
            AndroidOwnerOperation::FakeIpSettings
            | AndroidOwnerOperation::FakeIpSettingsSave { .. },
            AndroidOwnerOperationReadback::FakeIpSettings(readback),
            Some(AndroidOwnerDataSnapshotData::FakeIpSettings(snapshot)),
        ) => readback == snapshot,
        (
            AndroidOwnerOperation::RulesList | AndroidOwnerOperation::RulesSave { .. },
            AndroidOwnerOperationReadback::Rules(readback),
            Some(AndroidOwnerDataSnapshotData::Rules(snapshot)),
        ) => readback == snapshot,
        (
            AndroidOwnerOperation::RuleProviders | AndroidOwnerOperation::RuleProvidersSave { .. },
            AndroidOwnerOperationReadback::RuleProviders(readback),
            Some(AndroidOwnerDataSnapshotData::RuleProviders(snapshot)),
        ) => readback == snapshot,
        (
            AndroidOwnerOperation::AppRoutingSnapshot
            | AndroidOwnerOperation::AppRoutingModeSet { .. }
            | AndroidOwnerOperation::AppRoutingPackagesSet { .. },
            AndroidOwnerOperationReadback::AppRouting(readback),
            Some(AndroidOwnerDataSnapshotData::AppRouting(snapshot)),
        ) => readback == snapshot,
        (
            AndroidOwnerOperation::WebDavSettings
            | AndroidOwnerOperation::WebDavSettingsSave { .. },
            AndroidOwnerOperationReadback::WebDavSettings(readback),
            Some(AndroidOwnerDataSnapshotData::WebDavSettings(snapshot)),
        ) => readback.status.code == FfiErrorCode::Ok && readback == snapshot,
        (
            AndroidOwnerOperation::WebDavTest { .. },
            AndroidOwnerOperationReadback::WebDavTest(status),
            Some(AndroidOwnerDataSnapshotData::WebDavSettings(snapshot)),
        ) => status.code == FfiErrorCode::Ok && snapshot.status.code == FfiErrorCode::Ok,
        (
            AndroidOwnerOperation::WebDavSyncNow,
            AndroidOwnerOperationReadback::WebDavSyncNow(result),
            Some(AndroidOwnerDataSnapshotData::WebDavSettings(snapshot)),
        ) => result.status.code == FfiErrorCode::Ok && snapshot.status.code == FfiErrorCode::Ok,
        (
            AndroidOwnerOperation::FakeIpCacheClear,
            AndroidOwnerOperationReadback::FakeIpCacheClear(_),
            None,
        ) => true,
        _ => false,
    }
}

