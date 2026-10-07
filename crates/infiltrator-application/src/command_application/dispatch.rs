//! The shared command dispatch table for `CommandApplication`.
//!
//! Owns the single `match` that routes every inbound `CommandIntent` to
//! the application facade or host port that implements it. Split out of the
//! application module so the composition/accessor surface stays readable.

use super::*;
use crate::profile_aggregation_application::ProfileAggregationApplication;
use crate::protocol_codec_application::ProtocolCodecApplication;
use crate::proxy_application::test_proxy_delays;
use crate::subscription_import_application::SubscriptionImportApplication;
use infiltrator_contract::network_roaming::NetworkRoamingStatus;
use infiltrator_contract::pac::PacRequest;
use infiltrator_contract::privileged_network::PrivilegedNetworkRequest;
use infiltrator_contract::rule_edit::RuleDraft;
use infiltrator_contract::speedtest::SpeedtestScope;
use infiltrator_contract::vpn::VpnSessionState;
use infiltrator_domain::apply::ApplyStrategy;
use std::sync::Mutex;

impl CommandApplication {
    /// Route one inbound command. Lifecycle and proxy-mode commands are
    /// owned by `CoreApplication`; everything else is dispatched here.
    pub(super) async fn execute_dispatch(&self, intent: CommandIntent) -> Result<(), Failure> {
        match intent {
            CommandIntent::SwitchProfile { profile_id } => {
                let profile = self.profile()?;
                if let Some(runtime) = self.managed_runtime.clone() {
                    profile
                        .activate_profile(Some(runtime), &profile_id)
                        .await
                        .map(|_| ())
                } else {
                    profile.select_profile(&profile_id).await.map(|_| ())
                }
            }
            CommandIntent::SelectProxyNode { group, node } => self
                .runtime()?
                .switch_proxy(&group, &node)
                .await
                .map_err(Failure::from),
            CommandIntent::ToggleProxyGroupExpand { group } => {
                self.proxy_preferences()?.toggle_group_expand(&group)?;
                Ok(())
            }
            CommandIntent::SetProxyGroupExpanded { group, expanded } => {
                self.proxy_preferences()?
                    .set_group_expanded(&group, expanded)?;
                Ok(())
            }
            CommandIntent::SetProxySortOrder { order } => {
                self.proxy_preferences()?.set_sort_order(order)?;
                Ok(())
            }
            CommandIntent::SetProxyProbeOptions { options } => {
                self.save_probe_options(options).await
            }
            CommandIntent::SetProxySearchQuery { query } => {
                self.proxy_preferences()?.set_search_query(query)?;
                Ok(())
            }
            CommandIntent::ToggleFilterAlive { enabled } => {
                self.proxy_preferences()?.set_filter_alive(enabled)?;
                Ok(())
            }
            CommandIntent::ToggleFavoriteProxy { proxy } => {
                self.proxy_preferences()?.toggle_favorite(&proxy)?;
                Ok(())
            }
            CommandIntent::SetProxyCompactView { compact } => {
                self.proxy_preferences()?.set_compact_view(compact)?;
                Ok(())
            }
            CommandIntent::ReorderProxyGroups { group_names } => {
                self.apply_group_order(group_names).await
            }
            CommandIntent::ResetProxyGroupOrder => {
                self.proxy_preferences()?.reset_group_order()?;
                Ok(())
            }
            CommandIntent::TestDelay {
                group,
                url,
                timeout_ms,
            } => {
                let options = self.resolve_probe_options(url, timeout_ms).await?;
                if let Ok(speedtest) = self.speedtest() {
                    let scope = match group {
                        Some(g) => SpeedtestScope::SingleGroup(g),
                        None => SpeedtestScope::AllGroups,
                    };
                    speedtest
                        .test_delays(
                            scope,
                            Some(options.test_url.clone()),
                            Some(options.timeout_ms),
                        )
                        .await?;
                    return Ok(());
                }
                let runtime = self.runtime()?;
                let proxies = runtime.get_proxies().await.map_err(Failure::from)?;
                let candidates = delay_candidates(&proxies, group.as_deref())?;
                let outcomes = test_proxy_delays(
                    runtime,
                    candidates,
                    options.test_url,
                    options.timeout_ms,
                    DEFAULT_DELAY_CONCURRENCY,
                )
                .await;
                if let Some(failed) = outcomes
                    .iter()
                    .filter(|outcome| outcome.result.is_err())
                    .min_by(|left, right| left.proxy_name.cmp(&right.proxy_name))
                {
                    let failure = failed.result.as_ref().unwrap_err();
                    let failed_count = outcomes
                        .iter()
                        .filter(|outcome| outcome.result.is_err())
                        .count();
                    return Err(Failure::new(
                        failure.code.clone(),
                        format!(
                            "{failed_count}/{} proxy probes failed; {}: {}",
                            outcomes.len(),
                            failed.proxy_name,
                            failure.message
                        ),
                        failure.retryable,
                    ));
                }
                Ok(())
            }
            CommandIntent::TestNodeDelay {
                node,
                url,
                timeout_ms,
            } => {
                let options = self.resolve_probe_options(url, timeout_ms).await?;
                let runtime = self.runtime()?;
                let proxies = runtime.get_proxies().await.map_err(Failure::from)?;
                let proxy = proxies.get(&node).ok_or_else(|| {
                    Failure::new(
                        ErrorCode::InvalidInput,
                        "proxy identity is no longer present",
                        false,
                    )
                })?;
                if proxy.is_group() || matches!(proxy, Proxy::Unknown) {
                    return Err(Failure::new(
                        ErrorCode::InvalidInput,
                        "latency probing requires a known leaf proxy",
                        false,
                    ));
                }
                runtime
                    .test_delay(&node, &options.test_url, options.timeout_ms)
                    .await
                    .map_err(Failure::from)?;
                Ok(())
            }
            CommandIntent::RunSpeedtest { node, url } => {
                let speedtest = self.speedtest()?;
                speedtest.probe_node_jitter(&node, 5, url, None).await?;
                Ok(())
            }
            CommandIntent::RecordSpeedtestBandwidth {
                node,
                total_bytes,
                duration_ms,
            } => {
                let speedtest = self.speedtest()?;
                speedtest.record_bandwidth(&node, total_bytes, duration_ms)?;
                Ok(())
            }
            CommandIntent::RecordSpeedtestOutboundIp { node, ip, country } => {
                let speedtest = self.speedtest()?;
                speedtest.record_outbound_ip(&node, &ip, country.as_deref())?;
                Ok(())
            }
            CommandIntent::SetSpeedtestConcurrency { limit } => {
                let speedtest = self.speedtest()?;
                speedtest.set_concurrency(limit);
                Ok(())
            }
            CommandIntent::CancelSpeedtest => {
                let speedtest = self.speedtest()?;
                speedtest.cancel();
                Ok(())
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
                .restore_backup(&profile_id)
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
                    .create_profile_with_runtime(self.managed_runtime.clone(), &draft)
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
                    .reaggregate(self.managed_runtime.clone(), &template_name)
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
                // DUAL-05-14: splice the node into the active profile without
                // disturbing any other section, then commit through the same
                // apply transaction the other profile editors use.
                let trust = draft.params.tls_trust.clone();
                let committed = Arc::new(Mutex::new(None));
                let written = committed.clone();
                self.profile()?
                    .save_current_profile_content(
                        self.managed_runtime.clone(),
                        ApplyStrategy::PreferReload,
                        move |content| {
                            let commit = ProtocolCodecApplication::upsert_draft_into_profile(
                                content, &draft,
                            )
                            .map_err(|failure| failure.message)?;
                            let content = commit.profile_yaml.clone();
                            *written.lock().expect("protocol commit") = Some(commit);
                            Ok::<String, String>(content)
                        },
                    )
                    .await?;
                let commit = committed
                    .lock()
                    .expect("protocol commit")
                    .take()
                    .ok_or_else(|| {
                        Failure::new(
                            ErrorCode::Internal,
                            "profile transaction did not produce a protocol commit",
                            false,
                        )
                    })?;
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
                // DUAL-07-09: a host without a managed-runtime reload seam must
                // reject the enable action with a typed failure instead of
                // persisting a preference that can only silently no-op.
                if enabled && self.managed_runtime.is_none() {
                    return Err(Failure::unsupported(
                        "this host exposes no managed core-reload seam, so auto reload cannot be enabled",
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
                let runtime = self.runtime()?;
                let providers = runtime.get_rule_providers().await.map_err(Failure::from)?;
                for provider in providers {
                    runtime
                        .update_rule_provider(&provider.name)
                        .await
                        .map_err(Failure::from)?;
                }
                Ok(())
            }
            CommandIntent::CloseConnection { id } => self
                .runtime()?
                .close_connection(&id)
                .await
                .map_err(Failure::from),
            CommandIntent::CloseAllConnections => self
                .runtime()?
                .close_all_connections()
                .await
                .map_err(Failure::from),
            CommandIntent::ClearDnsCache { operation } => match self.dns_cache.as_ref() {
                Some(dns_cache) => {
                    let report = dns_cache.flush_with_id(operation).await?;
                    if report.fake_ip.is_flushed() || report.os_cache.is_flushed() {
                        Ok(())
                    } else {
                        Err(Failure::unsupported(
                            "neither DNS cache target has a drivable host adapter",
                        ))
                    }
                }
                None => Err(Failure::unsupported("DNS cache application is unavailable")),
            },
            CommandIntent::ApplyDnsSettings { patch } => self
                .configuration()?
                .apply_dns_settings_with_runtime(self.managed_runtime.clone(), patch)
                .await
                .map(|_| ()),
            CommandIntent::RunDoctorDiagnostics => self.doctor()?.run(None).await.map(|_| ()),
            CommandIntent::BootstrapDoctor => self.doctor()?.bootstrap().await.map(|_| ()),
            CommandIntent::RepairDoctorIssue { check_id } => {
                self.doctor()?.fix(Some(check_id)).await.map(|_| ())
            }
            CommandIntent::RepairAllDoctorIssues => self.doctor()?.fix(None).await.map(|_| ()),
            CommandIntent::ToggleAppRouting { app_id, enabled } => {
                self.routing()?.set_package_enabled(&app_id, enabled)
            }
            CommandIntent::SetAppRoutingMode { mode } => {
                let mode = parse_routing_mode(&mode)?;
                self.routing()?.set_mode(mode)
            }
            CommandIntent::SetAppRule { app_id, rule } => {
                let rule = parse_routing_rule(&rule)?;
                self.routing()?.set_rule(&app_id, rule)
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
                let channel = parse_release_channel(&settings.core_channel)?;
                self.versions()?.latest(channel).await.map(|_| ())
            }
            CommandIntent::SetLanguage { preference } => {
                self.update_setting("language", preference.as_setting())
                    .await
            }
            CommandIntent::UpdateSetting { key, value } => self.update_setting(&key, &value).await,
            CommandIntent::SetProxyMode { mode } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_proxy_mode(mode)
                    .await
            }
            CommandIntent::SetCoreLogLevel { level } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_core_log_level(level)
                    .await
            }
            CommandIntent::SetTunStack { stack } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_tun_stack(stack)
                    .await
            }
            CommandIntent::ToggleTun { enabled } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_tun_enabled(enabled)
                    .await
            }
            CommandIntent::SetTunAutoRoute { enabled } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_tun_auto_route(enabled)
                    .await
            }
            CommandIntent::SetTunStrictRoute { enabled } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_tun_strict_route(enabled)
                    .await
            }
            CommandIntent::ProbeTunMtu => {
                let runtime = self.runtime()?.clone();
                let snapshot = self.mtu()?.probe_and_apply(runtime).await;
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
                    let config = self.runtime()?.get_config().await.map_err(Failure::from)?;
                    let port = if config.mixed_port > 0 {
                        config.mixed_port
                    } else {
                        config.port
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
            } => RuntimeQueryApplication::new(self.runtime()?)
                .set_lan_sharing(enabled, mixed_port, &bind_address)
                .await
                .map(|_| ()),
            CommandIntent::SetLanSecurity {
                allowed_ips,
                disallowed_ips,
                skip_auth_prefixes,
                authentication_enabled,
                credentials,
            } => RuntimeQueryApplication::new(self.runtime()?)
                .set_lan_security(
                    &allowed_ips,
                    &disallowed_ips,
                    &skip_auth_prefixes,
                    authentication_enabled,
                    credentials.as_ref(),
                )
                .await
                .map(|_| ()),
            CommandIntent::SetIpv6Routing { enabled } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_ipv6_routing(enabled)
                    .await
                    .map(|_| ())
            }
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
                .apply(PacRequest {
                    enabled,
                    bypass_domains,
                    bypass_lan,
                    minify,
                })
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
            CommandIntent::StartVpn => {
                let snapshot = self.vpn()?.request_start().await?;
                match snapshot.state {
                    VpnSessionState::Unsupported { reason } => Err(Failure::unsupported(reason)),
                    VpnSessionState::Failed { failure } => Err(failure),
                    _ => Ok(()),
                }
            }
            CommandIntent::StopVpn => self.vpn()?.stop().await.map(|_| ()),
            CommandIntent::ResetRuleHitCounters { expected_source } => self
                .rule_tracer()?
                .clear_hits(&expected_source)
                .await
                .map(|_| ()),
            CommandIntent::SimulateRuleTrace { operation, request } => {
                let proxies = match &self.runtime {
                    Some(runtime) => runtime.get_proxies().await.ok(),
                    None => None,
                };
                self.rule_tracer()?
                    .simulate(operation, request, proxies.as_ref())
                    .await
            }
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
            CommandIntent::UpgradeGeoDatabases => self
                .runtime()?
                .upgrade_geo()
                .await
                .map_err(|error| Failure::new(ErrorCode::Network, error.to_string(), true)),
            // DUAL-11-14: validate the edited document and hand it to the same
            // shared configuration use-case the profile writers use. An invalid
            // document is a typed input error and never a silent no-op.
            CommandIntent::ApplyRulesJsonDocument { section, json } => {
                self.apply_rules_json_document(section, &json).await
            }
            // DUAL-11-06: import a provider's real rules through the same
            // read/modify/write seam the other rule edits use.
            CommandIntent::UnpackRuleProvider { provider_name } => {
                self.unpack_rule_provider(&provider_name).await
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
            | CommandIntent::ToggleIncludeSystemApps { .. }
            | CommandIntent::ResolveConflictKeepLocal
            | CommandIntent::ResolveConflictTakeRemote => Err(unsupported()),
        }
    }
}
