import { reactive, ref, watch } from 'vue';
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
  const languageSetting = ref('system');
  const themeSetting = ref('system');
  const editorPath = ref('');
  const webdav = reactive<WebDavConfig>({ ...DEFAULT_WEBDAV });
  const dirty = reactive({
    editorPath: false,
    webdav: false,
  });
  const suppressDirty = {
    editorPath: false,
    webdav: false,
  };

  function setCleanEditorPath(value: string) {
    suppressDirty.editorPath = true;
    editorPath.value = value;
    dirty.editorPath = false;
    suppressDirty.editorPath = false;
  }

  function setCleanWebdav(value: WebDavConfig) {
    suppressDirty.webdav = true;
    Object.assign(webdav, value);
    dirty.webdav = false;
    suppressDirty.webdav = false;
  }

  watch(
    editorPath,
    () => {
      if (!suppressDirty.editorPath) {
        dirty.editorPath = true;
      }
    },
    { flush: 'sync' },
  );
  watch(
    webdav,
    () => {
      if (!suppressDirty.webdav) {
        dirty.webdav = true;
      }
    },
    { deep: true, flush: 'sync' },
  );

  const themeMedia =
    typeof window !== 'undefined'
      ? window.matchMedia('(prefers-color-scheme: dark)')
      : null;
  let themeListener: ((event: MediaQueryListEvent) => void) | null = null;

  function resolveSystemLanguage() {
    if (typeof navigator === 'undefined') {
      return 'zh-CN';
    }
    const candidates = Array.isArray(navigator.languages) && navigator.languages.length
      ? navigator.languages
      : [navigator.language];
    for (const lang of candidates) {
      const normalized = String(lang || '').trim().toLowerCase();
      if (normalized.startsWith('zh')) {
        return 'zh-CN';
      }
      if (normalized.startsWith('en')) {
        return 'en-US';
      }
    }
    return 'zh-CN';
  }

  function resolveLanguage(value: string) {
    const normalized = value.trim().toLowerCase();
    if (normalized === 'system') {
      return resolveSystemLanguage();
    }
    if (normalized.startsWith('zh')) {
      return 'zh-CN';
    }
    if (normalized.startsWith('en')) {
      return 'en-US';
    }
    return value;
  }

  function applyTheme(value: string) {
    if (typeof document === 'undefined') {
      return;
    }
    const resolved =
      value === 'system'
        ? themeMedia?.matches
          ? 'dark'
          : 'light'
        : value;
    const root = document.documentElement;
    root.dataset.theme = resolved;
    root.style.colorScheme = resolved === 'dark' ? 'dark' : 'light';
  }

  function attachThemeListener() {
    if (!themeMedia || themeListener) {
      return;
    }
    themeListener = (event) => {
      if (themeSetting.value === 'system') {
        applyTheme(event.matches ? 'dark' : 'light');
      }
    };
    if (typeof themeMedia.addEventListener === 'function') {
      themeMedia.addEventListener('change', themeListener);
    } else if (typeof themeMedia.addListener === 'function') {
      themeMedia.addListener(themeListener);
    }
  }

  function detachThemeListener() {
    if (!themeMedia || !themeListener) {
      return;
    }
    if (typeof themeMedia.removeEventListener === 'function') {
      themeMedia.removeEventListener('change', themeListener);
    } else if (typeof themeMedia.removeListener === 'function') {
      themeMedia.removeListener(themeListener);
    }
    themeListener = null;
  }

  watch(
    languageSetting,
    (value) => {
      locale.value = resolveLanguage(value);
    },
    { immediate: true, flush: 'sync' },
  );

  watch(
    themeSetting,
    (value) => {
      if (value === 'system') {
        applyTheme('system');
        attachThemeListener();
      } else {
        detachThemeListener();
        applyTheme(value);
      }
    },
    { immediate: true, flush: 'sync' },
  );

  async function refreshSettings() {
    try {
      const data = await api.getAppSettings();
      setCleanEditorPath(data.editor_path || '');
      languageSetting.value = data.language || 'system';
      themeSetting.value = data.theme || 'system';
      if (data.webdav) {
        setCleanWebdav(data.webdav);
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    }
  }

  return {
    languageSetting,
    themeSetting,
    editorPath,
    webdav,
    dirty,
    refreshSettings,
  };
}
