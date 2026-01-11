import type { WebDavConfig } from '../types';
import { useI18n } from 'vue-i18n';
import { api } from '../api';
import type { ToastTone } from './useToasts';

type BusyActions = {
  startBusy: (message: string, detail: string) => void;
  endBusy: () => void;
};

export function useWebDavSync(
  webdav: WebDavConfig,
  busy: BusyActions,
  pushToast: (message: string, tone?: ToastTone) => void,
  refreshProfiles: (silent?: boolean) => Promise<void>,
  refreshSettings: () => Promise<void>,
) {
  const { t } = useI18n();

  async function saveSyncConfig() {
    try {
      await api.saveAppSettings({ webdav: { ...webdav } });
      await refreshSettings();
      pushToast(t('settings.save_success') || t('common.save_success'));
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    }
  }

  async function testSync() {
    busy.startBusy(t('sync.test_conn'), t('sync.testing_detail') || '...');
    try {
      await api.testWebDav({ ...webdav });
      pushToast(t('sync.test_success'));
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(t('sync.test_failed') + ': ' + message, 'error');
    } finally {
      busy.endBusy();
    }
  }

  async function performSyncNow() {
    busy.startBusy(t('sync.sync_now_btn'), t('sync.syncing_detail') || '...');
    try {
      const result = await api.syncWebDavNow();
      if (result.total_actions === 0) {
        pushToast(t('sync.sync_no_changes') || t('sync.sync_success'));
      } else {
        const msg =
          t('sync.sync_summary', {
            success: result.success_count,
            failed: result.failed_count,
          }) || `${t('sync.sync_success')}: ${result.success_count} synced`;
        pushToast(msg, result.failed_count > 0 ? 'warning' : 'success');
      }
      await refreshProfiles(true);
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(t('sync.sync_failed') + ': ' + message, 'error');
    } finally {
      busy.endBusy();
    }
  }

  return {
    saveSyncConfig,
    testSync,
    performSyncNow,
  };
}
