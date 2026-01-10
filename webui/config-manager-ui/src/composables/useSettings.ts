import { reactive, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../api';
import type { WebDavConfig } from '../types';
import type { ToastTone } from './useToasts';

const DEFAULT_WEBDAV: WebDavConfig = {
  enabled: false,
  url: '',
  username: '',
  password: '',
  sync_interval_mins: 60,
  sync_on_startup: false,
};

export function useSettings(pushToast: (message: string, tone?: ToastTone) => void) {
  const { locale } = useI18n();
  const editorPath = ref('');
  const webdav = reactive<WebDavConfig>({ ...DEFAULT_WEBDAV });

  async function refreshSettings() {
    try {
      const data = await api.getAppSettings();
      editorPath.value = data.editor_path || '';
      if (data.language) {
        locale.value = data.language;
      }
      if (data.webdav) {
        Object.assign(webdav, data.webdav);
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    }
  }

  return {
    editorPath,
    webdav,
    refreshSettings,
  };
}
