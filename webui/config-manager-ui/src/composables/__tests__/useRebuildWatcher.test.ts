import { afterEach, describe, expect, it, vi } from 'vitest';
import { useRebuildWatcher } from '../useRebuildWatcher';
import { api } from '../../api';

vi.mock('../../api', () => ({
  api: {
    getRebuildStatus: vi.fn(),
  },
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe('useRebuildWatcher', () => {
  it('resolves after rebuild completes', async () => {
    const updates: string[] = [];
    const { waitForRebuild } = useRebuildWatcher((detail) => updates.push(detail));
    const getRebuildStatus = vi.mocked(api.getRebuildStatus);
    getRebuildStatus
      .mockResolvedValueOnce({ in_progress: true, last_error: null, last_reason: 'import-activate' })
      .mockResolvedValueOnce({ in_progress: false, last_error: null, last_reason: 'import-activate' });

    vi.useFakeTimers();
    const promise = waitForRebuild('label');
    await vi.advanceTimersByTimeAsync(1200);
    await promise;

    expect(getRebuildStatus).toHaveBeenCalledTimes(2);
    expect(updates.length).toBeGreaterThan(0);
  });

  it('throws when rebuild reports an error', async () => {
    const { waitForRebuild } = useRebuildWatcher(() => undefined);
    const getRebuildStatus = vi.mocked(api.getRebuildStatus);
    getRebuildStatus.mockResolvedValueOnce({
      in_progress: false,
      last_error: 'boom',
      last_reason: 'import-activate',
    });

    await expect(waitForRebuild('label')).rejects.toThrow('boom');
  });
});
