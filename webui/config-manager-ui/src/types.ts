export interface ProfileInfo {
  name: string;
  active: boolean;
  path: string;
  controller_url?: string | null;
  controller_changed?: boolean | null;
  subscription_url?: string | null;
  auto_update_enabled?: boolean | null;
  update_interval_hours?: number | null;
  last_updated?: string | null;
  next_update?: string | null;
}

export interface ProfileDetail {
  name: string;
  active: boolean;
  path: string;
  content: string;
  subscription_url?: string | null;
  auto_update_enabled?: boolean | null;
  update_interval_hours?: number | null;
  last_updated?: string | null;
  next_update?: string | null;
}

export interface ProfileActionResponse {
  profile: ProfileInfo;
  rebuild_scheduled: boolean;
}

export interface CoreVersionsResponse {
  current: string | null;
  versions: string[];
}

export interface CoreLatestStableResponse {
  version: string;
  release_date: string;
}

export interface CoreDownloadResponse {
  version: string;
  downloaded: boolean;
  already_installed: boolean;
}

export interface CoreUpdateStableResponse {
  version: string;
  downloaded: boolean;
  already_installed: boolean;
  rebuild_scheduled: boolean;
}

export interface RebuildStatusResponse {
  in_progress: boolean;
  last_error?: string | null;
  last_reason?: string | null;
}

export interface WebDavConfig {
  enabled: boolean;
  url: string;
  username: string;
  password: string;
  sync_interval_mins: number;
  sync_on_startup: boolean;
}

export interface SyncResult {
  success_count: number;
  failed_count: number;
  total_actions: number;
}

export interface AppSettings {
  open_webui_on_startup: boolean;
  editor_path: string | null;
  use_bundled_core: boolean;
  language: string;
  theme?: string;
  webdav: WebDavConfig;
  autostart_enabled?: boolean | null;
  system_proxy_enabled?: boolean | null;
  runtime_running?: boolean | null;
}

export interface RuntimeCapabilitySet {
  status: boolean;
  lifecycle: boolean;
}

export interface ProxyCapabilitySet {
  list: boolean;
  mode_switch: boolean;
  select: boolean;
}

export interface SettingsCapabilitySet {
  autostart: boolean;
  system_proxy: boolean;
}

export interface AdminCapabilities {
  schema_version: number;
  platform: string;
  runtime: RuntimeCapabilitySet;
  proxy: ProxyCapabilitySet;
  settings: SettingsCapabilitySet;
}

export interface DnsFallbackFilter {
  geoip?: boolean;
  geoip_code?: string;
  ipcidr?: string[];
  domain?: string[];
  domain_suffix?: string[];
}

export interface DnsConfig {
  enable?: boolean;
  ipv6?: boolean;
  listen?: string;
  default_nameserver?: string[];
  nameserver?: string[];
  fallback?: string[];
  fallback_filter?: DnsFallbackFilter;
  enhanced_mode?: string;
  fake_ip_range?: string;
  fake_ip_filter?: string[];
  use_hosts?: boolean;
  use_system_hosts?: boolean;
  respect_rules?: boolean;
  proxy_server_nameserver?: string[];
  direct_nameserver?: string[];
  cache?: boolean;
}

export interface FakeIpConfig {
  fake_ip_range?: string;
  fake_ip_filter?: string[];
  store_fake_ip?: boolean;
}

export interface RuleEntry {
  rule: string;
  enabled: boolean;
}

export interface RuleProvidersPayload {
  providers: Record<string, RuleProvider>;
}

export interface ProxyProvidersPayload {
  providers: Record<string, Record<string, unknown>>;
}

export type SnifferConfig = Record<string, unknown>;

export interface RulesPayload {
  rules: RuleEntry[];
}

export interface RuleProvider {
  type: string;
  behavior?: string;
  path?: string;
  url?: string;
  interval?: number;
  format?: string;
}

export interface TunConfig {
  enable?: boolean;
  stack?: string;
  dns_hijack?: string[];
  auto_route?: boolean;
  auto_detect_interface?: boolean;
  mtu?: number;
  strict_route?: boolean;
}

export interface CacheFlushResponse {
  removed: boolean;
}

export interface AdminEvent {
  kind: string;
  detail?: string | null;
  timestamp?: number;
}

export type RuntimeLogLevel = 'debug' | 'info' | 'warning' | 'error' | 'silent';

export interface RuntimeConnectionMetadata {
  network?: string;
  type?: string;
  sourceIP?: string;
  destinationIP?: string;
  sourcePort?: string;
  destinationPort?: string;
  host?: string;
  dnsMode?: string;
  processPath?: string;
  specialProxy?: string;
}

export interface RuntimeConnection {
  id: string;
  metadata?: RuntimeConnectionMetadata;
  upload?: number;
  download?: number;
  start?: string;
  chains?: string[];
  rule?: string;
  rulePayload?: string;
}

export interface RuntimeConnectionsResponse {
  downloadTotal: number;
  uploadTotal: number;
  connections: RuntimeConnection[];
}

export interface RuntimeTrafficSnapshot {
  up_rate: number;
  down_rate: number;
  up_total: number;
  down_total: number;
  up_peak: number;
  down_peak: number;
  connections: number;
}

export interface RuntimeMemoryData {
  inuse: number;
  oslimit: number;
}

export interface RuntimeIpCheckResponse {
  ip: string;
  country?: string | null;
  region?: string | null;
  city?: string | null;
}

export interface RuntimeProxyDelayNode {
  name: string;
  proxy_type: string;
  delay_ms?: number | null;
  tested_at?: string | null;
}

export interface RuntimeProxyDelayNodesResponse {
  nodes: RuntimeProxyDelayNode[];
  default_test_url: string;
  default_timeout_ms: number;
}

export interface RuntimeDelayTestPayload {
  proxy: string;
  test_url?: string;
  timeout_ms?: number;
}

export interface RuntimeDelayBatchPayload {
  proxies?: string[];
  test_url?: string;
  timeout_ms?: number;
}

export interface RuntimeDelayTestResponse {
  proxy: string;
  delay_ms: number;
  tested_at: string;
  test_url: string;
  timeout_ms: number;
}

export interface RuntimeDelayBatchResult {
  proxy: string;
  delay_ms?: number | null;
  tested_at?: string | null;
  error?: string | null;
}

export interface RuntimeDelayBatchResponse {
  results: RuntimeDelayBatchResult[];
  success_count: number;
  failed_count: number;
  test_url: string;
  timeout_ms: number;
}

export interface RuntimeProxyGroupEntry {
  name: string;
  proxy_type: string;
  current?: string | null;
  all: string[];
}

export interface RuntimeProxiesResponse {
  mode: string;
  groups: RuntimeProxyGroupEntry[];
}

export interface RuntimeStatusResponse {
  running: boolean;
  controller?: string | null;
  mode?: string | null;
}
