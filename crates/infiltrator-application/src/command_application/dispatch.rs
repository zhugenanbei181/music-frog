//! The shared command dispatch table for `CommandApplication`.
//!
//! Owns the single `match` that routes every inbound `CommandIntent` to
//! the application facade or host port that implements it. Split out of the
//! application module so the composition/accessor surface stays readable.

use super::*;

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
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.toggle_group_expand(&group);
                }
                Ok(())
            }
            CommandIntent::SetProxyGroupExpanded { group, expanded } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.set_group_expanded(&group, expanded);
                }
                Ok(())
            }
            CommandIntent::SetProxySortOrder { order } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.set_sort_order(order);
                }
                Ok(())
            }
            CommandIntent::ToggleFilterAlive { enabled } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.set_filter_alive(enabled);
                }
                Ok(())
            }
            CommandIntent::ToggleFavoriteProxy { proxy } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.toggle_favorite(&proxy);
                }
                Ok(())
            }
            CommandIntent::SetProxyCompactView { compact } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.set_compact_view(compact);
                }
                Ok(())
            }
            CommandIntent::ReorderProxyGroups { group_names } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.reorder_groups(group_names);
                }
                Ok(())
            }
            CommandIntent::ResetProxyGroupOrder => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.reset_group_order();
                }
                Ok(())
            }
            CommandIntent::TestDelay {
                group,
                url,
                timeout_ms,
            } => {
                if let Ok(speedtest) = self.speedtest() {
                    let scope = match group {
                        Some(g) => infiltrator_contract::speedtest::SpeedtestScope::SingleGroup(g),
                        None => infiltrator_contract::speedtest::SpeedtestScope::AllGroups,
                    };
                    speedtest.test_delays(scope, url, timeout_ms).await?;
                    return Ok(());
                }
                let runtime = self.runtime()?;
                let proxies = runtime.get_proxies().await.map_err(Failure::from)?;
                let candidates = delay_candidates(&proxies, group.as_deref())?;
                let test_url = url.unwrap_or_else(|| DEFAULT_DELAY_TEST_URL.to_string());
                let timeout = timeout_ms.unwrap_or(DEFAULT_DELAY_TIMEOUT_MS);
                let _ = crate::proxy_application::test_proxy_delays(
                    runtime,
                    candidates,
                    test_url,
                    timeout,
                    DEFAULT_DELAY_CONCURRENCY,
                )
                .await;
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
            CommandIntent::SaveSubscriptionFilter { profile_id, filter } => self
                .profile_options()?
                .save_filter(self.managed_runtime.clone(), &profile_id, &filter)
                .await
                .map(|_| ()),
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
                let application =
                    crate::subscription_import_application::SubscriptionImportApplication::new(
                        self.profile()?,
                        port,
                    );
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
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application.preview(&draft).await.map(|_| ())
            }
            CommandIntent::CreateAggregatedProfile { draft } => {
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application
                    .create_profile_with_runtime(self.managed_runtime.clone(), &draft)
                    .await
                    .map(|_| ())
            }
            CommandIntent::SaveAggregationTemplate { name, draft } => {
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application.save_template(&name, &draft).await.map(|_| ())
            }
            CommandIntent::DeleteAggregationTemplate { name } => {
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application.delete_template(&name).await.map(|_| ())
            }
            CommandIntent::ReAggregateProfile { template_name } => {
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application
                    .reaggregate(self.managed_runtime.clone(), &template_name)
                    .await
                    .map(|_| ())
            }
            CommandIntent::ImportCustomNodeUri { uri } => {
                // DUAL-05-14: decode through the shared codec and publish the
                // typed draft for both surfaces. Nothing is persisted here.
                match crate::protocol_codec_application::ProtocolCodecApplication::draft_from_uri(
                    &uri,
                ) {
                    Ok(draft) => {
                        let preview =
                            crate::protocol_codec_application::ProtocolCodecApplication::uri_from_draft(
                                &draft,
                            )
                            .ok();
                        crate::protocol_codec_application::ProtocolCodecApplication::publish_draft(
                            draft, preview,
                        );
                        Ok(())
                    }
                    Err(failure) => {
                        crate::protocol_codec_application::ProtocolCodecApplication::publish_error(
                            failure.message.clone(),
                        );
                        Err(failure)
                    }
                }
            }
            CommandIntent::SaveCustomNodeDraft { draft } => {
                // DUAL-05-14: splice the node into the active profile without
                // disturbing any other section, then commit through the same
                // apply transaction the other profile editors use.
                let trust = draft.params.tls_trust.clone();
                self.profile()?
                    .save_current_profile_content(
                        self.managed_runtime.clone(),
                        infiltrator_domain::apply::ApplyStrategy::PreferReload,
                        move |content| {
                            crate::protocol_codec_application::ProtocolCodecApplication::upsert_draft_into_profile(
                                content, &draft,
                            )
                            .map(|commit| commit.profile_yaml)
                            .map_err(|failure| failure.message)
                        },
                    )
                    .await?;
                // DUAL-05-13: the CA request is resolved against the host
                // reader *after* the profile committed, so a reported load can
                // never describe a document that was not written.
                crate::protocol_codec_application::ProtocolCodecApplication::publish_ca_trust(
                    &trust,
                    self.certificate_authority.as_deref(),
                );
                Ok(())
            }
            CommandIntent::ScanDialerChains => {
                // DUAL-05-09/10: one shared analyzer over the active profile;
                // both surfaces render the published report.
                let content = self.profile()?.current_profile().await?;
                crate::protocol_codec_application::ProtocolCodecApplication::publish_dialer_report(
                    &content,
                )?;
                Ok(())
            }
            CommandIntent::UpdateCustomNodeDraftField { field, value } => {
                // DUAL-05-09/13: a typed, whitelisted draft edit; an unknown
                // field is refused by the shared application.
                crate::protocol_codec_application::ProtocolCodecApplication::update_draft_field(
                    &field, &value,
                )?;
                Ok(())
            }
            CommandIntent::ResolveCertificateAuthority { trust } => {
                // DUAL-05-13: resolve and publish; the outcome is a state, not
                // an error, because a host without a reader is expected.
                crate::protocol_codec_application::ProtocolCodecApplication::publish_ca_trust(
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
            CommandIntent::ClearDnsCache => match self.dns_cache.as_ref() {
                Some(dns_cache) => dns_cache.flush_all().await.map(|_| ()),
                None => self
                    .runtime()?
                    .flush_fakeip_cache()
                    .await
                    .map_err(Failure::from),
            },
            CommandIntent::ApplyDnsSettings { patch } => self
                .configuration()?
                .apply_dns_settings_with_runtime(self.managed_runtime.clone(), patch)
                .await
                .map(|_| ()),
            CommandIntent::RunDoctorDiagnostics => self.doctor()?.run(None).await.map(|_| ()),
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
            CommandIntent::CreateBackupSnapshot => {
                self.snapshots()?.create_current().await.map(|_| ())
            }
            CommandIntent::RestoreSnapshot { id } => {
                let profile = self.profile()?.current_profile().await?;
                let path = std::path::PathBuf::from(id);
                self.snapshots()?
                    .restore(self.managed_runtime.clone(), &profile, &path)
                    .await
            }
            CommandIntent::LoadSnapshotDiff { snapshot_id } => {
                let profile = self.profile()?.current_profile().await?;
                let snapshots = self.snapshots()?;
                match snapshot_id {
                    Some(id) => snapshots
                        .diff_snapshot(&profile, std::path::Path::new(&id))
                        .await
                        .map(|_| ()),
                    None => snapshots.diff_newest(&profile).await.map(|_| ()),
                }
            }
            // DUAL-09-06/07: refresh the shared snapshot history (entries plus
            // the shared prune view) for the active profile.
            CommandIntent::LoadSnapshotHistory => {
                let profile = self.profile()?.current_profile().await?;
                self.snapshots()?
                    .history(
                        &profile,
                        infiltrator_contract::snapshot_history::SNAPSHOT_DEFAULT_KEEP,
                    )
                    .await
                    .map(|_| ())
            }
            CommandIntent::PruneSnapshots { keep } => {
                let profile = self.profile()?.current_profile().await?;
                let keep = keep
                    .map(
                        infiltrator_contract::snapshot_history::SnapshotHistorySnapshot::clamp_keep,
                    )
                    .unwrap_or(infiltrator_contract::snapshot_history::SNAPSHOT_DEFAULT_KEEP);
                self.snapshots()?
                    .prune(
                        &profile,
                        keep,
                        infiltrator_contract::snapshot_history::SnapshotPruneSource::Manual,
                    )
                    .await
                    .map(|_| ())
            }
            // DUAL-09-03/14: the editor surfaces load/commit the stored profile
            // document through the shared read model and guarded write path.
            CommandIntent::LoadProfileDocument { profile } => self
                .profile_document()?
                .load(profile.as_deref())
                .await
                .map(|_| ()),
            CommandIntent::SaveProfileDocument {
                profile,
                content,
                allow_protected,
            } => {
                let runtime = self.managed_runtime.clone();
                self.profile_document()?
                    .save(runtime, &profile, &content, allow_protected)
                    .await
                    .map(|_| ())
            }
            // DUAL-09-14: the editor panes load/commit the option sidecar
            // through the same use-case the Iced Mixin/Filter panes call.
            CommandIntent::LoadProfileOptions { profile } => self
                .profile_options()?
                .load(profile.as_deref())
                .await
                .map(|_| ()),
            CommandIntent::SaveMixinOverlay {
                profile,
                mixin_yaml,
            } => {
                let runtime = self.managed_runtime.clone();
                self.profile_options()?
                    .save_mixin(runtime, &profile, &mixin_yaml)
                    .await
                    .map(|_| ())?;
                // The composed document changed: republish it so the editor's
                // YAML pane renders the merged bytes, not the stale pre-mixin
                // content (the Iced editor reloads through the same read model).
                self.profile_document()?
                    .load(Some(&profile))
                    .await
                    .map(|_| ())
            }
            CommandIntent::RollbackCore => self.versions()?.rollback().await.map(|_| ()),
            CommandIntent::PrepareServiceMode => self.service_mode()?.prepare().await.map(|_| ()),
            CommandIntent::RepairPortConflicts => self.port_conflicts()?.repair().await.map(|_| ()),
            CommandIntent::CheckUpdates => {
                let settings = self.settings()?.load().await?;
                let channel = parse_release_channel(&settings.core_channel)?;
                self.versions()?.latest(channel).await.map(|_| ())
            }
            CommandIntent::UpdateSetting { key, value } => self.update_setting(&key, &value).await,
            CommandIntent::SetProxyMode { mode } => self
                .runtime()?
                .set_proxy_mode(mode)
                .await
                .map_err(Failure::from),
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
                .apply(infiltrator_contract::pac::PacRequest {
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
                    infiltrator_contract::network_roaming::NetworkRoamingStatus::Failed {
                        failure,
                    } => Err(failure),
                    infiltrator_contract::network_roaming::NetworkRoamingStatus::Unsupported {
                        reason,
                    } => Err(Failure::unsupported(reason)),
                    _ => Ok(()),
                }
            }
            CommandIntent::RepairNetworkRoutes => {
                self.network_roaming()?.force_repair().await.map(|_| ())
            }
            CommandIntent::StartVpn => {
                let snapshot = self.vpn()?.request_start().await?;
                match snapshot.state {
                    infiltrator_contract::vpn::VpnSessionState::Unsupported { reason } => {
                        Err(Failure::unsupported(reason))
                    }
                    infiltrator_contract::vpn::VpnSessionState::Failed { failure } => Err(failure),
                    _ => Ok(()),
                }
            }
            CommandIntent::StopVpn => self.vpn()?.stop().await.map(|_| ()),
            CommandIntent::ResetRuleHitCounters => {
                self.rule_tracer()?.clear_hits();
                Ok(())
            }
            CommandIntent::SimulateRuleTrace { query } => {
                self.rule_tracer()?.set_query(&query);
                Ok(())
            }
            CommandIntent::SetRuleTracerContext { src_ip } => {
                self.rule_tracer()?.set_context(&TrafficContextSnapshot {
                    src_ip,
                    ..TrafficContextSnapshot::default()
                });
                Ok(())
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
            CommandIntent::MoveRule { index, direction } => {
                self.edit_rules(|rules| edit::move_rule(rules, index, direction))
                    .await
            }
            CommandIntent::AddCustomRule {
                rule_type,
                payload,
                target,
            } => {
                let draft = infiltrator_contract::rule_edit::RuleDraft {
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
                .run(infiltrator_contract::privileged_network::PrivilegedNetworkRequest::standard())
                .await
                .map(|_| ()),
            CommandIntent::TestDnsLatency => self.test_dns_latency().await,
            CommandIntent::TestDnsLeak => self.test_dns_leak().await,
            CommandIntent::RunStunProbe => self.run_stun_probe().await,
            CommandIntent::RefreshPublicIpProbe
            | CommandIntent::ReorderOverviewCards { .. }
            | CommandIntent::ResetOverviewCardOrder
            | CommandIntent::StartCore
            | CommandIntent::StopCore
            | CommandIntent::RestartCore
            | CommandIntent::ClearLogs
            | CommandIntent::SetLogLevelFilter { .. }
            | CommandIntent::ToggleIncludeSystemApps { .. }
            | CommandIntent::ResolveConflictKeepLocal
            | CommandIntent::ResolveConflictTakeRemote => Err(unsupported()),
        }
    }
}
