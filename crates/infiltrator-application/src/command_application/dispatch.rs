//! The shared command dispatch table for `CommandApplication`.
//!
//! Owns the single `match` that routes every inbound `CommandIntent` to
//! the application facade or host port that implements it. Split out of the
//! application module so the composition/accessor surface stays readable.

use super::vpn_start_result::confirmed;
use super::*;
use crate::profile_aggregation_application::ProfileAggregationApplication;
use crate::profile_application::ProfileRuntimeAccess;
use crate::protocol_codec_application::ProtocolCodecApplication;
use crate::runtime_configuration_application::RuntimeConfigurationApplication;
use crate::subscription_import_application::SubscriptionImportApplication;
use infiltrator_contract::android_control::VpnAuthorizationState;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::network_roaming::NetworkRoamingStatus;
use infiltrator_contract::pac::PacRequest;
use infiltrator_contract::privileged_network::PrivilegedNetworkRequest;
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_contract::rule_edit::RuleDraft;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::runtime_configuration::ObservedValue;
use std::sync::Arc;

impl CommandApplication {
    pub(super) fn profile_access(&self) -> ProfileRuntimeAccess<dyn ActiveProfileReferenceApplier> {
        match self.profile_reference_applier.as_ref() {
            Some(runtime) => ProfileRuntimeAccess::CoreOwner(Arc::clone(runtime)),
            None => ProfileRuntimeAccess::Offline,
        }
    }

