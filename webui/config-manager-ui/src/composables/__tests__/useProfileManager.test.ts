import { describe, expect, it, vi } from 'vitest';
import { ref } from 'vue';
import { useProfileManager } from '../useProfileManager';
import { api } from '../../api';

vi.mock('../../api', () => ({
  api: {
    listProfiles: vi.fn().mockResolvedValue([]),
    setProfileSubscription: vi.fn().mockResolvedValue({}),
    updateProfileNow: vi.fn().mockResolvedValue({
      profile: { name: 'p' },
      rebuild_scheduled: true,
    }),
  },
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

function buildOptions() {
  const busy = ref(false);
  return {
    busy: {
      busy,
      startBusy: vi.fn(),
      updateBusyDetail: vi.fn(),
      endBusy: vi.fn(),
    },
    setStatus: vi.fn(),
    pushToast: vi.fn(),
    waitForRebuild: vi.fn().mockResolvedValue(undefined),
    scrollToEditor: vi.fn(),
  };
}

describe('useProfileManager', () => {
  it('sends null interval when update interval is not provided', async () => {
    const options = buildOptions();
    const { updateSubscription } = useProfileManager(options);

    await updateSubscription({
      name: 'p',
      url: 'https://example.com',
      auto_update_enabled: true,
    });

    expect(api.setProfileSubscription).toHaveBeenCalledWith('p', {
      url: 'https://example.com',
      auto_update_enabled: true,
      update_interval_hours: null,
    });
  });

  it('waits for rebuild when update-now schedules one', async () => {
    const options = buildOptions();
    const { updateSubscriptionNow } = useProfileManager(options);

    await updateSubscriptionNow('p');

    expect(api.updateProfileNow).toHaveBeenCalledWith('p');
    expect(options.waitForRebuild).toHaveBeenCalled();
  });
});
