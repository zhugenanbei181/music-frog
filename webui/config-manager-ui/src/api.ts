import type {
  AdminCapabilities,
  AppSettings,
  CacheFlushResponse,
  CoreDownloadResponse,
  CoreLatestStableResponse,
  CoreUpdateStableResponse,
  CoreVersionsResponse,
  DnsConfig,
  FakeIpConfig,
  ProfileActionResponse,
  ProfileDetail,
  ProfileInfo,
  ProxyProvidersPayload,
  RebuildStatusResponse,
  RuntimeConnectionsResponse,
  RuntimeDelayBatchPayload,
  RuntimeDelayBatchResponse,
  RuntimeDelayTestPayload,
  RuntimeDelayTestResponse,
  RuntimeIpCheckResponse,
  RuntimeLogLevel,
  RuntimeMemoryData,
  RuntimeProxyDelayNodesResponse,
  RuntimeProxiesResponse,
  RuntimeStatusResponse,
  RuntimeTrafficSnapshot,
  RuleProvidersPayload,
  RulesPayload,
  SnifferConfig,
  SyncResult,
  TunConfig,
  WebDavConfig,
} from './types';

const API_BASE = `${window.location.origin}/admin/api`;
export const adminEventsUrl = `${API_BASE}/events`;
export const runtimeLogsUrl = (level?: RuntimeLogLevel) =>
  `${API_BASE}/runtime/logs${level ? `?level=${encodeURIComponent(level)}` : ''}`;

type RequestOptions = {
  method?: string;
  body?: unknown;
  timeoutMs?: number;
};

const DEFAULT_TIMEOUT_MS = 30000;

export async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const { method = 'GET', body, timeoutMs = DEFAULT_TIMEOUT_MS } = options;
  const headers: Record<string, string> = {};
  let payload: BodyInit | undefined;
  if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
    payload = JSON.stringify(body);
  }
  const controller = new AbortController();
  const timeoutId = window.setTimeout(() => controller.abort(), timeoutMs);
  let response: Response;
  try {
    response = await fetch(`${API_BASE}/${path}`, {
      method,
      headers,
      body: payload,
      signal: controller.signal,
    });
  } catch (err) {
    if ((err as Error).name === 'AbortError') {
      throw new Error(`请求超时（${Math.ceil(timeoutMs / 1000)}s）`);
    }
    throw err;
  } finally {
    window.clearTimeout(timeoutId);
  }

  const contentType = response.headers.get('content-type') || '';
  let data: unknown = null;
  if (contentType.includes('application/json')) {
    data = await response.json();
  } else if (!response.ok) {
    data = await response.text();
  }

  if (!response.ok) {
    const message = (data as { error?: string })?.error || data || response.statusText;
    throw new Error(String(message));
  }
  return data as T;
}

