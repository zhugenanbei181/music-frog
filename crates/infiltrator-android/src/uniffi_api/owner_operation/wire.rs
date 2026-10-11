use super::*;

/// Service-only Messenger adapter. The JSON payload is decoded exhaustively
/// into the Rust operation enum before owner state is touched. The Messenger
/// envelope supplies sessions, Core generation and revision separately.
#[uniffi::export]
pub async fn android_dispatch_owner_operation_wire_json(
    operation_json: String,
    client_session_id: String,
    expected_owner_session_id: String,
    owner_session_id: String,
    expected_core_generation: u64,
    expected_data_revision: u64,
) -> AndroidOwnerOperationIpcResult {
    let fallback_context = AndroidOwnerOperationContext {
        client_session_id,
        expected_owner_session_id,
        owner_session_id,
        expected_core_generation,
        expected_data_revision,
    };
    let task_context = fallback_context.clone();
    get_runtime()
        .spawn(async move {
            let context = task_context;
            let request =
                match serde_json::from_str::<AndroidOwnerOperationRequest>(&operation_json) {
                    Ok(request) if request.protocol == AndroidOwnerOperationProtocolVersion::V4 => {
                        request
                    }
                    Ok(_) => {
                        return rejected_result(
                            context,
                            0,
                            android_owner_data_revision(),
                            FfiStatus::err(
                                FfiErrorCode::NotSupported,
                                "Android owner operation protocol version is unsupported",
                            ),
                            AndroidOwnerOperationIpcState::Failed,
                        );
                    }
                    Err(error) => {
                        return rejected_result(
                            context,
                            0,
                            android_owner_data_revision(),
                            FfiStatus::err(
                                FfiErrorCode::InvalidInput,
                                format!("Android owner operation payload is invalid: {error}"),
                            ),
                            AndroidOwnerOperationIpcState::Failed,
                        );
                    }
                };

            let Some(bridge) = get_android_bridge() else {
                return rejected_result(
                    context,
                    0,
                    android_owner_data_revision(),
                    FfiStatus::err(FfiErrorCode::NotReady, "Android host bridge is unavailable"),
                    AndroidOwnerOperationIpcState::Failed,
                );
            };
            if bridge.process_role() != AndroidHostProcessRole::CoreOwner {
                return rejected_result(
                    context,
                    0,
                    android_owner_data_revision(),
                    FfiStatus::err(
                        FfiErrorCode::NotReady,
                        "persisted Android product data is owned by the :vpn process",
                    ),
                    AndroidOwnerOperationIpcState::Failed,
                );
            }
            if context.client_session_id.trim().is_empty()
                || context.owner_session_id.trim().is_empty()
                || context.expected_owner_session_id != context.owner_session_id
            {
                return rejected_result(
                    context,
                    0,
                    android_owner_data_revision(),
                    FfiStatus::err(
                        FfiErrorCode::NotReady,
                        "Android owner IPC session fence did not match",
                    ),
                    AndroidOwnerOperationIpcState::StaleOwnerData,
                );
            }

            let _owner_operation_guard = match request.operation.mutates_data() {
                true => match try_owner_data_revision_fence() {
                    OwnerDataRevisionFenceAcquire::Acquired(guard) => guard,
                    OwnerDataRevisionFenceAcquire::Busy => {
                        let expected_generation = context.expected_core_generation;
                        let observed_revision = android_owner_data_revision();
                        return rejected_result(
                            context,
                            expected_generation,
                            observed_revision,
                            FfiStatus::err(
                                FfiErrorCode::NotReady,
                                "VPN configuration preparation or activation currently fences owner-data writes",
                            ),
                            AndroidOwnerOperationIpcState::VpnConfigurationBusy,
                        );
                    }
                },
                false => owner_data_revision_fence().await,
            };
            let lifecycle = android_core_lifecycle_status().await;
            let observed_generation = lifecycle.generation;
            let observed_revision = android_owner_data_revision();
            if !owner_data_fence_matches(&context, observed_generation, observed_revision) {
                return rejected_result(
                    context,
                    observed_generation,
                    observed_revision,
                    FfiStatus::err(
                        FfiErrorCode::NotReady,
                        "Android owner data request is stale; refresh owner state before retrying",
                    ),
                    AndroidOwnerOperationIpcState::StaleOwnerData,
                );
            }
            match owner_operation_mutation_admission(
                &request.operation,
                crate::uniffi_api::vpn::vpn_configuration_activation_status(),
            ) {
                OwnerOperationMutationAdmission::ReadOnly
                | OwnerOperationMutationAdmission::WriteAllowed => {}
                OwnerOperationMutationAdmission::VpnConfigurationBusy => {
                    return rejected_result(
                        context,
                        observed_generation,
                        observed_revision,
                        FfiStatus::err(
                            FfiErrorCode::NotReady,
                            "VPN configuration activation holds the owner-data revision fence",
                        ),
                        AndroidOwnerOperationIpcState::VpnConfigurationBusy,
                    );
                }
            }
            if request.operation.mutates_data() && observed_revision == u64::MAX {
                return rejected_result(
                    context,
                    observed_generation,
                    observed_revision,
                    FfiStatus::err(
                        FfiErrorCode::InvalidState,
                        "Android owner data revision is exhausted",
                    ),
                    AndroidOwnerOperationIpcState::Failed,
                );
            }
            if matches!(
                &request.operation,
                AndroidOwnerOperation::AppRoutingModeSet { .. }
                    | AndroidOwnerOperation::AppRoutingPackagesSet { .. }
            ) && app_routing_revision() == u64::MAX
            {
                return rejected_result(
                    context,
                    observed_generation,
                    observed_revision,
                    FfiStatus::err(
                        FfiErrorCode::InvalidState,
                        "Android app-routing revision is exhausted",
                    ),
                    AndroidOwnerOperationIpcState::Failed,
                );
            }

            execute_operation(request.operation, context, observed_generation).await
        })
        .await
        .unwrap_or_else(|error| {
            rejected_result(
                fallback_context,
                0,
                android_owner_data_revision(),
                FfiStatus::err(
                    FfiErrorCode::Unknown,
                    format!("Android owner operation task failed: {error}"),
                ),
                AndroidOwnerOperationIpcState::Failed,
            )
        })
}

