import type { Ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../api';
import type { ToastTone } from './useToasts';

export function useEditorSettings(
  editorPath: Ref<string>,
  setStatus: (message: string, detail?: string) => void,
  pushToast: (message: string, tone?: ToastTone) => void,
) {
  const { t } = useI18n();

  async function saveEditorConfig() {
    try {
      await api.saveAppSettings({ editor_path: editorPath.value.trim() || null });
      setStatus(t('app.editor_saved'), editorPath.value || t('app.editor_auto'));
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    }
  }

  async function pickEditorPath() {
    try {
      const data = await api.pickEditor();
      if (data.editor) {
        editorPath.value = data.editor;
        setStatus(t('app.editor_picked'), data.editor);
      } else {
        pushToast(t('app.editor_cancelled'));
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    }
  }

  async function resetEditorConfig() {
    editorPath.value = '';
    await saveEditorConfig();
  }

  return {
    saveEditorConfig,
    pickEditorPath,
    resetEditorConfig,
  };
}
