import { describe, expect, it, vi } from 'vitest';
import { useWebDavSync } from '../useWebDavSync';
import { api } from '../../api';

vi.mock('../../api', () => ({
  api: {
    saveAppSettings: vi.fn(),
    testWebDav: vi.fn(),
    syncWebDavNow: vi.fn(),
  },
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string, params?: any) => {
        if (key === 'sync.sync_summary') return `success: ${params.success}, failed: ${params.failed}`;
        return key;
    },
  }),
}));

function buildMocks() {
  return {
    webdav: {
      enabled: true,
      url: 'http://dav.com',
      username: 'u',
      password: 'p',
      sync_interval_mins: 60,
      sync_on_startup: true,
    },
    busy: {
      startBusy: vi.fn(),
      endBusy: vi.fn(),
    },
    pushToast: vi.fn(),
    refreshProfiles: vi.fn().mockResolvedValue(undefined),
    refreshSettings: vi.fn().mockResolvedValue(undefined),
  };
}

describe('useWebDavSync', () => {
  it('saves sync config successfully', async () => {
    const mocks = buildMocks();
    const { saveSyncConfig } = useWebDavSync(
      mocks.webdav,
      mocks.busy,
      mocks.pushToast,
      mocks.refreshProfiles,
      mocks.refreshSettings
    );

    await saveSyncConfig();

    expect(api.saveAppSettings).toHaveBeenCalledWith({ webdav: mocks.webdav });
    expect(mocks.refreshSettings).toHaveBeenCalled();
    expect(mocks.pushToast).toHaveBeenCalledWith('settings.save_success');
  });

  it('tests connection and shows success toast', async () => {
    const mocks = buildMocks();
    const { testSync } = useWebDavSync(
      mocks.webdav,
      mocks.busy,
      mocks.pushToast,
      mocks.refreshProfiles,
      mocks.refreshSettings
    );

    await testSync();

    expect(mocks.busy.startBusy).toHaveBeenCalled();
    expect(api.testWebDav).toHaveBeenCalledWith(mocks.webdav);
    expect(mocks.pushToast).toHaveBeenCalledWith('sync.test_success');
    expect(mocks.busy.endBusy).toHaveBeenCalled();
  });

  it('performs sync and shows summary toast', async () => {
    const mocks = buildMocks();
    vi.mocked(api.syncWebDavNow).mockResolvedValue({
      success_count: 5,
      failed_count: 1,
      total_actions: 6,
    });

    const { performSyncNow } = useWebDavSync(
      mocks.webdav,
      mocks.busy,
      mocks.pushToast,
      mocks.refreshProfiles,
      mocks.refreshSettings
    );

    await performSyncNow();

    expect(api.syncWebDavNow).toHaveBeenCalled();
    expect(mocks.pushToast).toHaveBeenCalledWith('success: 5, failed: 1', 'warning');
    expect(mocks.refreshProfiles).toHaveBeenCalledWith(true);
  });

  it('handles sync errors gracefully', async () => {
    const mocks = buildMocks();
    vi.mocked(api.syncWebDavNow).mockRejectedValue(new Error('Network error'));

    const { performSyncNow } = useWebDavSync(
      mocks.webdav,
      mocks.busy,
      mocks.pushToast,
      mocks.refreshProfiles,
      mocks.refreshSettings
    );

    await performSyncNow();

    expect(mocks.pushToast).toHaveBeenCalledWith(expect.stringContaining('Network error'), 'error');
    expect(mocks.busy.endBusy).toHaveBeenCalled();
  });
});