export const api = {
  getCapabilities: () => request<AdminCapabilities>('capabilities'),
  getAppSettings: () => request<AppSettings>('settings'),
  saveAppSettings: (settings: Partial<AppSettings>) =>
    request<void>('settings', { method: 'POST', body: settings }),
  getProxies: () => request<RuntimeProxiesResponse>('proxies'),
  setProxyMode: (mode: string) =>
    request<void>('proxy/mode', { method: 'POST', body: { mode } }),
  selectProxy: (group: string, name: string) =>
    request<void>('proxy/select', { method: 'POST', body: { group, name } }),
  syncWebDavNow: () => request<SyncResult>('webdav/sync', { method: 'POST' }),
  testWebDav: (config: WebDavConfig) =>
    request<void>('webdav/test', { method: 'POST', body: config }),
  listProfiles: () => request<ProfileInfo[]>('profiles'),
  getProfile: (name: string) => request<ProfileDetail>(`profiles/${encodeURIComponent(name)}`),
  switchProfile: (name: string) =>
    request<ProfileActionResponse>('profiles/switch', { method: 'POST', body: { name } }),
  importProfile: (name: string, url: string, activate: boolean) =>
    request<ProfileActionResponse>('profiles/import', {
      method: 'POST',
      body: { name, url, activate },
      timeoutMs: 120000,
    }),
  saveProfile: (name: string, content: string, activate: boolean) =>
    request<ProfileActionResponse>('profiles/save', {
      method: 'POST',
      body: { name, content, activate },
    }),
  deleteProfile: (name: string) =>
    request<void>(`profiles/${encodeURIComponent(name)}`, { method: 'DELETE' }),
  setProfileSubscription: (
    name: string,
    payload: { url: string; auto_update_enabled: boolean; update_interval_hours?: number | null },
  ) =>
    request<ProfileInfo>(`profiles/${encodeURIComponent(name)}/subscription`, {
      method: 'POST',
      body: payload,
    }),
  clearProfileSubscription: (name: string) =>
    request<ProfileInfo>(`profiles/${encodeURIComponent(name)}/subscription`, { method: 'DELETE' }),
  updateProfileNow: (name: string) =>
    request<ProfileActionResponse>(`profiles/${encodeURIComponent(name)}/update-now`, {
      method: 'POST',
    }),
  clearProfiles: () =>
    request<ProfileActionResponse>('profiles/clear', { method: 'POST' }),
  openProfile: (name: string) =>
    request<void>('profiles/open', { method: 'POST', body: { name } }),
  getEditor: () => request<{ editor?: string | null }>('editor'),
  setEditor: (editor?: string | null) =>
    request<void>('editor', { method: 'POST', body: { editor } }),
  pickEditor: () =>
    request<{ editor?: string | null }>('editor/pick', { method: 'POST', timeoutMs: 120000 }),
  listCoreVersions: () => request<CoreVersionsResponse>('core/versions'),
  getLatestStableCore: () => request<CoreLatestStableResponse>('core/latest-stable'),
  downloadCoreVersion: (version: string) =>
    request<CoreDownloadResponse>('core/download', {
      method: 'POST',
      body: { version },
      timeoutMs: 600000,
    }),
  updateStableCore: () =>
    request<CoreUpdateStableResponse>('core/update-stable', {
      method: 'POST',
      timeoutMs: 600000,
    }),
  activateCoreVersion: (version: string) =>
    request<void>('core/activate', { method: 'POST', body: { version } }),
  getRebuildStatus: () => request<RebuildStatusResponse>('rebuild/status', { timeoutMs: 10000 }),
  startRuntime: () => request<RuntimeStatusResponse>('runtime/start', { method: 'POST' }),
  stopRuntime: () => request<RuntimeStatusResponse>('runtime/stop', { method: 'POST' }),
  getRuntimeStatus: () => request<RuntimeStatusResponse>('runtime/status'),
  listRuntimeConnections: () => request<RuntimeConnectionsResponse>('runtime/connections'),
  closeAllRuntimeConnections: () =>
    request<void>('runtime/connections', { method: 'DELETE' }),
  closeRuntimeConnection: (id: string) =>
    request<void>(`runtime/connections/${encodeURIComponent(id)}`, { method: 'DELETE' }),
  getRuntimeTraffic: () => request<RuntimeTrafficSnapshot>('runtime/traffic'),
  getRuntimeMemory: () => request<RuntimeMemoryData>('runtime/memory'),
  getRuntimeIp: () => request<RuntimeIpCheckResponse>('runtime/ip'),
  listRuntimeProxyDelayNodes: () =>
    request<RuntimeProxyDelayNodesResponse>('runtime/proxies'),
  testRuntimeProxyDelay: (payload: RuntimeDelayTestPayload) =>
    request<RuntimeDelayTestResponse>('runtime/delay/test', {
      method: 'POST',
      body: payload,
      timeoutMs: 90000,
    }),
  testAllRuntimeProxyDelays: (payload?: RuntimeDelayBatchPayload) =>
    request<RuntimeDelayBatchResponse>('runtime/delay/test-all', {
      method: 'POST',
      body: payload || {},
      timeoutMs: 180000,
    }),
  getDnsConfig: () => request<DnsConfig>('dns'),
  saveDnsConfig: (config: DnsConfig) =>
    request<DnsConfig>('dns', { method: 'POST', body: config }),
  getFakeIpConfig: () => request<FakeIpConfig>('fake-ip'),
  saveFakeIpConfig: (config: FakeIpConfig) =>
    request<FakeIpConfig>('fake-ip', { method: 'POST', body: config }),
  flushFakeIpCache: () => request<CacheFlushResponse>('fake-ip/flush', { method: 'POST' }),
  getRuleProviders: () => request<RuleProvidersPayload>('rule-providers'),
  saveRuleProviders: (payload: RuleProvidersPayload) =>
    request<RuleProvidersPayload>('rule-providers', { method: 'POST', body: payload }),
  getProxyProviders: () => request<ProxyProvidersPayload>('proxy-providers'),
  saveProxyProviders: (payload: ProxyProvidersPayload) =>
    request<ProxyProvidersPayload>('proxy-providers', { method: 'POST', body: payload }),
  getSnifferConfig: () => request<SnifferConfig>('sniffer'),
  saveSnifferConfig: (payload: SnifferConfig) =>
    request<SnifferConfig>('sniffer', { method: 'POST', body: payload }),
  getRules: () => request<RulesPayload>('rules'),
  saveRules: (payload: RulesPayload) =>
    request<RulesPayload>('rules', { method: 'POST', body: payload }),
  getTunConfig: () => request<TunConfig>('tun'),
  saveTunConfig: (config: TunConfig) =>
    request<TunConfig>('tun', { method: 'POST', body: config }),
};