/// The only UniFFI client entry point for typed Android owner operations.
/// Product UI supplies the Rust enum directly; this adapter serializes it to
/// the Messenger wire and returns the owner-authored typed reply.
#[uniffi::export]
pub async fn android_execute_owner_operation(
    operation: AndroidOwnerOperation,
) -> AndroidOwnerOperationClientResult {
    get_runtime()
        .spawn(async move {
            let expected_operation = operation.clone();
            match route_owner_operation(operation).await {
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    match validate_owner_operation_reply(&expected_operation, *reply) {
                        Ok(reply) => AndroidOwnerOperationClientResult::Completed {
                            reply: Box::new(reply),
                        },
                        Err(status) => AndroidOwnerOperationClientResult::Failed { status },
                    }
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {
                    AndroidOwnerOperationClientResult::Failed {
                        status: FfiStatus::err(
                            FfiErrorCode::NotReady,
                            "owner operations execute through the :vpn Messenger dispatcher",
                        ),
                    }
                }
                Err(status) => AndroidOwnerOperationClientResult::Failed { status },
            }
        })
        .await
        .unwrap_or_else(|error| AndroidOwnerOperationClientResult::Failed {
            status: FfiStatus::err(
                FfiErrorCode::Unknown,
                format!("Android owner operation client task failed: {error}"),
            ),
        })
}

pub(super) fn validate_owner_operation_reply(
    operation: &AndroidOwnerOperation,
    reply: AndroidOwnerOperationReply,
) -> Result<AndroidOwnerOperationReply, FfiStatus> {
    if reply.protocol != AndroidOwnerOperationProtocolVersion::V4 {
        return Err(FfiStatus::err(
            FfiErrorCode::NotSupported,
            "Android owner operation reply protocol is unsupported",
        ));
    }
    if reply.context.client_session_id.trim().is_empty()
        || reply.context.owner_session_id.trim().is_empty()
        || reply.context.expected_owner_session_id != reply.context.owner_session_id
        || reply.context.expected_core_generation != reply.observed_core_generation
    {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android owner operation reply did not match its session or generation fence",
        ));
    }
    if reply.data_domain != Some(operation.data_domain())
        || !readback_matches_operation(operation, &reply.readback)
        || !snapshot_matches_operation(operation, reply.snapshot.as_ref())
    {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android owner returned a mismatched operation or data snapshot",
        ));
    }

    let status = status_from_readback(&reply.readback);
    let advances = should_advance_revision(
        operation.mutates_data(),
        operation.advances_revision_even_on_failure(),
        &reply.readback,
        status.code,
    );
    let expected_revision = if advances {
        reply
            .context
            .expected_data_revision
            .checked_add(1)
            .ok_or_else(|| {
                FfiStatus::err(
                    FfiErrorCode::InvalidState,
                    "Android owner data revision is exhausted",
                )
            })?
    } else {
        reply.context.expected_data_revision
    };
    if reply.data_revision != expected_revision {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android owner operation reply did not match its data revision fence",
        ));
    }

    if status.code == FfiErrorCode::Ok && !snapshot_is_successful(reply.snapshot.as_ref()) {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android owner operation succeeded without a readable data snapshot",
        ));
    }
    if status.code == FfiErrorCode::Ok && !readback_matches_snapshot(operation, &reply) {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android owner readback did not match its data snapshot",
        ));
    }
    Ok(reply)
}

