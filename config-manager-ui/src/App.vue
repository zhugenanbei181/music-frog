<template>
  <div class="min-h-screen px-6 py-8 lg:px-10">
    <div :class="busy ? 'pointer-events-none opacity-60' : ''" :aria-busy="busy">
      <StatusHeader
        :status-message="status.message"
        :status-detail="status.detail"
        @refresh="refreshAll"
        @toggle-lang="toggleLanguage"
      />

      <main class="mt-6 grid gap-6 lg:grid-cols-12">
        <ProfilesPanel
          :profiles="profiles"
          :active-count="activeCount"
          v-model:filter="profileFilter"
          @refresh="refreshProfiles"
          @clear="clearProfiles"
          @load="loadProfile"
          @open-external="openExternal"
          @switch="switchProfile"
          @delete="deleteProfile"
          @update-subscription="updateSubscription"
          @update-now="updateSubscriptionNow"
        />

        <section class="lg:col-span-8 grid gap-6">
          <div class="grid gap-6 md:grid-cols-2">
            <ImportSubscriptionPanel
              v-model:name="importForm.name"
              v-model:url="importForm.url"
              v-model:activate="importForm.activate"
              @submit="importProfile"
            />
            <ImportLocalPanel
              v-model:name="localForm.name"
              v-model:activate="localForm.activate"
              @file-change="onLocalFileChange"
              @submit="importLocal"
            />
          </div>

          <div class="grid gap-6 md:grid-cols-2">
            <EditorPanel
              ref="editorSection"
              v-model:name="editor.name"
              v-model:content="editor.content"
              v-model:activate="editor.activate"
              @save="saveProfile"
              @reset="resetEditor"
              @open-external="openExternal"
            />
            <CorePanel
              :core-versions="coreVersions"
              :core-current="coreCurrent"
              @refresh="refreshCoreVersions"
              @activate="activateCore"
            />
          </div>

          <EditorSettingsPanel
            v-model:editor-path="editorPath"
            @pick="pickEditorPath"
            @save="saveEditorConfig"
            @reset="resetEditorConfig"
          />
        </section>
      </main>
    </div>

    <BusyOverlay :visible="busy" :message="busyMessage" :detail="busyDetail" />
    <ToastList :toasts="toasts" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from './api';
import type { ProfileInfo } from './types';
import StatusHeader from './components/StatusHeader.vue';
import ProfilesPanel from './components/ProfilesPanel.vue';
import ImportSubscriptionPanel from './components/ImportSubscriptionPanel.vue';
import ImportLocalPanel from './components/ImportLocalPanel.vue';
import EditorPanel from './components/EditorPanel.vue';
import CorePanel from './components/CorePanel.vue';
import EditorSettingsPanel from './components/EditorSettingsPanel.vue';
import BusyOverlay from './components/BusyOverlay.vue';
import ToastList from './components/ToastList.vue';

const { t, locale } = useI18n();

const status = reactive({
  message: t('app.ready_msg'),
  detail: t('app.ready_detail'),
});

const profiles = ref<ProfileInfo[]>([]);
const profileFilter = ref('');
const coreVersions = ref<string[]>([]);
const coreCurrent = ref<string | null>(null);
const editorSection = ref<InstanceType<typeof EditorPanel> | null>(null);

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

const editorPath = ref('');

const toasts = ref<Array<{ id: number; message: string; tone: 'info' | 'error' }>>([]);
let toastCounter = 0;
const busy = ref(false);
const busyMessage = ref('');
const busyDetail = ref('');

const activeCount = computed(() => profiles.value.filter((profile) => profile.active).length);

function pushToast(message: string, tone: 'info' | 'error' = 'info') {
  const id = ++toastCounter;
  toasts.value.push({ id, message, tone });
  setTimeout(() => {
    toasts.value = toasts.value.filter((toast) => toast.id !== id);
  }, 4200);
}

function setStatus(message: string, detail = '') {
  status.message = message;
  status.detail = detail || ' ';
}

function startBusy(message: string, detail: string) {
  busy.value = true;
  busyMessage.value = message;
  busyDetail.value = detail;
}

function updateBusyDetail(detail: string) {
  if (busy.value) {
    busyDetail.value = detail;
  }
}

