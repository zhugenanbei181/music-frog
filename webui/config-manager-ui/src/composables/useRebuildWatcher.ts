import { useI18n } from 'vue-i18n';
import { api } from '../api';

export function useRebuildWatcher(updateBusyDetail: (detail: string) => void) {
  const { t } = useI18n();

  async function waitForRebuild(label: string) {
    const timeoutMs = 120_000;
    const start = Date.now();
    while (true) {
      let status;
      try {
        status = await api.getRebuildStatus();
      } catch (err) {
        const message = (err as Error).message || String(err);
        const elapsed = Date.now() - start;
        if (elapsed > timeoutMs) {
          throw new Error(t('app.timeout_wait', { label, message }));
        }
        updateBusyDetail(t('app.timeout_retry', { message }));
        await sleep(1200);
        continue;
      }
      if (!status.in_progress) {
        if (status.last_error) {
          throw new Error(status.last_error);
        }
        return;
      }
      const elapsed = Date.now() - start;
      if (elapsed > timeoutMs) {
        throw new Error(t('app.timeout_generic', { label }));
      }
      const reason = status.last_reason
        ? t('app.restarting_reason', { reason: status.last_reason })
        : t('app.restarting');
      updateBusyDetail(reason);
      await sleep(1200);
    }
  }

  function sleep(ms: number) {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  return {
    waitForRebuild,
  };
}
