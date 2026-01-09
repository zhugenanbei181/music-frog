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
  webdav: WebDavConfig;
}