fn readback_matches_operation(
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
            AndroidOwnerOperation::ProfileCreate { .. }
                | AndroidOwnerOperation::ProfileSelect { .. }
                | AndroidOwnerOperation::ProfileUpdate { .. }
                | AndroidOwnerOperation::ProfileSave { .. }
                | AndroidOwnerOperation::ProfileDelete { .. }
                | AndroidOwnerOperation::ProfileSubscriptionSave { .. }
                | AndroidOwnerOperation::ProfileSubscriptionClear { .. },
            AndroidOwnerOperationReadback::ProfileMutation(_)
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

fn snapshot_matches_operation(
    operation: &AndroidOwnerOperation,
    snapshot: Option<&AndroidOwnerDataSnapshot>,
) -> bool {
    if matches!(operation, AndroidOwnerOperation::FakeIpCacheClear) {
        return snapshot.is_none();
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

fn snapshot_is_successful(snapshot: Option<&AndroidOwnerDataSnapshot>) -> bool {
    match snapshot.map(|snapshot| &snapshot.data) {
        Some(AndroidOwnerDataSnapshotData::Profiles(result)) => {
            result.status.code == FfiErrorCode::Ok
        }
        Some(AndroidOwnerDataSnapshotData::DnsSettings(result)) => {
            result.status.code == FfiErrorCode::Ok
        }
        Some(AndroidOwnerDataSnapshotData::TunSettings(result)) => {
            result.status.code == FfiErrorCode::Ok && result.settings.is_some()
        }
        Some(AndroidOwnerDataSnapshotData::FakeIpSettings(result)) => {
            result.status.code == FfiErrorCode::Ok
        }
        Some(AndroidOwnerDataSnapshotData::Rules(result)) => result.status.code == FfiErrorCode::Ok,
        Some(AndroidOwnerDataSnapshotData::RuleProviders(result)) => {
            result.status.code == FfiErrorCode::Ok
        }
        Some(AndroidOwnerDataSnapshotData::AppRouting(result)) => {
            result.status.code == FfiErrorCode::Ok && result.snapshot.is_some()
        }
        Some(AndroidOwnerDataSnapshotData::WebDavSettings(result)) => {
            result.status.code == FfiErrorCode::Ok && result.settings.is_some()
        }
        None => true,
    }
}

fn readback_matches_snapshot(
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
            AndroidOwnerOperation::ProfileCreate { .. }
            | AndroidOwnerOperation::ProfileSelect { .. }
            | AndroidOwnerOperation::ProfileUpdate { .. }
            | AndroidOwnerOperation::ProfileSave { .. }
            | AndroidOwnerOperation::ProfileDelete { .. }
            | AndroidOwnerOperation::ProfileSubscriptionSave { .. }
            | AndroidOwnerOperation::ProfileSubscriptionClear { .. },
            AndroidOwnerOperationReadback::ProfileMutation(status),
            Some(AndroidOwnerDataSnapshotData::Profiles(_)),
        ) => status.code == FfiErrorCode::Ok,
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

pub(super) fn owner_data_fence_matches(
    context: &AndroidOwnerOperationContext,
    observed_core_generation: u64,
    observed_data_revision: u64,
) -> bool {
    context.expected_core_generation == observed_core_generation
        && context.expected_data_revision == observed_data_revision
}

pub(super) async fn execute_operation(
    operation: AndroidOwnerOperation,
    context: AndroidOwnerOperationContext,
    observed_generation: u64,
) -> AndroidOwnerOperationIpcResult {
    use crate::uniffi_api::profiles;
    use crate::uniffi_api::webdav;

    let domain = operation.data_domain();
    let mutates_data = operation.mutates_data();
    let advances_on_failure = operation.advances_revision_even_on_failure();
    let _routing_guard = if domain == AndroidOwnerDataDomain::AppRouting {
        Some(APP_ROUTING_SERIALIZER.lock().await)
    } else {
        None
    };
    let (readback, snapshot) = match operation {
        AndroidOwnerOperation::ProfilesList => {
            let result = profiles::read_owner_profiles().await;
            let snapshot = AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::Profiles,
                data: AndroidOwnerDataSnapshotData::Profiles(result.clone()),
            };
            (
                AndroidOwnerOperationReadback::ProfilesList(result),
                Some(snapshot),
            )
        }
        AndroidOwnerOperation::ProfileDetail { name } => {
            let result = profiles::read_owner_profile_detail(name).await;
            let profiles = profiles::read_owner_profiles().await;
            let snapshot = AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::Profiles,
                data: AndroidOwnerDataSnapshotData::Profiles(profiles),
            };
            (
                AndroidOwnerOperationReadback::ProfileDetail(result),
                Some(snapshot),
            )
        }
        AndroidOwnerOperation::ProfileCreate { name, url } => {
            let status = profiles::create_owner_profile(name, url).await;
            profile_mutation_readback(status).await
        }
        AndroidOwnerOperation::ProfileSelect { name } => {
            let status = profiles::select_owner_profile(name).await;
            profile_mutation_readback(status).await
        }
        AndroidOwnerOperation::ProfileUpdate { name } => {
            let status = profiles::update_owner_profile(name).await;
            profile_mutation_readback(status).await
        }
        AndroidOwnerOperation::ProfileSave {
            name,
            content,
            activate,
        } => {
            let status = profiles::save_owner_profile(name, content, activate).await;
            profile_mutation_readback(status).await
        }
        AndroidOwnerOperation::ProfileDelete { name } => {
            let status = profiles::delete_owner_profile(name).await;
            profile_mutation_readback(status).await
        }
        AndroidOwnerOperation::ProfileSubscriptionSave {
            name,
            url,
            auto_update_enabled,
            update_interval_hours,
        } => {
            let status = profiles::save_owner_profile_subscription(
                name,
                url,
                auto_update_enabled,
                update_interval_hours,
            )
            .await;
            profile_mutation_readback(status).await
        }
        AndroidOwnerOperation::ProfileSubscriptionClear { name } => {
            let status = profiles::clear_owner_profile_subscription(name).await;
            profile_mutation_readback(status).await
        }
        AndroidOwnerOperation::DnsSettings => {
            let result = dns_settings().await;
            let snapshot = AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::DnsSettings,
                data: AndroidOwnerDataSnapshotData::DnsSettings(result.clone()),
            };
            (
                AndroidOwnerOperationReadback::DnsSettings(result),
                Some(snapshot),
            )
        }
        AndroidOwnerOperation::DnsSettingsSave { patch } => {
            let result = match dns_settings_save_local(patch).await {
                Ok(settings) => DnsSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => DnsSettingsResult {
                    status,
                    settings: None,
                },
            };
            let snapshot = dns_settings().await;
            (
                AndroidOwnerOperationReadback::DnsSettings(result),
                Some(AndroidOwnerDataSnapshot {
                    domain: AndroidOwnerDataDomain::DnsSettings,
                    data: AndroidOwnerDataSnapshotData::DnsSettings(snapshot),
                }),
            )
        }
        AndroidOwnerOperation::TunSettings => {
            let result = crate::uniffi_api::vpn::owner_tun_settings_read().await;
            let snapshot = AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::TunSettings,
                data: AndroidOwnerDataSnapshotData::TunSettings(result.clone()),
            };
            (
                AndroidOwnerOperationReadback::TunSettings(result),
                Some(snapshot),
            )
        }
        AndroidOwnerOperation::TunSettingsSave { patch } => {
            let result = crate::uniffi_api::vpn::owner_tun_settings_save(patch).await;
            let snapshot = crate::uniffi_api::vpn::owner_tun_settings_read().await;
            (
                AndroidOwnerOperationReadback::TunSettings(result),
                Some(AndroidOwnerDataSnapshot {
                    domain: AndroidOwnerDataDomain::TunSettings,
                    data: AndroidOwnerDataSnapshotData::TunSettings(snapshot),
                }),
            )
        }
        AndroidOwnerOperation::FakeIpSettings => {
            let result = fake_ip_settings().await;
            let snapshot = AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::FakeIpSettings,
                data: AndroidOwnerDataSnapshotData::FakeIpSettings(result.clone()),
            };
            (
                AndroidOwnerOperationReadback::FakeIpSettings(result),
                Some(snapshot),
            )
        }
        AndroidOwnerOperation::FakeIpSettingsSave { patch } => {
            let result = fake_ip_settings_save(patch).await;
            let snapshot = fake_ip_settings().await;
            (
                AndroidOwnerOperationReadback::FakeIpSettings(result),
                Some(AndroidOwnerDataSnapshot {
                    domain: AndroidOwnerDataDomain::FakeIpSettings,
                    data: AndroidOwnerDataSnapshotData::FakeIpSettings(snapshot),
                }),
            )
        }
        AndroidOwnerOperation::FakeIpCacheClear => {
            let result = fake_ip_cache_clear().await;
            (
                AndroidOwnerOperationReadback::FakeIpCacheClear(result),
                None,
            )
        }
        AndroidOwnerOperation::RulesList => {
            let result = load_rules_local().await;
            let snapshot = AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::Rules,
                data: AndroidOwnerDataSnapshotData::Rules(result.clone()),
            };
            (AndroidOwnerOperationReadback::Rules(result), Some(snapshot))
        }
        AndroidOwnerOperation::RulesSave { rules } => {
            let result = save_rules_local(rules).await;
            let snapshot = load_rules_local().await;
            (
                AndroidOwnerOperationReadback::Rules(result),
                Some(AndroidOwnerDataSnapshot {
                    domain: AndroidOwnerDataDomain::Rules,
                    data: AndroidOwnerDataSnapshotData::Rules(snapshot),
                }),
            )
        }
        AndroidOwnerOperation::RuleProviders => {
            let result = load_rule_providers_local().await;
            let snapshot = AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::RuleProviders,
                data: AndroidOwnerDataSnapshotData::RuleProviders(result.clone()),
            };
            (
                AndroidOwnerOperationReadback::RuleProviders(result),
                Some(snapshot),
            )
        }
        AndroidOwnerOperation::RuleProvidersSave { providers } => {
            let result = save_rule_providers_local(providers).await;
            let snapshot = load_rule_providers_local().await;
            (
                AndroidOwnerOperationReadback::RuleProviders(result),
                Some(AndroidOwnerDataSnapshot {
                    domain: AndroidOwnerDataDomain::RuleProviders,
                    data: AndroidOwnerDataSnapshotData::RuleProviders(snapshot),
                }),
            )
        }
        AndroidOwnerOperation::AppRoutingSnapshot => {
            let result = load_android_app_routing();
            let snapshot = AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::AppRouting,
                data: AndroidOwnerDataSnapshotData::AppRouting(result.clone()),
            };
            (
                AndroidOwnerOperationReadback::AppRouting(result),
                Some(snapshot),
            )
        }
        AndroidOwnerOperation::AppRoutingModeSet { mode } => {
            let status = match app_routing_mutation_permission() {
                AppRoutingMutationPermission::Allowed => set_android_app_routing_mode(mode),
                AppRoutingMutationPermission::Blocked(status) => status,
            };
            app_routing_mutation_readback(status)
        }
        AndroidOwnerOperation::AppRoutingPackagesSet {
            packages,
            selection,
        } => {
            let status = match app_routing_mutation_permission() {
                AppRoutingMutationPermission::Allowed => {
                    set_android_app_routing_packages(packages, selection)
                }
                AppRoutingMutationPermission::Blocked(status) => status,
            };
            app_routing_mutation_readback(status)
        }
        AndroidOwnerOperation::WebDavSettings => {
            let result = webdav::read_owner_settings().await;
            let snapshot = AndroidOwnerDataSnapshot {
                domain: AndroidOwnerDataDomain::WebDav,
                data: AndroidOwnerDataSnapshotData::WebDavSettings(result.clone()),
            };
            (
                AndroidOwnerOperationReadback::WebDavSettings(result),
                Some(snapshot),
            )
        }
        AndroidOwnerOperation::WebDavSettingsSave { settings } => {
            let result = webdav::save_owner_settings(settings).await;
            let snapshot = webdav::read_owner_settings().await;
            (
                AndroidOwnerOperationReadback::WebDavSettings(result),
                Some(AndroidOwnerDataSnapshot {
                    domain: AndroidOwnerDataDomain::WebDav,
                    data: AndroidOwnerDataSnapshotData::WebDavSettings(snapshot),
                }),
            )
        }
        AndroidOwnerOperation::WebDavTest { settings } => {
            let result = webdav::test_owner_connection(settings).await;
            let snapshot = webdav::read_owner_settings().await;
            (
                AndroidOwnerOperationReadback::WebDavTest(result),
                Some(AndroidOwnerDataSnapshot {
                    domain: AndroidOwnerDataDomain::WebDav,
                    data: AndroidOwnerDataSnapshotData::WebDavSettings(snapshot),
                }),
            )
        }
        AndroidOwnerOperation::WebDavSyncNow => {
            let result = webdav::sync_owner_now().await;
            let snapshot = webdav::read_owner_settings().await;
            (
                AndroidOwnerOperationReadback::WebDavSyncNow(result),
                Some(AndroidOwnerDataSnapshot {
                    domain: AndroidOwnerDataDomain::WebDav,
                    data: AndroidOwnerDataSnapshotData::WebDavSettings(snapshot),
                }),
            )
        }
    };
    let status = status_from_readback(&readback);
    let should_advance =
        should_advance_revision(mutates_data, advances_on_failure, &readback, status.code);
    if domain == AndroidOwnerDataDomain::AppRouting
        && mutates_data
        && matches!(
            status.code,
            FfiErrorCode::Ok | FfiErrorCode::Io | FfiErrorCode::Unknown
        )
    {
        advance_app_routing_revision().unwrap_or_else(app_routing_revision);
    }
    let data_revision = if should_advance {
        advance_owner_data_revision().unwrap_or_else(android_owner_data_revision)
    } else {
        android_owner_data_revision()
    };
    let reply = AndroidOwnerOperationReply {
        protocol: AndroidOwnerOperationProtocolVersion::V4,
        context,
        observed_core_generation: observed_generation,
        data_revision,
        data_domain: Some(domain),
        snapshot,
        readback,
    };
    ipc_result_from_reply(
        AndroidOwnerOperationIpcState::Completed,
        status_from_readback(&reply.readback),
        observed_generation,
        data_revision,
        &reply,
    )
}