    fn configuration_for_scope(
        &self,
        scope: Option<&CoreScope>,
    ) -> Result<RuntimeConfigurationApplication, Failure> {
        let scope = scope.ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Core configuration command has no validated scope",
                false,
            )
        })?;
        Ok(RuntimeConfigurationApplication::new(
            scope.clone(),
            self.runtime_configuration_ports()?,
        ))
    }

    /// Route one inbound command. Lifecycle and routing-intent commands are
    /// owned by `CoreApplication`; everything else is dispatched here.
    pub(super) async fn execute_dispatch(
        &self,
        scope: Option<&CoreScope>,
        intent: CommandIntent,
    ) -> Result<(), Failure> {
        match intent {
            CommandIntent::SwitchProfile { profile_id } => {
                let profile = self.profile()?;
                profile
                    .activate_profile(self.profile_access(), &profile_id)
                    .await
                    .map(|_| ())
            }
            CommandIntent::ReadOutboundInventory
            | CommandIntent::SelectOutbound { .. }
            | CommandIntent::ProbeOutbound { .. }
            | CommandIntent::ProbeOutboundBatch { .. }
            | CommandIntent::ApplyOutboundUiPreference { .. } => Err(Failure::unsupported(
                "outbound commands require their typed output route",
            )),
            CommandIntent::SetRuntimeProbeSettings { settings } => {
                self.save_runtime_probe_settings(settings).await
            }
            CommandIntent::UpdateProfile { profile_id } => {
                let profile = self.profile()?;
                let source = self.subscription_source()?;
                if let Some(runtime) = self.application_runtime.clone() {
                    self.refresh_application(profile, runtime)
                        .refresh_profile(source.as_ref(), &profile_id)
                        .await
                        .map(|_| ())
                } else {
                    profile
                        .update_subscription(source.as_ref(), &profile_id)
                        .await
                        .map(|_| ())
                }
            }
            CommandIntent::UpdateAllSubscriptions => {
                let profile = self.profile()?;
                let source = self.subscription_source()?;
                if let Some(runtime) = self.application_runtime.clone() {
                    self.refresh_application(profile, runtime)
                        .refresh_all(source.as_ref(), BATCH_UPDATE_CONCURRENCY)
                        .await
                        .map(|_| ())
                } else {
                    profile
                        .update_all_subscriptions(source.as_ref(), BATCH_UPDATE_CONCURRENCY)
                        .await
                        .map(|_| ())
                }
            }
            CommandIntent::SaveSubscriptionFilter { .. } => {
                unreachable!("typed filter output is handled before unit dispatch")
            }
            CommandIntent::ClearDnsCache { .. } => {
                unreachable!("typed DNS cache output is handled before unit dispatch")
            }
            CommandIntent::ImportSubscription {
                profile_id,
                channel,
                source,
            } => {
                let port = self.import_source.clone().ok_or_else(|| {
                    Failure::unsupported(
                        "local file / clipboard import is not configured for this host",
                    )
                })?;
                let application = SubscriptionImportApplication::new(self.profile()?, port);
                let subscription_source = self.subscription_source()?;
                application
                    .import(subscription_source.as_ref(), &profile_id, channel, &source)
                    .await
                    .map(|_| ())
            }
            CommandIntent::RestoreSubscriptionBackup { profile_id } => self
                .profile()?
                .restore_inactive_profile_backup(&profile_id)
                .await
                .map(|_| ()),
            CommandIntent::UpdateSubscriptionFetchSettings {
                profile_id,
                user_agent,
                insecure_skip_verify,
            } => {
                let profile = self.profile()?;
                profile
                    .update_subscription_fetch_settings(
                        &profile_id,
                        user_agent,
                        insecure_skip_verify,
                    )
                    .await
            }
            CommandIntent::DeleteProfile { profile_id } => {
                self.profile()?.delete_profile(&profile_id).await
            }
            CommandIntent::PreviewProfileAggregation { draft } => {
                let application = ProfileAggregationApplication::new(self.profile()?);
                application.preview(&draft).await.map(|_| ())
            }
            CommandIntent::CreateAggregatedProfile { draft } => {
                let application = ProfileAggregationApplication::new(self.profile()?);
                application
                    .create_profile_with_access(self.profile_access(), &draft)
                    .await
                    .map(|_| ())
            }
            CommandIntent::SaveAggregationTemplate { name, draft } => {
                let application = ProfileAggregationApplication::new(self.profile()?);
                application.save_template(&name, &draft).await.map(|_| ())
            }
            CommandIntent::DeleteAggregationTemplate { name } => {
                let application = ProfileAggregationApplication::new(self.profile()?);
                application.delete_template(&name).await.map(|_| ())
            }
            CommandIntent::ReAggregateProfile { template_name } => {
                let application = ProfileAggregationApplication::new(self.profile()?);
                application
                    .reaggregate(self.profile_access(), &template_name)
                    .await
                    .map(|_| ())
            }
            CommandIntent::PrepareCustomNodeDraft { draft } => {
                let preview = ProtocolCodecApplication::uri_from_draft(&draft).ok();
                ProtocolCodecApplication::publish_draft(*draft, preview);
                Ok(())
            }
            CommandIntent::ImportCustomNodeUri { uri } => {
                // DUAL-05-14: decode through the shared codec and publish the
                // typed draft for both surfaces. Nothing is persisted here.
                match ProtocolCodecApplication::draft_from_uri(&uri) {
                    Ok(draft) => {
                        let preview = ProtocolCodecApplication::uri_from_draft(&draft).ok();
                        ProtocolCodecApplication::publish_draft(draft, preview);
                        Ok(())
                    }
                    Err(failure) => {
                        ProtocolCodecApplication::publish_error(failure.message.clone());
                        Err(failure)
                    }
                }
            }
            CommandIntent::SaveCustomNodeDraft { draft } => {
                // DUAL-05-14: select engine-owned protocol and CA codecs,
                // verify both typed readbacks, and commit their exact output
                // through the source-bound workspace transaction.
                let trust = draft.params.tls_trust.clone();
                let runtime = self.profile_reference_applier.clone().ok_or_else(|| {
                    Failure::unsupported(
                        "custom protocol editing requires an engine profile transaction",
                    )
                })?;
                let commit = ProtocolCodecApplication::save_draft_to_active_profile(
                    &self.profile()?,
                    runtime,
                    &draft,
                    ProfileWriteAuthorization::RequireUnlock,
                )
                .await?;
                ProtocolCodecApplication::publish_commit(&commit);
                // DUAL-05-13: the CA request is resolved against the host
                // reader *after* the profile committed, so a reported load can
                // never describe a document that was not written.
                ProtocolCodecApplication::publish_ca_trust(
                    &trust,
                    self.certificate_authority.as_deref(),
                );
                Ok(())
            }
            CommandIntent::ScanDialerChains => {
                // DUAL-05-09/10: one shared analyzer over the active profile;
                // both surfaces render the published report.
                let content = self.profile()?.current_profile().await?;
                ProtocolCodecApplication::publish_dialer_report(&content)?;
                Ok(())
            }
            CommandIntent::UpdateCustomNodeDraftField { field, value } => {
                // DUAL-05-09/13: a typed, whitelisted draft edit; an unknown
                // field is refused by the shared application.
                ProtocolCodecApplication::update_draft_field(&field, &value)?;
                Ok(())
            }
            CommandIntent::ResolveCertificateAuthority { trust } => {
                // DUAL-05-13: resolve and publish; the outcome is a state, not
                // an error, because a host without a reader is expected.
                ProtocolCodecApplication::publish_ca_trust(
                    &trust,
                    self.certificate_authority.as_deref(),
                );
                Ok(())
            }
            CommandIntent::SetSubscriptionAutoReload {
                profile_id,
                enabled,
            } => {
                // DUAL-07-09: a host without the Core-owner profile applier must
                // reject the enable action with a typed failure instead of
                // persisting a preference that can only silently no-op.
                if enabled && self.profile_reference_applier.is_none() {
                    return Err(Failure::unsupported(
                        "this host exposes no Core-owner profile applier, so auto reload cannot be enabled",
                    ));
                }
                self.profile()?
                    .update_subscription_auto_reload(&profile_id, enabled)
                    .await
            }
            CommandIntent::UpdateSubscriptionSchedule { profile_id, draft } => {
                self.profile()?
                    .update_subscription_schedule(&profile_id, &draft)
                    .await
            }
            CommandIntent::RefreshRuleProviders => {
                let scope = scope.ok_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "rule-provider refresh has no validated Core scope",
                        true,
                    )
                })?;
                self.rule_provider_runtime()?
                    .refresh_all(scope)
                    .await
                    .map(|_| ())
            }
            CommandIntent::RefreshRuleProvider { reference } => {
                let scope = scope.ok_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "rule-provider refresh has no validated Core scope",
                        true,
                    )
                })?;
                self.rule_provider_runtime()?
                    .refresh(scope, reference.inventory_revision, reference.id)
                    .await
                    .map(|_| ())
            }
            CommandIntent::CloseConnection { id } => {
                let scope = scope.cloned().ok_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "connection close has no validated Core scope",
                        false,
                    )
                })?;
                self.connections()?.close_for_scope(scope, &id).await
            }
            CommandIntent::CloseAllConnections => {
                let scope = scope.cloned().ok_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "close-all has no validated Core scope",
                        false,
                    )
                })?;
                self.connections()?.close_all_for_scope(scope).await
            }
            CommandIntent::ApplyDnsSettings { patch } => self
                .configuration()?
                .apply_dns_settings_with_access(self.profile_access(), patch)
                .await
                .map(|_| ()),
            CommandIntent::RunDoctorDiagnostics => self.doctor()?.run(None).await.map(|_| ()),
            CommandIntent::BootstrapDoctor => self.doctor()?.bootstrap().await.map(|_| ()),
            CommandIntent::RepairDoctorIssue { check_id } => {
                self.doctor()?.fix(Some(check_id)).await.map(|_| ())
            }
            CommandIntent::RepairAllDoctorIssues => self.doctor()?.fix(None).await.map(|_| ()),
            CommandIntent::SetAppRoutingPackageSelection { app_id, selection } => self
                .routing()?
                .set_package_selection(&app_id, selection.into()),
            CommandIntent::SetAppRoutingMode { mode } => self.routing()?.set_mode(mode.into()),
            CommandIntent::SetAppRule { app_id, rule } => {
                self.routing()?.set_rule(&app_id, rule.into())
            }
            CommandIntent::SyncNow => {
                let settings = self.settings()?.load_hydrated().await?;
                self.sync()?
                    .sync(settings.webdav, settings.configs_dir)
                    .await
                    .map(|_| ())
            }
            intent @ (CommandIntent::CreateBackupSnapshot { .. }
            | CommandIntent::PrepareSnapshotRestore { .. }
            | CommandIntent::ConfirmSnapshotRestore { .. }
            | CommandIntent::CancelSnapshotRestore { .. }
            | CommandIntent::LoadSnapshotDiff { .. }
            | CommandIntent::LoadSnapshotHistory { .. }
            | CommandIntent::PruneSnapshots { .. }) => {
                self.execute_snapshot_output(intent).await.map(|_| ())
            }
            CommandIntent::LoadProfileDocument { .. }
            | CommandIntent::SaveProfileDocument { .. }
            | CommandIntent::LoadProfileOptions { .. }
            | CommandIntent::SaveMixinOverlay { .. } => {
                unreachable!("typed profile outputs are dispatched before unit commands")
            }
            CommandIntent::RollbackCore => self.versions()?.rollback().await.map(|_| ()),
            CommandIntent::PrepareServiceMode => self.service_mode()?.prepare().await.map(|_| ()),
            CommandIntent::RepairPortConflicts => self.port_conflicts()?.repair().await.map(|_| ()),
            CommandIntent::CheckUpdates => {
                let settings = self.settings()?.load().await?;
                let versions = self.versions()?;
                let channel = parse_release_channel(&settings.core_channel, versions.engine_id())?;
                versions.latest(channel).await.map(|_| ())
            }
            CommandIntent::SetLanguage { preference } => {
                self.update_setting("language", preference.as_setting())
                    .await
            }
            CommandIntent::UpdateSetting { key, value } => self.update_setting(&key, &value).await,
            CommandIntent::SetRoutingIntent { .. } | CommandIntent::GetRoutingIntent => Err(
                Failure::unsupported("routing intent requires its validated Core scope route"),
            ),
            CommandIntent::SetCoreLogLevel { level } => {
                self.runtime_queries()?.set_core_log_level(level).await
            }
            CommandIntent::SetTunStack { stack } => self
                .configuration_for_scope(scope)?
                .set_tun_stack(stack)
                .await
                .map(|_| ()),
            CommandIntent::ToggleTun { enabled } => self
                .configuration_for_scope(scope)?
                .set_tun_enabled(enabled)
                .await
                .map(|_| ()),
            CommandIntent::SetTunAutoRoute { enabled } => self
                .configuration_for_scope(scope)?
                .set_tun_auto_route(enabled)
                .await
                .map(|_| ()),
            CommandIntent::SetTunStrictRoute { enabled } => self
                .configuration_for_scope(scope)?
                .set_tun_strict_route(enabled)
                .await
                .map(|_| ()),
            CommandIntent::ProbeTunMtu => {
                let configuration_ports = self.runtime_configuration_ports()?;
                let scope = scope.ok_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "TUN MTU apply has no validated Core scope",
                        false,
                    )
                })?;
                let snapshot = self
                    .mtu()?
                    .probe_and_apply(configuration_ports, scope.clone())
                    .await;
                match snapshot.state {
                    MtuProbeState::Ready => Ok(()),
                    MtuProbeState::Unsupported => Err(Failure::unsupported(
                        "current host does not expose physical-link MTU probing",
                    )),
                    MtuProbeState::Failed { failure } => Err(failure),
                    MtuProbeState::Unknown | MtuProbeState::Probing => Err(Failure::new(
                        ErrorCode::InvalidState,
                        "MTU probe did not reach a terminal state",
                        true,
                    )),
                }
            }
            CommandIntent::SetSystemProxy { enabled } => {
                let endpoint = if enabled {
                    let config = self.configuration_for_scope(scope)?;
                    let ports = config.observe_proxy_ports().await?;
                    let mixed_port = reported_port(&ports.value.mixed_port);
                    let http_port = reported_port(&ports.value.http_port);
                    let port = if mixed_port > 0 {
                        mixed_port
                    } else {
                        http_port
                    };
                    (port > 0)
                        .then(|| format!("127.0.0.1:{port}"))
                        .ok_or_else(|| {
                            Failure::new(
                                ErrorCode::NotReady,
                                "current configuration has no HTTP proxy endpoint",
                                true,
                            )
                        })
                        .map(Some)?
                } else {
                    None
                };
                self.system_proxy()?
                    .set_enabled(enabled, endpoint, None)
                    .await
                    .map(|_| ())
            }
            CommandIntent::SetLanSharing {
                enabled,
                mixed_port,
                bind_address,
            } => self
                .configuration_for_scope(scope)?
                .set_lan_sharing(enabled, mixed_port, &bind_address)
                .await
                .map(|_| ()),
            CommandIntent::SetLanSecurity {
                allowed_ips,
                disallowed_ips,
                skip_auth_prefixes,
                authentication_enabled,
                credentials,
            } => self
                .configuration_for_scope(scope)?
                .set_lan_security(
                    &allowed_ips,
                    &disallowed_ips,
                    &skip_auth_prefixes,
                    if authentication_enabled {
                        let credentials = credentials.ok_or_else(|| {
                            Failure::new(
                                ErrorCode::InvalidInput,
                                "LAN authentication requires credentials",
                                false,
                            )
                        })?;
                        infiltrator_ports::runtime_configuration::LanAuthenticationUpdate::Enabled(
                            credentials,
                        )
                    } else {
                        infiltrator_ports::runtime_configuration::LanAuthenticationUpdate::Disabled
                    },
                )
                .await
                .map(|_| ()),
            CommandIntent::SetIpv6Routing { enabled } => self
                .configuration_for_scope(scope)?
                .set_ipv6_routing(enabled)
                .await
                .map(|_| ()),
            CommandIntent::ScanUwpApps => {
                self.uwp_loopback()?.snapshot().await;
                Ok(())
            }
            CommandIntent::SetUwpAppExemption { sid, exempt } => self
                .uwp_loopback()?
                .set_exempt(&sid, exempt)
                .await
                .map(|_| ()),
            CommandIntent::SetAllUwpExemptions { exempt } => {
                self.uwp_loopback()?.set_all(exempt).await.map(|_| ())
            }
            CommandIntent::ApplyPac {
                enabled,
                bypass_domains,
                bypass_lan,
                minify,
            } => self
                .pac()?
                .apply_scoped(
                    scope
                        .ok_or_else(|| {
                            Failure::new(
                                ErrorCode::NotReady,
                                "PAC configuration has no validated Core scope",
                                false,
                            )
                        })?
                        .clone(),
                    PacRequest {
                        enabled,
                        bypass_domains,
                        bypass_lan,
                        minify,
                    },
                )
                .await
                .map(|_| ()),
            CommandIntent::RefreshNetworkRoaming => {
                let snapshot = self.network_roaming()?.refresh().await;
                match snapshot.status {
                    NetworkRoamingStatus::Failed { failure } => Err(failure),
                    NetworkRoamingStatus::Unsupported { reason } => {
                        Err(Failure::unsupported(reason))
                    }
                    _ => Ok(()),
                }
            }
            CommandIntent::RepairNetworkRoutes => {
                self.network_roaming()?.force_repair().await.map(|_| ())
            }
            CommandIntent::CheckVpnAuthorization => {
                match self.vpn()?.authorization_state().await? {
                    VpnAuthorizationState::Granted => Ok(()),
                    VpnAuthorizationState::Required => Err(Failure::new(
                        ErrorCode::Permission,
                        "Android VPN authorization is required before starting the proxy",
                        false,
                    )),
                }
            }
            CommandIntent::StartProxy
            | CommandIntent::StopProxy
            | CommandIntent::ResumeKernel
            | CommandIntent::PauseKernel
            | CommandIntent::RestartKernel => Err(Failure::unsupported(
                "product lifecycle intents must be executed by CoreApplication",
            )),
            CommandIntent::StartVpn => {
                let snapshot = self.vpn()?.request_start().await?;
                confirmed(snapshot)
            }
            CommandIntent::StopVpn => self.vpn()?.stop().await.map(|_| ()),
            CommandIntent::ResetRuleHitCounters { expected_source } => self
                .rule_tracer()?
                .clear_hits(&expected_source)
                .await
                .map(|_| ()),
            CommandIntent::ApplyTracerRuleOverride { request } => {
                let result = self.rule_tracer()?.apply_override(&request).await;
                match result.into_failure() {
                    Some(failure) => Err(failure),
                    None => Ok(()),
                }
            }
            CommandIntent::ToggleRuleEnabled { index } => {
                self.edit_rules(|rules| edit::toggle_rule_enabled(rules, index))
                    .await
            }
            CommandIntent::CommitRuleList { request } => {
                self.rule_list
                    .as_ref()
                    .ok_or_else(|| {
                        Failure::unsupported("Atomic rule-list persistence is not composed")
                    })?
                    .commit(&request)
                    .await
            }
            CommandIntent::MoveRule { index, direction } => {
                self.edit_rules(|rules| edit::move_rule(rules, index, direction))
                    .await
            }
            CommandIntent::AddCustomRule {
                rule_type,
                payload,
                target,
            } => {
                let draft = RuleDraft {
                    rule_type,
                    payload,
                    target,
                };
                let entry = edit::build_custom_rule(&draft)
                    .map_err(|error| Failure::new(ErrorCode::InvalidInput, error, false))?;
                self.edit_rules(move |rules| edit::prepend_rules(rules, [entry]) > 0)
                    .await
            }
            CommandIntent::ApplyGameRoutingPresets { target } => {
                self.edit_rules(move |rules| edit::inject_game_presets(rules, &target) > 0)
                    .await
            }
            // DUAL-11-14: the kernel owns the database upgrade; the shared
            // gateway call is the only real trigger (`POST /upgrade/geo`).
            CommandIntent::UpgradeGeoDatabases => {
                let scope = scope.ok_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "Geo database trigger has no validated Core scope",
                        true,
                    )
                })?;
                self.geo_database()?.trigger(scope).await.map(|_| ())
            }
            CommandIntent::ApplyProfileConfiguration { request } => self
                .configuration()?
                .apply_profile_configuration_request(*request)
                .await
                .map(|_| ()),
            // DUAL-11-06: import a provider's real rules through the same
            // read/modify/write seam the other rule edits use.
            CommandIntent::UnpackRuleProvider { provider_name } => {
                self.unpack_rule_provider(scope, &provider_name).await
            }
            // DUAL-11-07: delete only the kernel's cached provider files.
            CommandIntent::PurgeRuleProviderCache => self.purge_rule_provider_cache().await,
            CommandIntent::RunPrivilegedNetworkRegression => self
                .privileged_network()?
                .run(PrivilegedNetworkRequest::standard())
                .await
                .map(|_| ()),
            CommandIntent::QueryDns { operation, request } => {
                self.dns_query
                    .as_ref()
                    .ok_or_else(|| Failure::unsupported("DNS query application is unavailable"))?
                    .query(operation, request)
                    .await
            }
            CommandIntent::TestDnsLatency => self.test_dns_latency().await,
            CommandIntent::TestDnsLeak => self.test_dns_leak().await,
            CommandIntent::RunStunProbe => self.run_stun_probe().await,
            CommandIntent::ClearLogs => {
                self.logs
                    .as_ref()
                    .ok_or_else(|| Failure::unsupported("log buffer is not composed"))?
                    .clear();
                Ok(())
            }
            CommandIntent::SetLogLevelFilter { level } => self
                .logs
                .as_ref()
                .ok_or_else(|| Failure::unsupported("log buffer is not composed"))?
                .set_filter(level),
            CommandIntent::RefreshPublicIpProbe
            | CommandIntent::SimulateRuleTrace { .. }
            | CommandIntent::RunScriptSandbox { .. }
            | CommandIntent::ClearScriptSandbox { .. }
            | CommandIntent::PrepareScriptExport { .. }
            | CommandIntent::SaveScriptExport { .. }
            | CommandIntent::CancelScriptExport { .. }
            | CommandIntent::PrepareLogExport
            | CommandIntent::SaveLogExport { .. }
            | CommandIntent::CancelLogExport { .. }
            | CommandIntent::ReorderOverviewCards { .. }
            | CommandIntent::ResetOverviewCardOrder
            | CommandIntent::StartCore
            | CommandIntent::StopCore
            | CommandIntent::RestartCore
            | CommandIntent::StartSpeedtest { .. }
            | CommandIntent::CancelSpeedtestRun { .. }
            | CommandIntent::SetSpeedtestConcurrency { .. }
            | CommandIntent::ResolveConflictKeepLocal
            | CommandIntent::ResolveConflictTakeRemote => Err(unsupported()),
        }
    }
}

fn reported_port(value: &ObservedValue<u16>) -> u16 {
    match value {
        ObservedValue::Unreported => 0,
        ObservedValue::Reported(port) => *port,
    }
}
