import type {
  AppSettings,
  CoreVersionsResponse,
  ProfileActionResponse,
  ProfileDetail,
  ProfileInfo,
  RebuildStatusResponse,
  SyncResult,
  WebDavConfig,
} from './types';

const API_BASE = `${window.location.origin}/admin/api`;

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
  getAppSettings: () => request<AppSettings>('settings'),
  saveAppSettings: (settings: Partial<AppSettings>) =>
    request<void>('settings', { method: 'POST', body: settings }),
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
  activateCoreVersion: (version: string) =>
    request<void>('core/activate', { method: 'POST', body: { version } }),
  getRebuildStatus: () => request<RebuildStatusResponse>('rebuild/status', { timeoutMs: 10000 }),
};
