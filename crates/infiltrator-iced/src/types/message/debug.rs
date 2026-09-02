use super::Message;

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Noop => write!(f, "Noop"),
            Message::Navigate(route) => write!(f, "Navigate({:?})", route),
            Message::StartProxy => write!(f, "StartProxy"),
            Message::StopProxy => write!(f, "StopProxy"),
            Message::ProxyStarted(Ok(_), token) => write!(f, "ProxyStarted(Ok, token={token})"),
            Message::ProxyStarted(Err(e), token) => {
                write!(f, "ProxyStarted(Err({:?}), token={token})", e)
            }
            Message::ProxyStopped => write!(f, "ProxyStopped"),
            Message::SettingsLoaded(Ok(_)) => write!(f, "SettingsLoaded(Ok)"),
            Message::SettingsLoaded(Err(e)) => write!(f, "SettingsLoaded(Err({:?}))", e),
            Message::LoadProfiles => write!(f, "LoadProfiles"),
            Message::ProfilesLoaded(Ok(p)) => write!(f, "ProfilesLoaded(Ok({} profiles))", p.len()),
            Message::ProfilesLoaded(Err(e)) => write!(f, "ProfilesLoaded(Err({:?}))", e),
            Message::SetActiveProfile(name) => write!(f, "SetActiveProfile({})", name),
            Message::ProfileActivationFinished(Ok(reloaded)) => {
                write!(f, "ProfileActivationFinished(Ok(reloaded={}))", reloaded)
            }
            Message::ProfileActivationFinished(Err(error)) => {
                write!(f, "ProfileActivationFinished(Err({:?}))", error)
            }
            Message::UpdateImportUrl(url) => write!(f, "UpdateImportUrl({})", url),
            Message::UpdateImportName(name) => write!(f, "UpdateImportName({})", name),
            Message::UpdateImportActivate(enabled) => {
                write!(f, "UpdateImportActivate({})", enabled)
            }
            Message::ImportProfile => write!(f, "ImportProfile"),
            Message::ProfileImported(Ok(reloaded)) => {
                write!(f, "ProfileImported(Ok(reloaded={}))", reloaded)
            }
            Message::ProfileImported(Err(e)) => write!(f, "ProfileImported(Err({:?}))", e),
            Message::DeleteProfile(name) => write!(f, "DeleteProfile({})", name),
            Message::ProfileDeleted(Ok(_)) => write!(f, "ProfileDeleted(Ok)"),
            Message::ProfileDeleted(Err(e)) => write!(f, "ProfileDeleted(Err({:?}))", e),
            Message::UpdateLocalImportPath(path) => write!(f, "UpdateLocalImportPath({})", path),
            Message::BrowseLocalImportFile => write!(f, "BrowseLocalImportFile"),
            Message::LocalImportFilePicked(Some(path)) => {
                write!(f, "LocalImportFilePicked(Some({:?}))", path)
            }
            Message::LocalImportFilePicked(None) => write!(f, "LocalImportFilePicked(None)"),
            Message::UpdateLocalImportName(name) => write!(f, "UpdateLocalImportName({})", name),
            Message::UpdateLocalImportActivate(enabled) => {
                write!(f, "UpdateLocalImportActivate({})", enabled)
            }
            Message::ImportLocalProfile => write!(f, "ImportLocalProfile"),
            Message::LocalProfileImported(Ok(reloaded)) => {
                write!(f, "LocalProfileImported(Ok(reloaded={}))", reloaded)
            }
            Message::LocalProfileImported(Err(e)) => {
                write!(f, "LocalProfileImported(Err({:?}))", e)
            }
            Message::SelectSubscriptionProfile(name) => {
                write!(f, "SelectSubscriptionProfile({})", name)
            }
            Message::UpdateSubscriptionUrl(url) => write!(f, "UpdateSubscriptionUrl({})", url),
            Message::UpdateSubscriptionAutoUpdate(enabled) => {
                write!(f, "UpdateSubscriptionAutoUpdate({})", enabled)
            }
            Message::UpdateSubscriptionInterval(v) => {
                write!(f, "UpdateSubscriptionInterval({})", v)
            }
            Message::UpdateSubscriptionUserAgent(v) => {
                write!(f, "UpdateSubscriptionUserAgent({})", v)
            }
            Message::SaveSubscriptionSettings => write!(f, "SaveSubscriptionSettings"),
            Message::SubscriptionSettingsSaved(Ok(_)) => {
                write!(f, "SubscriptionSettingsSaved(Ok)")
            }
            Message::SubscriptionSettingsSaved(Err(e)) => {
                write!(f, "SubscriptionSettingsSaved(Err({:?}))", e)
            }
            Message::UpdateSubscriptionNow => write!(f, "UpdateSubscriptionNow"),
            Message::SubscriptionUpdatedNow(Ok(reloaded)) => {
                write!(f, "SubscriptionUpdatedNow(Ok(reloaded={}))", reloaded)
            }
            Message::SubscriptionUpdatedNow(Err(e)) => {
                write!(f, "SubscriptionUpdatedNow(Err({:?}))", e)
            }
            Message::SubscriptionAutoUpdated(Ok((names, active_updated))) => write!(
                f,
                "SubscriptionAutoUpdated(Ok({} profiles, active_updated={}))",
                names.len(),
                active_updated
            ),
            Message::SubscriptionAutoUpdated(Err(e)) => {
                write!(f, "SubscriptionAutoUpdated(Err({:?}))", e)
            }
            Message::UpdateAllSubscriptionsNow => write!(f, "UpdateAllSubscriptionsNow"),
            Message::AllSubscriptionsUpdated(Ok(results)) => write!(
                f,
                "AllSubscriptionsUpdated(Ok({} profiles, {} failed))",
                results.len(),
                results.iter().filter(|(_, r)| r.is_err()).count()
            ),
            Message::AllSubscriptionsUpdated(Err(e)) => {
                write!(f, "AllSubscriptionsUpdated(Err({:?}))", e)
            }
            Message::SetProfileAutoUpdate { name, enabled } => {
                write!(f, "SetProfileAutoUpdate({name}, {enabled})")
            }
            Message::ProfileAutoUpdateSet(Ok(name)) => {
                write!(f, "ProfileAutoUpdateSet(Ok({}))", name)
            }
            Message::ProfileAutoUpdateSet(Err(e)) => {
                write!(f, "ProfileAutoUpdateSet(Err({:?}))", e)
            }
            Message::UpdateProfilesFilter(s) => write!(f, "UpdateProfilesFilter({})", s),
            Message::ClearProfiles => write!(f, "ClearProfiles"),
            Message::ProfilesCleared(Ok(_)) => write!(f, "ProfilesCleared(Ok)"),
            Message::ProfilesCleared(Err(e)) => write!(f, "ProfilesCleared(Err({:?}))", e),
            Message::LoadProxies => write!(f, "LoadProxies"),
            Message::ProxiesLoaded(Ok(p)) => write!(f, "ProxiesLoaded(Ok({} proxies))", p.len()),
            Message::ProxiesLoaded(Err(e)) => write!(f, "ProxiesLoaded(Err({:?}))", e),
            Message::SelectProxy(g, n) => write!(f, "SelectProxy({}, {})", g, n),
            Message::FilterProxies(s) => write!(f, "FilterProxies({})", s),
            Message::ToggleFilterAlive(v) => write!(f, "ToggleFilterAlive({v})"),
            Message::ToggleFavoriteProxy(p) => write!(f, "ToggleFavoriteProxy({p})"),
            Message::ToggleProxyCompactView => write!(f, "ToggleProxyCompactView"),
            Message::InspectProxy(p) => write!(f, "InspectProxy({p:?})"),
            Message::OpenAddCustomNodeModal(b) => write!(f, "OpenAddCustomNodeModal({b})"),
            Message::UpdateNewNodeType(s) => write!(f, "UpdateNewNodeType({s})"),
            Message::UpdateNewNodeName(s) => write!(f, "UpdateNewNodeName({s})"),
            Message::UpdateNewNodeServer(s) => write!(f, "UpdateNewNodeServer({s})"),
            Message::UpdateNewNodePort(s) => write!(f, "UpdateNewNodePort({s})"),
            Message::UpdateNewNodeCredential(s) => write!(f, "UpdateNewNodeCredential({s})"),
            Message::UpdateNewNodeCipher(s) => write!(f, "UpdateNewNodeCipher({s})"),
            Message::UpdateNewNodeTls(b) => write!(f, "UpdateNewNodeTls({b})"),
            Message::SubmitAddCustomNode => write!(f, "SubmitAddCustomNode"),
            Message::CustomNodeAdded(Ok(_)) => write!(f, "CustomNodeAdded(Ok)"),
            Message::CustomNodeAdded(Err(e)) => write!(f, "CustomNodeAdded(Err({:?}))", e),
            Message::ToggleProxySort => write!(f, "ToggleProxySort"),
            Message::UpdateProxyDelaySort(s) => write!(f, "UpdateProxyDelaySort({})", s),
            Message::UpdateDelayTestUrl(s) => write!(f, "UpdateDelayTestUrl({})", s),
            Message::UpdateDelayTimeoutMs(s) => write!(f, "UpdateDelayTimeoutMs({})", s),
            Message::UpdateRuntimeSelectedGroup(s) => {
                write!(f, "UpdateRuntimeSelectedGroup({})", s)
            }
            Message::UpdateRuntimeSelectedProxy(s) => {
                write!(f, "UpdateRuntimeSelectedProxy({})", s)
            }
            Message::ApplyRuntimeSelectedProxy => write!(f, "ApplyRuntimeSelectedProxy"),
            Message::UpdateRuntimeConnectionFilter(s) => {
                write!(f, "UpdateRuntimeConnectionFilter({})", s)
            }
            Message::UpdateRuntimeConnectionSort(s) => {
                write!(f, "UpdateRuntimeConnectionSort({})", s)
            }
            Message::RefreshRuntimeNow => write!(f, "RefreshRuntimeNow"),
            Message::TrafficReceived(t) => {
                write!(f, "TrafficReceived(up: {}, down: {})", t.up, t.down)
            }
            Message::MemoryReceived(m) => write!(
                f,
                "MemoryReceived(in_use: {}, os_limit: {})",
                m.in_use, m.os_limit
            ),
            Message::IpInfoReceived(Ok(result), id) => {
                write!(
                    f,
                    "IpInfoReceived(Ok(ip={}, provider={}), taskId: {})",
                    result.ip, result.provider, id
                )
            }
            Message::IpInfoReceived(Err(e), id) => {
                write!(f, "IpInfoReceived(Err({:?}), taskId: {})", e, id)
            }
            Message::ConnectionsReceived(c) => write!(
                f,
                "ConnectionsReceived({} connections)",
                c.connections.len()
            ),
            Message::LogReceived(l) => write!(f, "LogReceived({})", l),
            Message::RuntimeStreamLogReceived(generation, _) => {
                write!(f, "RuntimeStreamLogReceived(generation={generation})")
            }
            Message::RuntimeStreamTrafficReceived(generation, data) => write!(
                f,
                "RuntimeStreamTrafficReceived(generation={}, up={}, down={})",
                generation, data.up, data.down
            ),
            Message::RuntimeStreamConnectionsReceived(generation, snapshot) => write!(
                f,
                "RuntimeStreamConnectionsReceived(generation={}, connections={})",
                generation,
                snapshot.connections.len()
            ),
            Message::RuntimeStreamStateChanged {
                kind,
                generation,
                state,
            } => write!(
                f,
                "RuntimeStreamStateChanged({kind:?}, generation={generation}, state={state:?})"
            ),
            Message::RuntimePollFailed(error) => write!(f, "RuntimePollFailed({error})"),
            Message::ClearRuntimeLogs => write!(f, "ClearRuntimeLogs"),
            Message::SetLogLevel(l) => write!(f, "SetLogLevel({})", l),
            Message::CloseConnection(id) => write!(f, "CloseConnection({})", id),
            Message::CloseAllConnections => write!(f, "CloseAllConnections"),
            Message::ConnectionsPrevPage => write!(f, "ConnectionsPrevPage"),
            Message::ConnectionsNextPage => write!(f, "ConnectionsNextPage"),
            Message::FetchRuntimeConfig => write!(f, "FetchRuntimeConfig"),
            Message::FetchIpInfo => write!(f, "FetchIpInfo"),
            Message::RuntimeConfigFetched(Ok(config), generation) => {
                write!(
                    f,
                    "RuntimeConfigFetched(gen={}, {}, {}, {} DNS, {} FB, {}, {}, {}, {}, {})",
                    generation,
                    config.mode,
                    config.tun_enabled,
                    config.dns_nameservers.len(),
                    config.dns_fallback.len(),
                    config.dns_enhanced_mode,
                    config.tun_stack,
                    config.tun_auto_route,
                    config.tun_strict_route,
                    config.sniffer_enabled
                )
            }
            Message::RuntimeConfigFetched(Err(e), generation) => {
                write!(
                    f,
                    "RuntimeConfigFetched(Err({:?}), generation={})",
                    e, generation
                )
            }
            Message::SetProxyMode(m) => write!(f, "SetProxyMode({})", m),
            Message::SetTunEnabled(t) => write!(f, "SetTunEnabled({})", t),
            Message::InstallTunService => write!(f, "InstallTunService"),
            Message::RefreshTunServiceStatus => write!(f, "RefreshTunServiceStatus"),
            Message::TunServiceStatusLoaded(Ok(status)) => {
                write!(f, "TunServiceStatusLoaded(Ok({status:?}))")
            }
            Message::TunServiceStatusLoaded(Err(error)) => {
                write!(f, "TunServiceStatusLoaded(Err({:?}))", error)
            }
            Message::TunServiceInstalled(Ok(_)) => write!(f, "TunServiceInstalled(Ok)"),
            Message::TunServiceInstalled(Err(error)) => {
                write!(f, "TunServiceInstalled(Err({:?}))", error)
            }
            Message::SetTunStack(s) => write!(f, "SetTunStack({})", s),
            Message::SetTunAutoRoute(a) => write!(f, "SetTunAutoRoute({})", a),
            Message::SetTunStrictRoute(s) => write!(f, "SetTunStrictRoute({})", s),
            Message::SetSnifferEnabled(s) => write!(f, "SetSnifferEnabled({})", s),
            Message::ModeSetResult(Ok(_)) => write!(f, "ModeSetResult(Ok)"),
            Message::ModeSetResult(Err(e)) => write!(f, "ModeSetResult(Err({:?}))", e),
            Message::RuntimePatchResult(Ok(_), token, generation) => write!(
                f,
                "RuntimePatchResult(Ok, token={token}, generation={generation})"
            ),
            Message::RuntimePatchResult(Err(error), token, generation) => write!(
                f,
                "RuntimePatchResult(Err({:?}), token={token}, generation={generation})",
                error
            ),
            Message::OperationResult(Ok(_)) => write!(f, "OperationResult(Ok)"),
            Message::OperationResult(Err(e)) => write!(f, "OperationResult(Err({:?}))", e),
            Message::LoadRules => write!(f, "LoadRules"),
            Message::RulesBundleLoaded(Ok(bundle)) => write!(
                f,
                "RulesBundleLoaded(Ok({} rules, rp:{} chars, pp:{} chars, sn:{} chars))",
                bundle.rules.len(),
                bundle.rule_providers_json.len(),
                bundle.proxy_providers_json.len(),
                bundle.sniffer_json.len()
            ),
            Message::RulesBundleLoaded(Err(e)) => write!(f, "RulesBundleLoaded(Err({:?}))", e),
            Message::RulesLoaded(Ok(r)) => write!(f, "RulesLoaded(Ok({} rules))", r.len()),
            Message::RulesLoaded(Err(e)) => write!(f, "RulesLoaded(Err({:?}))", e),
            Message::SetRulesTab(tab) => write!(f, "SetRulesTab({:?})", tab),
            Message::SetRulesJsonTab(tab) => write!(f, "SetRulesJsonTab({:?})", tab),
            Message::ToggleRulesProvidersExpanded => write!(f, "ToggleRulesProvidersExpanded"),
            Message::RulesPrevPage => write!(f, "RulesPrevPage"),
            Message::RulesNextPage => write!(f, "RulesNextPage"),
            Message::RulesSetPage(page) => write!(f, "RulesSetPage({})", page),
            Message::EnsureRuleProvidersEditorLoaded => {
                write!(f, "EnsureRuleProvidersEditorLoaded")
            }
            Message::EnsureProxyProvidersEditorLoaded => {
                write!(f, "EnsureProxyProvidersEditorLoaded")
            }
            Message::EnsureSnifferEditorLoaded => write!(f, "EnsureSnifferEditorLoaded"),
            Message::ActivateRulesHeavyView => write!(f, "ActivateRulesHeavyView"),
            Message::RuleProvidersJsonLoaded(Ok(json)) => {
                write!(f, "RuleProvidersJsonLoaded(Ok({} chars))", json.len())
            }
            Message::RuleProvidersJsonLoaded(Err(e)) => {
                write!(f, "RuleProvidersJsonLoaded(Err({:?}))", e)
            }
            Message::ProxyProvidersJsonLoaded(Ok(json)) => {
                write!(f, "ProxyProvidersJsonLoaded(Ok({} chars))", json.len())
            }
            Message::ProxyProvidersJsonLoaded(Err(e)) => {
                write!(f, "ProxyProvidersJsonLoaded(Err({:?}))", e)
            }
            Message::SnifferJsonLoaded(Ok(json)) => {
                write!(f, "SnifferJsonLoaded(Ok({} chars))", json.len())
            }
            Message::SnifferJsonLoaded(Err(e)) => write!(f, "SnifferJsonLoaded(Err({:?}))", e),
            Message::LoadProviders => write!(f, "LoadProviders"),
            Message::ProvidersLoaded(Ok((p, r))) => write!(
                f,
                "ProvidersLoaded(Ok({} proxies, {} rules))",
                p.len(),
                r.len()
            ),
            Message::ProvidersLoaded(Err(e)) => write!(f, "ProvidersLoaded(Err({:?}))", e),
            Message::UpdateProxyProvider(p) => write!(f, "UpdateProxyProvider({})", p),
            Message::UpdateRuleProvider(p) => write!(f, "UpdateRuleProvider({})", p),
            Message::FilterRules(s) => write!(f, "FilterRules({})", s),
            Message::UpdateRulesTracerInput(s) => write!(f, "UpdateRulesTracerInput({s})"),
            Message::RunRulesTracer => write!(f, "RunRulesTracer"),
            Message::UpdateFilteredGroups => write!(f, "UpdateFilteredGroups"),
            Message::UpdateNewRuleType(s) => write!(f, "UpdateNewRuleType({})", s),
            Message::UpdateNewRulePayload(s) => write!(f, "UpdateNewRulePayload({})", s),
            Message::UpdateNewRuleTarget(s) => write!(f, "UpdateNewRuleTarget({})", s),
            Message::AddCustomRule => write!(f, "AddCustomRule"),
            Message::RuleAdded(Ok(_)) => write!(f, "RuleAdded(Ok)"),
            Message::RuleAdded(Err(e)) => write!(f, "RuleAdded(Err({:?}))", e),
            Message::ToggleRuleEnabled(index) => write!(f, "ToggleRuleEnabled({})", index),
            Message::MoveRuleUp(index) => write!(f, "MoveRuleUp({})", index),
            Message::MoveRuleDown(index) => write!(f, "MoveRuleDown({})", index),
            Message::SaveRules => write!(f, "SaveRules"),
            Message::ApplyGameRoutingPresets => write!(f, "ApplyGameRoutingPresets"),
            Message::UpdateGeoDatabases => write!(f, "UpdateGeoDatabases"),
            Message::GeoDatabasesUpdated(Ok(_)) => write!(f, "GeoDatabasesUpdated(Ok)"),
            Message::GeoDatabasesUpdated(Err(e)) => write!(f, "GeoDatabasesUpdated(Err({:?}))", e),
            Message::RulesSaved(Ok(_)) => write!(f, "RulesSaved(Ok)"),
            Message::RulesSaved(Err(e)) => write!(f, "RulesSaved(Err({:?}))", e),
            Message::InspectRuleProviderDiff(opt) => write!(f, "InspectRuleProviderDiff({opt:?})"),
            Message::UnpackRuleProvider(name) => write!(f, "UnpackRuleProvider({name})"),
            Message::RuleProviderDiffLoaded(Ok(diff)) => {
                write!(f, "RuleProviderDiffLoaded(Ok({}))", diff.provider_name)
            }
            Message::RuleProviderDiffLoaded(Err(e)) => {
                write!(f, "RuleProviderDiffLoaded(Err({e:?}))")
            }
            Message::RuleProvidersEditorAction(_) => write!(f, "RuleProvidersEditorAction"),
            Message::SaveRuleProvidersJson => write!(f, "SaveRuleProvidersJson"),
            Message::RuleProvidersJsonSaved(Ok(_)) => write!(f, "RuleProvidersJsonSaved(Ok)"),
            Message::RuleProvidersJsonSaved(Err(e)) => {
                write!(f, "RuleProvidersJsonSaved(Err({:?}))", e)
            }
            Message::ProxyProvidersEditorAction(_) => write!(f, "ProxyProvidersEditorAction"),
            Message::SaveProxyProvidersJson => write!(f, "SaveProxyProvidersJson"),
            Message::ProxyProvidersJsonSaved(Ok(_)) => write!(f, "ProxyProvidersJsonSaved(Ok)"),
            Message::ProxyProvidersJsonSaved(Err(e)) => {
                write!(f, "ProxyProvidersJsonSaved(Err({:?}))", e)
            }
            Message::SnifferEditorAction(_) => write!(f, "SnifferEditorAction"),
            Message::SaveSnifferJson => write!(f, "SaveSnifferJson"),
            Message::SnifferJsonSaved(Ok(_)) => write!(f, "SnifferJsonSaved(Ok)"),
            Message::SnifferJsonSaved(Err(e)) => write!(f, "SnifferJsonSaved(Err({:?}))", e),
            Message::LoadAdvancedConfigs => write!(f, "LoadAdvancedConfigs"),
            Message::AdvancedConfigsBundleLoaded(Ok(bundle)) => write!(
                f,
                "AdvancedConfigsBundleLoaded(Ok(dns:{} chars, fake:{} chars, tun:{} chars))",
                bundle.dns_json.len(),
                bundle.fake_ip_json.len(),
                bundle.tun_json.len()
            ),
            Message::AdvancedConfigsBundleLoaded(Err(e)) => {
                write!(f, "AdvancedConfigsBundleLoaded(Err({:?}))", e)
            }
            Message::SetDnsTab(tab) => write!(f, "SetDnsTab({:?})", tab),
            Message::SetAdvancedMode(tab, mode) => {
                write!(f, "SetAdvancedMode({:?}, {:?})", tab, mode)
            }
            Message::RefreshDnsOnly => write!(f, "RefreshDnsOnly"),
            Message::RefreshFakeIpOnly => write!(f, "RefreshFakeIpOnly"),
            Message::RefreshTunOnly => write!(f, "RefreshTunOnly"),
            Message::EnsureDnsEditorLoaded => write!(f, "EnsureDnsEditorLoaded"),
            Message::EnsureFakeIpEditorLoaded => write!(f, "EnsureFakeIpEditorLoaded"),
            Message::EnsureTunEditorLoaded => write!(f, "EnsureTunEditorLoaded"),
            Message::ActivateDnsHeavyView => write!(f, "ActivateDnsHeavyView"),
            Message::DnsConfigJsonLoaded(Ok(json)) => {
                write!(f, "DnsConfigJsonLoaded(Ok({} chars))", json.len())
            }
            Message::DnsConfigJsonLoaded(Err(e)) => {
                write!(f, "DnsConfigJsonLoaded(Err({:?}))", e)
            }
            Message::FakeIpConfigJsonLoaded(Ok(json)) => {
                write!(f, "FakeIpConfigJsonLoaded(Ok({} chars))", json.len())
            }
            Message::FakeIpConfigJsonLoaded(Err(e)) => {
                write!(f, "FakeIpConfigJsonLoaded(Err({:?}))", e)
            }
            Message::TunConfigJsonLoaded(Ok(json)) => {
                write!(f, "TunConfigJsonLoaded(Ok({} chars))", json.len())
            }
            Message::TunConfigJsonLoaded(Err(e)) => {
                write!(f, "TunConfigJsonLoaded(Err({:?}))", e)
            }
            Message::UpdateDnsFormEnable(v) => write!(f, "UpdateDnsFormEnable({})", v),
            Message::UpdateDnsFormNameserver(v) => write!(f, "UpdateDnsFormNameserver({})", v),
            Message::UpdateDnsFormFallback(v) => write!(f, "UpdateDnsFormFallback({})", v),
            Message::UpdateDnsFormEnhancedMode(v) => {
                write!(f, "UpdateDnsFormEnhancedMode({})", v)
            }
            Message::UpdateDnsFormFakeIpRange(v) => {
                write!(f, "UpdateDnsFormFakeIpRange({})", v)
            }
            Message::UpdateDnsFormFakeIpFilter(v) => {
                write!(f, "UpdateDnsFormFakeIpFilter({})", v)
            }
            Message::UpdateDnsFormIpv6(v) => write!(f, "UpdateDnsFormIpv6({})", v),
            Message::UpdateDnsFormCache(v) => write!(f, "UpdateDnsFormCache({})", v),
            Message::UpdateDnsFormUseHosts(v) => write!(f, "UpdateDnsFormUseHosts({})", v),
            Message::UpdateDnsFormUseSystemHosts(v) => {
                write!(f, "UpdateDnsFormUseSystemHosts({})", v)
            }
            Message::UpdateDnsFormRespectRules(v) => {
                write!(f, "UpdateDnsFormRespectRules({})", v)
            }
            Message::UpdateDnsFormProxyServerNameserver(v) => {
                write!(f, "UpdateDnsFormProxyServerNameserver({})", v)
            }
            Message::UpdateDnsFormDirectNameserver(v) => {
                write!(f, "UpdateDnsFormDirectNameserver({})", v)
            }
            Message::UpdateFakeIpFormRange(v) => write!(f, "UpdateFakeIpFormRange({})", v),
            Message::UpdateFakeIpFormFilter(v) => write!(f, "UpdateFakeIpFormFilter({})", v),
            Message::UpdateFakeIpFormStore(v) => write!(f, "UpdateFakeIpFormStore({})", v),
            Message::UpdateTunFormEnable(v) => write!(f, "UpdateTunFormEnable({})", v),
            Message::UpdateTunFormStack(v) => write!(f, "UpdateTunFormStack({})", v),
            Message::UpdateTunFormMtu(v) => write!(f, "UpdateTunFormMtu({})", v),
            Message::UpdateTunFormDnsHijack(v) => write!(f, "UpdateTunFormDnsHijack({})", v),
            Message::UpdateTunFormAutoRoute(v) => write!(f, "UpdateTunFormAutoRoute({})", v),
            Message::UpdateTunFormAutoDetectInterface(v) => {
                write!(f, "UpdateTunFormAutoDetectInterface({})", v)
            }
            Message::UpdateTunFormStrictRoute(v) => {
                write!(f, "UpdateTunFormStrictRoute({})", v)
            }
            Message::DnsConfigEditorAction(_) => write!(f, "DnsConfigEditorAction"),
            Message::FakeIpConfigEditorAction(_) => write!(f, "FakeIpConfigEditorAction"),
            Message::TunConfigEditorAction(_) => write!(f, "TunConfigEditorAction"),
            Message::TickSubUpdate => write!(f, "TickSubUpdate"),
            Message::TickWebDavSync => write!(f, "TickWebDavSync"),
            Message::TickRuntimeRefresh => write!(f, "TickRuntimeRefresh"),
            Message::TickFrame(now) => write!(f, "TickFrame({:?})", now),
            Message::TrayEvent(e) => write!(f, "TrayEvent({:?})", e),
            Message::Exit => write!(f, "Exit"),
            Message::UpdateDnsServer(i, s) => write!(f, "UpdateDnsServer({}, {})", i, s),
            Message::UpdateDnsEnhancedMode(m) => write!(f, "UpdateDnsEnhancedMode({})", m),
            Message::AddDnsServer => write!(f, "AddDnsServer"),
            Message::AddDnsServerTemplate(s) => write!(f, "AddDnsServerTemplate({})", s),
            Message::RemoveDnsServer(i) => write!(f, "RemoveDnsServer({})", i),
            Message::UpdateFallbackDnsServer(i, s) => {
                write!(f, "UpdateFallbackDnsServer({}, {})", i, s)
            }
            Message::AddFallbackDnsServer => write!(f, "AddFallbackDnsServer"),
            Message::RemoveFallbackDnsServer(i) => write!(f, "RemoveFallbackDnsServer({})", i),
            Message::SaveDns => write!(f, "SaveDns"),
            Message::DnsSaved(Ok(_)) => write!(f, "DnsSaved(Ok)"),
            Message::DnsSaved(Err(e)) => write!(f, "DnsSaved(Err({:?}))", e),
            Message::SaveFakeIpConfig => write!(f, "SaveFakeIpConfig"),
            Message::FakeIpConfigSaved(Ok(_)) => write!(f, "FakeIpConfigSaved(Ok)"),
            Message::FakeIpConfigSaved(Err(e)) => write!(f, "FakeIpConfigSaved(Err({:?}))", e),
            Message::SaveTunConfig => write!(f, "SaveTunConfig"),
            Message::TunConfigSaved(Ok(_)) => write!(f, "TunConfigSaved(Ok)"),
            Message::TunConfigSaved(Err(e)) => write!(f, "TunConfigSaved(Err({:?}))", e),
            Message::SetAutostart(b) => write!(f, "SetAutostart({})", b),
            Message::AutostartSet(Ok(_)) => write!(f, "AutostartSet(Ok)"),
            Message::AutostartSet(Err(e)) => write!(f, "AutostartSet(Err({:?}))", e),
            Message::UpdateNotificationsEnabled(b) => {
                write!(f, "UpdateNotificationsEnabled({})", b)
            }
            Message::UpdateCloseToTray(b) => write!(f, "UpdateCloseToTray({b})"),
            Message::UpdateWebDavEnabled(b) => write!(f, "UpdateWebDavEnabled({})", b),
            Message::UpdateWebDavUrl(s) => write!(f, "UpdateWebDavUrl({})", s),
            Message::UpdateWebDavUser(s) => write!(f, "UpdateWebDavUser({})", s),
            Message::UpdateWebDavPass(_) => write!(f, "UpdateWebDavPass(***)"),
            Message::UpdateWebDavSyncInterval(s) => {
                write!(f, "UpdateWebDavSyncInterval({})", s)
            }
            Message::UpdateWebDavSyncOnStartup(b) => {
                write!(f, "UpdateWebDavSyncOnStartup({})", b)
            }
            Message::SaveAppSettings => write!(f, "SaveAppSettings"),
            Message::AppSettingsSaved(Ok(_)) => write!(f, "AppSettingsSaved(Ok)"),
            Message::AppSettingsSaved(Err(e)) => write!(f, "AppSettingsSaved(Err({:?}))", e),
            Message::UpdateEditorPathSetting(s) => write!(f, "UpdateEditorPathSetting({})", s),
            Message::SetLanguage(language) => write!(f, "SetLanguage({})", language),
            Message::SetAdminEnabled(b) => write!(f, "SetAdminEnabled({})", b),
            Message::UpdateAdminPort(s) => write!(f, "UpdateAdminPort({})", s),
            Message::ApplyAdminSettings => write!(f, "ApplyAdminSettings"),
            Message::AdminSettingsSaved(Ok(_)) => write!(f, "AdminSettingsSaved(Ok)"),
            Message::AdminSettingsSaved(Err(e)) => write!(f, "AdminSettingsSaved(Err({:?}))", e),
            Message::AdminServerStarted(Ok(url)) => write!(f, "AdminServerStarted(Ok({}))", url),
            Message::AdminServerStarted(Err(e)) => write!(f, "AdminServerStarted(Err({:?}))", e),
            Message::AdminHostCommand(command) => {
                write!(f, "AdminHostCommand({:?})", command)
            }
            Message::ExternalSettingsLoaded(Ok(_)) => write!(f, "ExternalSettingsLoaded(Ok)"),
            Message::ExternalSettingsLoaded(Err(e)) => {
                write!(f, "ExternalSettingsLoaded(Err({:?}))", e)
            }
            Message::SyncUpload => write!(f, "SyncUpload"),
            Message::SyncDownload => write!(f, "SyncDownload"),
            Message::SyncFinished(Ok(summary)) => write!(
                f,
                "SyncFinished(Ok(uploaded={}, downloaded={}, conflicts={}, active_changed={}))",
                summary.uploaded,
                summary.downloaded,
                summary.conflicts,
                summary.active_profile_changed
            ),
            Message::SyncFinished(Err(e)) => write!(f, "SyncFinished(Err({:?}))", e),
            Message::SyncProgress(progress) => write!(
                f,
                "SyncProgress({}, {}/{})",
                progress.phase, progress.current, progress.total
            ),
            Message::ResolveSyncConflict(profile) => {
                write!(f, "ResolveSyncConflict({})", profile)
            }
            Message::DismissSyncConflict(profile) => {
                write!(f, "DismissSyncConflict({})", profile)
            }
            Message::SyncConflictResolved(Ok(profile)) => {
                write!(f, "SyncConflictResolved(Ok({}))", profile)
            }
            Message::SyncConflictResolved(Err(error)) => {
                write!(f, "SyncConflictResolved(Err({:?}))", error)
            }
            Message::SyncConflictDismissed(Ok(profile)) => {
                write!(f, "SyncConflictDismissed(Ok({}))", profile)
            }
            Message::SyncConflictDismissed(Err(error)) => {
                write!(f, "SyncConflictDismissed(Err({:?}))", error)
            }
            Message::CancelWebDavSync => write!(f, "CancelWebDavSync"),
            Message::TestWebDavConnection => write!(f, "TestWebDavConnection"),
            Message::WebDavConnectionTested(Ok(_)) => {
                write!(f, "WebDavConnectionTested(Ok)")
            }
            Message::WebDavConnectionTested(Err(error)) => {
                write!(f, "WebDavConnectionTested(Err({:?}))", error)
            }
            Message::SetSystemProxy(b) => write!(f, "SetSystemProxy({})", b),
            Message::UpdateSystemProxyBypass(s) => write!(f, "UpdateSystemProxyBypass({s})"),
            Message::SystemProxySet(Ok(_)) => write!(f, "SystemProxySet(Ok)"),
            Message::SystemProxySet(Err(e)) => write!(f, "SystemProxySet(Err({:?}))", e),
            Message::RequestAdminPrivilege => write!(f, "RequestAdminPrivilege"),
            Message::RequestConfirmation(action) => {
                write!(f, "RequestConfirmation({action:?})")
            }
            Message::ConfirmAction => write!(f, "ConfirmAction"),
            Message::CancelConfirmation => write!(f, "CancelConfirmation"),
            Message::ClearError => write!(f, "ClearError"),
            Message::EditProfile(p) => write!(f, "EditProfile({:?})", p),
            Message::ProfileContentLoaded(Ok((p, _))) => {
                write!(f, "ProfileContentLoaded(Ok({:?}))", p)
            }
            Message::ProfileContentLoaded(Err(e)) => {
                write!(f, "ProfileContentLoaded(Err({:?}))", e)
            }
            Message::LoadProfileSnapshots => write!(f, "LoadProfileSnapshots"),
            Message::ProfileSnapshotsLoaded(Ok(snapshots)) => {
                write!(
                    f,
                    "ProfileSnapshotsLoaded(Ok({} snapshots))",
                    snapshots.len()
                )
            }
            Message::ProfileSnapshotsLoaded(Err(error)) => {
                write!(f, "ProfileSnapshotsLoaded(Err({:?}))", error)
            }
            Message::RestoreProfileSnapshot(path) => {
                write!(f, "RestoreProfileSnapshot({:?})", path)
            }
            Message::ProfileSnapshotRestored(Ok(_)) => {
                write!(f, "ProfileSnapshotRestored(Ok)")
            }
            Message::ProfileSnapshotRestored(Err(error)) => {
                write!(f, "ProfileSnapshotRestored(Err({:?}))", error)
            }
            Message::EditorAction(_) => write!(f, "EditorAction"),
            Message::SaveProfile => write!(f, "SaveProfile"),
            Message::ProfileSaved(Ok(_)) => write!(f, "ProfileSaved(Ok)"),
            Message::ProfileSaved(Err(e)) => write!(f, "ProfileSaved(Err({:?}))", e),
            Message::OpenConfigDirFinished(Ok(_)) => {
                write!(f, "OpenConfigDirFinished(Ok)")
            }
            Message::OpenConfigDirFinished(Err(error)) => {
                write!(f, "OpenConfigDirFinished(Err({:?}))", error)
            }
            Message::LoadKernels => write!(f, "LoadKernels"),
            Message::KernelsLoaded(Ok(k)) => write!(f, "KernelsLoaded(Ok({} kernels))", k.len()),
            Message::KernelsLoaded(Err(e)) => write!(f, "KernelsLoaded(Err({:?}))", e),
            Message::CheckCoreUpdate => write!(f, "CheckCoreUpdate"),
            Message::CoreUpdateInfo(Ok(v)) => write!(f, "CoreUpdateInfo(Ok({}))", v),
            Message::CoreUpdateInfo(Err(e)) => write!(f, "CoreUpdateInfo(Err({:?}))", e),
            Message::SetCoreChannel(channel) => write!(f, "SetCoreChannel({})", channel),
            Message::DownloadCore(v) => write!(f, "DownloadCore({})", v),
            Message::CoreDownloadProgress(progress, token) => {
                write!(
                    f,
                    "CoreDownloadProgress({}/{:?}, {} B/s, token={})",
                    progress.downloaded, progress.total, progress.speed_bytes, token
                )
            }
            Message::CoreDownloadFinished(Ok(v), token) => {
                write!(f, "CoreDownloadFinished(Ok({}), token={})", v, token)
            }
            Message::CoreDownloadFinished(Err(e), token) => {
                write!(f, "CoreDownloadFinished(Err({:?}), token={})", e, token)
            }
            Message::CancelCoreDownload => write!(f, "CancelCoreDownload"),
            Message::DeleteKernel(v) => write!(f, "DeleteKernel({})", v),
            Message::SetDefaultKernel(v) => write!(f, "SetDefaultKernel({})", v),
            Message::KernelOperationFinished(Ok(_)) => {
                write!(f, "KernelOperationFinished(Ok)")
            }
            Message::KernelOperationFinished(Err(error)) => {
                write!(f, "KernelOperationFinished(Err({:?}))", error)
            }
            Message::FactoryReset => write!(f, "FactoryReset"),
            Message::FactoryResetFinished(Ok(_)) => write!(f, "FactoryResetFinished(Ok)"),
            Message::FactoryResetFinished(Err(error)) => {
                write!(f, "FactoryResetFinished(Err({:?}))", error)
            }
            Message::OpenConfigDir => write!(f, "OpenConfigDir"),
            Message::FlushFakeIpCache => write!(f, "FlushFakeIpCache"),
            Message::TestProxyDelay(p) => write!(f, "TestProxyDelay({})", p),
            Message::TestGroupDelay(g) => write!(f, "TestGroupDelay({})", g),
            Message::ProxyTested(p, Ok(d)) => write!(f, "ProxyTested({}, Ok({}ms))", p, d),
            Message::ProxyTested(p, Err(e)) => write!(f, "ProxyTested({}, Err({:?}))", p, e),
            Message::WindowClosed(id) => write!(f, "WindowClosed({:?})", id),
            Message::HideWindow => write!(f, "HideWindow"),
            Message::ShowWindow => write!(f, "ShowWindow"),
            Message::UpdateRuntimeAutoRefresh(v) => write!(f, "UpdateRuntimeAutoRefresh({})", v),
            Message::RuntimePanelSettingsSaved(Ok(_)) => {
                write!(f, "RuntimePanelSettingsSaved(Ok)")
            }
            Message::RuntimePanelSettingsSaved(Err(e)) => {
                write!(f, "RuntimePanelSettingsSaved(Err({:?}))", e)
            }
            Message::RuntimeRebuildFinished(Ok(_)) => write!(f, "RuntimeRebuildFinished(Ok)"),
            Message::RuntimeRebuildFinished(Err(e)) => {
                write!(f, "RuntimeRebuildFinished(Err({:?}))", e)
            }
            Message::ClearRebuildFlow => write!(f, "ClearRebuildFlow"),
            Message::TogglePerfPanel => write!(f, "TogglePerfPanel"),
            Message::ToggleTheme => write!(f, "ToggleTheme"),
            Message::SetTheme(t) => write!(f, "SetTheme({t})"),
            Message::ShowToast(s, st) => write!(f, "ShowToast({}, {:?})", s, st),
            Message::RemoveToast(i) => write!(f, "RemoveToast({})", i),
            Message::TestAllProxyDelays => write!(f, "TestAllProxyDelays"),
            Message::AllProxyDelaysTested(Ok((s, f_cnt))) => {
                write!(
                    f,
                    "AllProxyDelaysTested(Ok(success={}, failed={}))",
                    s, f_cnt
                )
            }
            Message::AllProxyDelaysTested(Err(e)) => {
                write!(f, "AllProxyDelaysTested(Err({:?}))", e)
            }
            // ui-wave2-p
            Message::ToggleProxyGroupExpanded(name) => {
                write!(f, "ToggleProxyGroupExpanded({})", name)
            }
            Message::SetEditorPane(pane) => write!(f, "SetEditorPane({:?})", pane),
            Message::EditProfileAs(path, pane) => {
                write!(f, "EditProfileAs({},{:?})", path.display(), pane)
            }
            Message::MixinEditorAction(_) => write!(f, "MixinEditorAction"),
            Message::MixinLoaded(Ok(_)) => write!(f, "MixinLoaded(Ok)"),
            Message::MixinLoaded(Err(e)) => write!(f, "MixinLoaded(Err({:?}))", e),
            Message::SaveMixin => write!(f, "SaveMixin"),
            Message::MixinSaved(Ok(_)) => write!(f, "MixinSaved(Ok)"),
            Message::MixinSaved(Err(e)) => write!(f, "MixinSaved(Err({:?}))", e),
            Message::LoadProfileFilter => write!(f, "LoadProfileFilter"),
            Message::ProfileFilterLoaded(Ok(_)) => write!(f, "ProfileFilterLoaded(Ok)"),
            Message::ProfileFilterLoaded(Err(e)) => write!(f, "ProfileFilterLoaded(Err({:?}))", e),
            Message::UpdateFilterInclude(v) => write!(f, "UpdateFilterInclude({})", v),
            Message::UpdateFilterExclude(v) => write!(f, "UpdateFilterExclude({})", v),
            Message::UpdateFilterExcludeTypes(v) => write!(f, "UpdateFilterExcludeTypes({})", v),
            Message::UpdateFilterRenames(v) => write!(f, "UpdateFilterRenames({})", v),
            Message::UpdateFilterDedup(i) => write!(f, "UpdateFilterDedup({})", i),
            Message::SaveProfileFilter => write!(f, "SaveProfileFilter"),
            Message::ProfileFilterSaved(Ok(report)) => {
                write!(f, "ProfileFilterSaved(Ok(passed={}))", report.passed)
            }
            Message::ProfileFilterSaved(Err(e)) => write!(f, "ProfileFilterSaved(Err({:?}))", e),
            Message::ScanMrsProviders => write!(f, "ScanMrsProviders"),
            Message::MrsDetailsReady(Ok(details)) => {
                write!(f, "MrsDetailsReady(Ok({} providers))", details.len())
            }
            Message::MrsDetailsReady(Err(e)) => write!(f, "MrsDetailsReady(Err({:?}))", e),
            Message::LoadSyncDiff(profile) => write!(f, "LoadSyncDiff({})", profile),
            Message::SyncDiffLoaded(Ok(bundle)) => {
                write!(f, "SyncDiffLoaded(Ok({}))", bundle.profile)
            }
            Message::SyncDiffLoaded(Err(e)) => write!(f, "SyncDiffLoaded(Err({:?}))", e),
            Message::PickSyncDiffKey(key, take_remote) => {
                write!(f, "PickSyncDiffKey({}, {})", key, take_remote)
            }
            Message::SetSyncDiffPicks(take_remote) => {
                write!(f, "SetSyncDiffPicks({})", take_remote)
            }
            Message::ApplySyncDiffMerge => write!(f, "ApplySyncDiffMerge"),
            Message::SyncDiffMerged(Ok(profile)) => write!(f, "SyncDiffMerged(Ok({}))", profile),
            Message::SyncDiffMerged(Err(e)) => write!(f, "SyncDiffMerged(Err({:?}))", e),
            Message::CloseSyncDiff => write!(f, "CloseSyncDiff"),
            // Doctor 体检面板
            Message::RunDoctor => write!(f, "RunDoctor"),
            Message::DoctorReportReady(Ok(report)) => {
                write!(f, "DoctorReportReady(Ok({} checks))", report.checks.len())
            }
            Message::DoctorReportReady(Err(e)) => write!(f, "DoctorReportReady(Err({:?}))", e),
            Message::RunDoctorFix => write!(f, "RunDoctorFix"),
            Message::DoctorFixApplied(Ok(report)) => {
                write!(f, "DoctorFixApplied(Ok({} actions))", report.actions.len())
            }
            Message::DoctorFixApplied(Err(e)) => write!(f, "DoctorFixApplied(Err({:?}))", e),
            Message::RunBootstrap => write!(f, "RunBootstrap"),
            Message::BootstrapFinished(Ok(report)) => {
                write!(f, "BootstrapFinished(Ok({} steps))", report.steps.len())
            }
            Message::BootstrapFinished(Err(e)) => write!(f, "BootstrapFinished(Err({:?}))", e),
        }
    }
}
