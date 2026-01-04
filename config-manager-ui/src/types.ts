export interface ProfileInfo {
  name: string;
  active: boolean;
  path: string;
  controller_url?: string | null;
  controller_changed?: boolean | null;
}

export interface ProfileDetail {
  name: string;
  active: boolean;
  path: string;
  content: string;
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
