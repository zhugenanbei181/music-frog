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