function endBusy() {
  busy.value = false;
  busyMessage.value = '';
  busyDetail.value = '';
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function toggleLanguage() {
  const newLang = locale.value === 'zh-CN' ? 'en-US' : 'zh-CN';
  locale.value = newLang;
  try {
    await api.saveAppSettings({ language: newLang });
    // Refresh UI text that relies on computed/functions
    refreshAll(true);
    setStatus(t('app.ready_msg'), t('app.ready_detail'));
  } catch (err) {
    pushToast(`Failed to save language setting: ${err}`, 'error');
  }
}

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
    const reason = status.last_reason ? t('app.restarting_reason', { reason: status.last_reason }) : t('app.restarting');
    updateBusyDetail(reason);
    await sleep(1200);
  }
}

async function refreshProfiles(silent = false) {
  try {
    const list = await api.listProfiles();
    profiles.value = list.sort((a, b) => a.name.localeCompare(b.name));
    if (!silent) {
      setStatus(t('app.profiles_updated_title'), t('app.profiles_updated_msg', { count: profiles.value.length }));
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    if (!silent) {
      setStatus(t('app.load_profiles_failed'), message);
    }
    pushToast(message, 'error');
  }
}

async function refreshCoreVersions(silent = false) {
  try {
    const data = await api.listCoreVersions();
    coreVersions.value = data.versions;
    coreCurrent.value = data.current || null;
    if (!silent) {
      setStatus(t('app.core_refreshed'), data.current ? t('app.core_current', { version: data.current }) : t('app.core_default'));
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    if (!silent) {
      setStatus(t('app.load_core_failed'), message);
    }
    pushToast(message, 'error');
  }
}

async function refreshSettings() {
  try {
    const data = await api.getAppSettings();
    editorPath.value = data.editor_path || '';
    if (data.language) {
      locale.value = data.language;
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    pushToast(message, 'error');
  }
}

async function refreshAll(silent = false) {
  await Promise.all([refreshProfiles(silent), refreshCoreVersions(silent), refreshSettings()]);
}

async function loadProfile(name: string) {
  try {
    const detail = await api.getProfile(name);
    editor.name = detail.name;
    editor.content = detail.content;
    editor.activate = detail.active;
    const element = editorSection.value?.$el as HTMLElement | undefined;
    element?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setStatus(t('app.profile_loaded'), detail.name);
  } catch (err) {
    const message = (err as Error).message || String(err);
    pushToast(message, 'error');
  }
}

async function openExternal(name: string) {
  if (!name) {
    pushToast(t('app.select_profile_hint'), 'error');
    return;
  }
  try {
    await api.openProfile(name);
    setStatus(t('app.external_opened'), name);
  } catch (err) {
    const message = (err as Error).message || String(err);
    pushToast(message, 'error');
  }
}

async function switchProfile(name: string) {
  if (busy.value) {
    return;
  }
  startBusy(t('app.switching_busy'), t('app.switching_detail', { name }));
  try {
    const result = await api.switchProfile(name);
    setStatus(t('app.switch_status'), t('app.switching_detail', { name: result.profile.name }));
    if (result.rebuild_scheduled) {
      updateBusyDetail(t('app.switch_rebuild'));
      await waitForRebuild(t('app.switch_status'));
    }
    setStatus(t('app.switch_success'), result.profile.name);
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus(t('app.switch_failed'), message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
  }
}

async function clearProfiles() {
  if (busy.value) {
    return;
  }
  const confirmed = window.confirm(t('app.clear_confirm'));
  if (!confirmed) {
    return;
  }
  startBusy(t('app.clearing_busy'), t('app.clearing_detail'));
  try {
    const result = await api.clearProfiles();
    resetEditor();
    setStatus(t('app.clear_success'), result.profile.name);
    if (result.rebuild_scheduled) {
      updateBusyDetail(t('app.switch_rebuild'));
      await waitForRebuild(t('app.clearing_busy'));
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus(t('app.clear_failed'), message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
  }
}

async function importProfile() {
  if (!importForm.name.trim() || !importForm.url.trim()) {
    pushToast(t('app.import_missing_info'), 'error');
    return;
  }
  if (busy.value) {
    return;
  }
  startBusy(t('app.importing_busy'), t('app.importing_detail', { name: importForm.name.trim() }));
  try {
    const result = await api.importProfile(importForm.name.trim(), importForm.url.trim(), importForm.activate);
    importForm.url = '';
    importForm.name = '';
    setStatus(t('app.import_success'), result.profile.name);
    if (result.rebuild_scheduled) {
      updateBusyDetail(t('app.switch_rebuild'));
      await waitForRebuild(t('app.importing_busy'));
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus(t('app.import_failed'), message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
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
    pushToast(t('app.file_missing'), 'error');
    return;
  }
  if (busy.value) {
    return;
  }
  const name = localForm.name.trim() || localForm.file.name.replace(/\.[^/.]+$/, '');
  startBusy(t('app.saving_local_busy'), t('app.saving_local_detail', { name }));
  try {
    const content = await localForm.file.text();
    if (!content.trim()) {
      throw new Error(t('app.file_empty'));
    }
    const result = await api.saveProfile(name, content, localForm.activate);
    setStatus(t('app.save_local_success'), result.profile.name);
    if (result.rebuild_scheduled) {
      updateBusyDetail(t('app.switch_rebuild'));
      await waitForRebuild(t('app.saving_local_busy'));
    }
    localForm.file = null;
    localForm.name = '';
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus(t('app.save_local_failed'), message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
  }
}

async function saveProfile() {
  if (!editor.name.trim()) {
    pushToast(t('app.name_empty'), 'error');
    return;
  }
  if (!editor.content.trim()) {
    pushToast(t('app.content_empty'), 'error');
    return;
  }
  if (busy.value) {
    return;
  }
  startBusy(t('app.saving_busy'), t('app.saving_detail', { name: editor.name.trim() }));
  try {
    const result = await api.saveProfile(editor.name.trim(), editor.content, editor.activate);
    setStatus(t('app.save_success'), result.profile.name);
    if (result.profile.controller_url) {
      pushToast(t('app.controller_info', { url: result.profile.controller_url }));
    }
    if (result.rebuild_scheduled) {
      updateBusyDetail(t('app.switch_rebuild'));
      await waitForRebuild(t('app.saving_busy'));
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus(t('app.save_failed'), message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
  }
}

async function deleteProfile(name: string) {
  const confirmation = window.prompt(t('app.delete_confirm', { name }));
  if (confirmation !== name) {
    return;
  }
  try {
    await api.deleteProfile(name);
    setStatus(t('app.delete_success'), name);
    if (editor.name === name) {
      resetEditor();
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    pushToast(message, 'error');
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
  if (busy.value) {
    return;
  }
  startBusy(t('app.save_sub_busy'), t('app.save_sub_detail', { name: payload.name }));
  try {
    await api.setProfileSubscription(payload.name, {
      url: payload.url,
      auto_update_enabled: payload.auto_update_enabled,
      update_interval_hours: payload.update_interval_hours ?? null,
    });
    setStatus(t('app.save_sub_success'), payload.name);
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus(t('app.save_sub_failed'), message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
  }
}

async function updateSubscriptionNow(name: string) {
  if (busy.value) {
    return;
  }
  startBusy(t('app.update_sub_busy'), t('app.update_sub_detail', { name }));
  try {
    const result = await api.updateProfileNow(name);
    setStatus(t('app.update_sub_success'), result.profile.name);
    if (result.rebuild_scheduled) {
      updateBusyDetail(t('app.switch_rebuild'));
      await waitForRebuild(t('app.update_sub_busy'));
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus(t('app.update_sub_failed'), message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
  }
}

function resetEditor() {
  editor.name = '';
  editor.content = '';
  editor.activate = false;
  setStatus(t('app.new_profile_ready'), t('app.new_profile_detail'));
}

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

async function activateCore(version: string) {
  if (busy.value) {
    return;
  }
  startBusy(t('app.switch_core_busy'), t('app.switch_core_detail', { version }));
  try {
    await api.activateCoreVersion(version);
    setStatus(t('app.switch_core_status'), t('app.switch_core_detail', { version }));
    updateBusyDetail(t('app.switch_rebuild'));
    await waitForRebuild(t('app.switch_core_busy'));
    setStatus(t('app.switch_core_success'), version);
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus(t('app.switch_core_failed'), message);
    pushToast(message, 'error');
  } finally {
    await refreshCoreVersions(true);
    endBusy();
  }
}

onMounted(() => {
  refreshAll();
});
</script>