/// Resolves typed owner operations by process role. UI clients forward through
/// Messenger; the service dispatches wire requests inside the `:vpn` owner.
pub(in crate::uniffi_api) async fn route_owner_operation(
    operation: AndroidOwnerOperation,
) -> Result<AndroidOwnerOperationRoute, FfiStatus> {
    let Some(bridge) = get_android_bridge() else {
        #[cfg(not(target_os = "android"))]
        return Ok(AndroidOwnerOperationRoute::CoreOwner);
        #[cfg(target_os = "android")]
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android host bridge is unavailable",
        ));
    };
    match bridge.process_role() {
        AndroidHostProcessRole::CoreOwner => Ok(AndroidOwnerOperationRoute::CoreOwner),
        AndroidHostProcessRole::Unsupported => Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "persisted Android product data is unavailable in this process",
        )),
        AndroidHostProcessRole::UiClient => {
            let request = AndroidOwnerOperationRequest {
                protocol: AndroidOwnerOperationProtocolVersion::V4,
                operation,
            };
            let request_json = serde_json::to_string(&request).map_err(|error| {
                FfiStatus::err(
                    FfiErrorCode::InvalidInput,
                    format!("Android owner request serialization failed: {error}"),
                )
            })?;
            let reply_json = bridge
                .execute_owner_operation_json(&request_json)
                .await
                .map_err(|error| FfiStatus::err(FfiErrorCode::NotReady, error.to_string()))?;
            let reply = serde_json::from_str::<AndroidOwnerOperationReply>(&reply_json).map_err(
                |error| {
                    FfiStatus::err(
                        FfiErrorCode::NotReady,
                        format!("Android owner returned an invalid typed reply: {error}"),
                    )
                },
            )?;
            if reply.protocol != AndroidOwnerOperationProtocolVersion::V4 {
                return Err(FfiStatus::err(
                    FfiErrorCode::NotSupported,
                    "Android owner reply protocol version is unsupported",
                ));
            }
            match &reply.readback {
                AndroidOwnerOperationReadback::Rejected(status) => Err(status.clone()),
                _ => Ok(AndroidOwnerOperationRoute::UiClient(Box::new(reply))),
            }
        }
    }
}
