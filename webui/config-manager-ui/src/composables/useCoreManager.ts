import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../api';
import type { ToastTone } from './useToasts';

type BusyActions = {
  busy: { value: boolean };
  startBusy: (message: string, detail: string) => void;
  updateBusyDetail: (detail: string) => void;
  endBusy: () => void;
};

export function useCoreManager(
  setStatus: (message: string, detail?: string) => void,
  pushToast: (message: string, tone?: ToastTone) => void,
  waitForRebuild: (label: string) => Promise<void>,
  busy: BusyActions,
) {
  const { t } = useI18n();
  const coreVersions = ref<string[]>([]);
  const coreCurrent = ref<string | null>(null);

  async function refreshCoreVersions(silent = false) {
    try {
      const data = await api.listCoreVersions();
      coreVersions.value = data.versions;
      coreCurrent.value = data.current || null;
      if (!silent) {
        setStatus(
          t('app.core_refreshed'),
          data.current ? t('app.core_current', { version: data.current }) : t('app.core_default'),
        );
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      if (!silent) {
        setStatus(t('app.load_core_failed'), message);
      }
      pushToast(message, 'error');
    }
  }

  async function activateCore(version: string) {
    if (busy.busy.value) {
      return;
    }
    busy.startBusy(t('app.switch_core_busy'), t('app.switch_core_detail', { version }));
    try {
      await api.activateCoreVersion(version);
      setStatus(t('app.switch_core_status'), t('app.switch_core_detail', { version }));
      busy.updateBusyDetail(t('app.switch_rebuild'));
      await waitForRebuild(t('app.switch_core_busy'));
      setStatus(t('app.switch_core_success'), version);
    } catch (err) {
      const message = (err as Error).message || String(err);
      setStatus(t('app.switch_core_failed'), message);
      pushToast(message, 'error');
    } finally {
      await refreshCoreVersions(true);
      busy.endBusy();
    }
  }

  return {
    coreVersions,
    coreCurrent,
    refreshCoreVersions,
    activateCore,
  };
}
