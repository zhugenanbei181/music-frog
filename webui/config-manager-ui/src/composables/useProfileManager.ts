import { computed, reactive, ref, watch, type Ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../api';
import type { ProfileInfo } from '../types';
import type { ToastTone } from './useToasts';

type BusyActions = {
  busy: Ref<boolean>;
  startBusy: (message: string, detail: string) => void;
  updateBusyDetail: (detail: string) => void;
  endBusy: () => void;
};

type ProfileManagerOptions = {
  busy: BusyActions;
  setStatus: (message: string, detail?: string) => void;
  pushToast: (message: string, tone?: ToastTone) => void;
  waitForRebuild: (label: string) => Promise<void>;
  scrollToEditor: () => void;
};

export function useProfileManager(options: ProfileManagerOptions) {
  const { t } = useI18n();
  const profiles = ref<ProfileInfo[]>([]);
  const profileFilter = ref('');
  const importForm = reactive({
    name: '',
    url: '',
    activate: false,
  });
  const localForm = reactive({
    name: '',
    file: null as File | null,
    activate: false,
  });
  const editor = reactive({
    name: '',
    content: '',
    activate: false,
  });
  const editorDirty = ref(false);
  let suppressEditorDirty = false;

  function setEditorState(next: { name: string; content: string; activate: boolean }) {
    suppressEditorDirty = true;
    editor.name = next.name;
    editor.content = next.content;
    editor.activate = next.activate;
    editorDirty.value = false;
    suppressEditorDirty = false;
  }

  watch(
    editor,
    () => {
      if (!suppressEditorDirty) {
        editorDirty.value = true;
      }
    },
    { deep: true, flush: 'sync' },
  );

  const activeCount = computed(() => profiles.value.filter((profile) => profile.active).length);

  async function refreshProfiles(silent = false) {
    try {
      const list = await api.listProfiles();
      profiles.value = list.sort((a, b) => a.name.localeCompare(b.name));
      if (!silent) {
        options.setStatus(
          t('app.profiles_updated_title'),
          t('app.profiles_updated_msg', { count: profiles.value.length }),
        );
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      if (!silent) {
        options.setStatus(t('app.load_profiles_failed'), message);
      }
      options.pushToast(message, 'error');
    }
  }

  async function loadProfile(name: string) {
    try {
      const detail = await api.getProfile(name);
      setEditorState({
        name: detail.name,
        content: detail.content,
        activate: detail.active,
      });
      options.scrollToEditor();
      options.setStatus(t('app.profile_loaded'), detail.name);
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.pushToast(message, 'error');
    }
  }

  async function openExternal(name: string) {
    if (!name) {
      options.pushToast(t('app.select_profile_hint'), 'error');
      return;
    }
    try {
      await api.openProfile(name);
      options.setStatus(t('app.external_opened'), name);
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.pushToast(message, 'error');
    }
  }

  async function switchProfile(name: string) {
    if (options.busy.busy.value) {
      return;
    }
    options.busy.startBusy(t('app.switching_busy'), t('app.switching_detail', { name }));
    try {
      const result = await api.switchProfile(name);
      options.setStatus(
        t('app.switch_status'),
        t('app.switching_detail', { name: result.profile.name }),
      );
      if (result.rebuild_scheduled) {
        options.busy.updateBusyDetail(t('app.switch_rebuild'));
        await options.waitForRebuild(t('app.switch_status'));
      }
      options.setStatus(t('app.switch_success'), result.profile.name);
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.setStatus(t('app.switch_failed'), message);
      options.pushToast(message, 'error');
    } finally {
      await refreshProfiles(true);
      options.busy.endBusy();
    }
  }

  async function clearProfiles() {
    if (options.busy.busy.value) {
      return;
    }
    const confirmed = window.confirm(t('app.clear_confirm'));
    if (!confirmed) {
      return;
    }
    options.busy.startBusy(t('app.clearing_busy'), t('app.clearing_detail'));
    try {
      const result = await api.clearProfiles();
      resetEditor();
      options.setStatus(t('app.clear_success'), result.profile.name);
      if (result.rebuild_scheduled) {
        options.busy.updateBusyDetail(t('app.switch_rebuild'));
        await options.waitForRebuild(t('app.clearing_busy'));
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.setStatus(t('app.clear_failed'), message);
      options.pushToast(message, 'error');
    } finally {
      await refreshProfiles(true);
      options.busy.endBusy();
    }
  }

  async function importProfile() {
    if (!importForm.name.trim() || !importForm.url.trim()) {
      options.pushToast(t('app.import_missing_info'), 'error');
      return;
    }
    if (options.busy.busy.value) {
      return;
    }
    options.busy.startBusy(t('app.importing_busy'), t('app.importing_detail', { name: importForm.name.trim() }));
    try {
      const result = await api.importProfile(
        importForm.name.trim(),
        importForm.url.trim(),
        importForm.activate,
      );
      importForm.url = '';
      importForm.name = '';
      options.setStatus(t('app.import_success'), result.profile.name);
      if (result.rebuild_scheduled) {
        options.busy.updateBusyDetail(t('app.switch_rebuild'));
        await options.waitForRebuild(t('app.importing_busy'));
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.setStatus(t('app.import_failed'), message);
      options.pushToast(message, 'error');
    } finally {
      await refreshProfiles(true);
      options.busy.endBusy();
    }
  }

  function onLocalFileChange(file: File | null) {
    localForm.file = file;
    if (file && !localForm.name.trim()) {
      localForm.name = file.name.replace(/\.[^/.]+$/, '');
    }
  }

  async function importLocal() {
    if (!localForm.file) {
      options.pushToast(t('app.file_missing'), 'error');
      return;
    }
    if (options.busy.busy.value) {
      return;
    }
    const name = localForm.name.trim() || localForm.file.name.replace(/\.[^/.]+$/, '');
    options.busy.startBusy(t('app.saving_local_busy'), t('app.saving_local_detail', { name }));
    try {
      const content = await localForm.file.text();
      if (!content.trim()) {
        throw new Error(t('app.file_empty'));
      }
      const result = await api.saveProfile(name, content, localForm.activate);
      options.setStatus(t('app.save_local_success'), result.profile.name);
      if (result.rebuild_scheduled) {
        options.busy.updateBusyDetail(t('app.switch_rebuild'));
        await options.waitForRebuild(t('app.saving_local_busy'));
      }
      localForm.file = null;
      localForm.name = '';
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.setStatus(t('app.save_local_failed'), message);
      options.pushToast(message, 'error');
    } finally {
      await refreshProfiles(true);
      options.busy.endBusy();
    }
  }

  async function saveProfile() {
    if (!editor.name.trim()) {
      options.pushToast(t('app.name_empty'), 'error');
      return;
    }
    if (!editor.content.trim()) {
      options.pushToast(t('app.content_empty'), 'error');
      return;
    }
    if (options.busy.busy.value) {
      return;
    }
    options.busy.startBusy(t('app.saving_busy'), t('app.saving_detail', { name: editor.name.trim() }));
    try {
      const result = await api.saveProfile(editor.name.trim(), editor.content, editor.activate);
      options.setStatus(t('app.save_success'), result.profile.name);
      if (result.profile.controller_url) {
        options.pushToast(t('app.controller_info', { url: result.profile.controller_url }));
      }
      if (result.rebuild_scheduled) {
        options.busy.updateBusyDetail(t('app.switch_rebuild'));
        await options.waitForRebuild(t('app.saving_busy'));
      }
      editorDirty.value = false;
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.setStatus(t('app.save_failed'), message);
      options.pushToast(message, 'error');
    } finally {
      await refreshProfiles(true);
      options.busy.endBusy();
    }
  }

  async function deleteProfile(name: string) {
    const confirmation = window.prompt(t('app.delete_confirm', { name }));
    if (confirmation !== name) {
      return;
    }
    try {
      await api.deleteProfile(name);
      options.setStatus(t('app.delete_success'), name);
      if (editor.name === name) {
        resetEditor();
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.pushToast(message, 'error');
    } finally {
      await refreshProfiles(true);
    }
  }

  async function updateSubscription(payload: {
    name: string;
    url: string;
    auto_update_enabled: boolean;
    update_interval_hours?: number | null;
  }) {
    if (options.busy.busy.value) {
      return;
    }
    options.busy.startBusy(t('app.save_sub_busy'), t('app.save_sub_detail', { name: payload.name }));
    try {
      await api.setProfileSubscription(payload.name, {
        url: payload.url,
        auto_update_enabled: payload.auto_update_enabled,
        update_interval_hours: payload.update_interval_hours ?? null,
      });
      options.setStatus(t('app.save_sub_success'), payload.name);
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.setStatus(t('app.save_sub_failed'), message);
      options.pushToast(message, 'error');
    } finally {
      await refreshProfiles(true);
      options.busy.endBusy();
    }
  }

  async function updateSubscriptionNow(name: string) {
    if (options.busy.busy.value) {
      return;
    }
    options.busy.startBusy(t('app.update_sub_busy'), t('app.update_sub_detail', { name }));
    try {
      const result = await api.updateProfileNow(name);
      options.setStatus(t('app.update_sub_success'), result.profile.name);
      if (result.rebuild_scheduled) {
        options.busy.updateBusyDetail(t('app.switch_rebuild'));
        await options.waitForRebuild(t('app.update_sub_busy'));
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      options.setStatus(t('app.update_sub_failed'), message);
      options.pushToast(message, 'error');
    } finally {
      await refreshProfiles(true);
      options.busy.endBusy();
    }
  }

  function resetEditor() {
    setEditorState({ name: '', content: '', activate: false });
    options.setStatus(t('app.new_profile_ready'), t('app.new_profile_detail'));
  }

  return {
    profiles,
    profileFilter,
    importForm,
    localForm,
    editor,
    editorDirty,
    activeCount,
    refreshProfiles,
    loadProfile,
    openExternal,
    switchProfile,
    clearProfiles,
    importProfile,
    onLocalFileChange,
    importLocal,
    saveProfile,
    deleteProfile,
    updateSubscription,
    updateSubscriptionNow,
    resetEditor,
  };
}
