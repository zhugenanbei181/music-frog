<template>
  <div class="min-h-screen px-6 py-8 lg:px-10">
    <div :class="busy ? 'pointer-events-none opacity-60' : ''" :aria-busy="busy">
      <StatusHeader
        :status-message="status.message"
        :status-detail="status.detail"
        @refresh="refreshAll"
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

const status = reactive({
  message: '准备就绪',
  detail: '等待操作中…',
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
        throw new Error(`${label}超时：${message}`);
      }
      updateBusyDetail(`等待状态失败，重试中… (${message})`);
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
      throw new Error(`${label}超时，请稍后重试`);
    }
    const reason = status.last_reason ? `内核重启中：${status.last_reason}` : '内核重启中，请稍候…';
    updateBusyDetail(reason);
    await sleep(1200);
  }
}

async function refreshProfiles(silent = false) {
  try {
    const list = await api.listProfiles();
    profiles.value = list.sort((a, b) => a.name.localeCompare(b.name));
    if (!silent) {
      setStatus('配置列表已更新', `共 ${profiles.value.length} 份配置`);
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    if (!silent) {
      setStatus('加载配置失败', message);
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
      setStatus('内核版本已刷新', data.current ? `当前 ${data.current}` : '使用默认内核');
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    if (!silent) {
      setStatus('加载内核版本失败', message);
    }
    pushToast(message, 'error');
  }
}

async function refreshEditorConfig() {
  try {
    const data = await api.getEditor();
    editorPath.value = data.editor || '';
  } catch (err) {
    const message = (err as Error).message || String(err);
    pushToast(message, 'error');
  }
}

async function refreshAll() {
  await Promise.all([refreshProfiles(), refreshCoreVersions(), refreshEditorConfig()]);
}

async function loadProfile(name: string) {
  try {
    const detail = await api.getProfile(name);
    editor.name = detail.name;
    editor.content = detail.content;
    editor.activate = detail.active;
    const element = editorSection.value?.$el as HTMLElement | undefined;
    element?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setStatus('已载入配置', detail.name);
  } catch (err) {
    const message = (err as Error).message || String(err);
    pushToast(message, 'error');
  }
}

async function openExternal(name: string) {
  if (!name) {
    pushToast('请先选择或输入配置名称', 'error');
    return;
  }
  try {
    await api.openProfile(name);
    setStatus('已在外部编辑器打开', name);
  } catch (err) {
    const message = (err as Error).message || String(err);
    pushToast(message, 'error');
  }
}

async function switchProfile(name: string) {
  if (busy.value) {
    return;
  }
  startBusy('切换配置中', `正在切换到 ${name}`);
  try {
    const result = await api.switchProfile(name);
    setStatus('配置切换中', `正在切换到 ${result.profile.name}`);
    if (result.rebuild_scheduled) {
      updateBusyDetail('内核重启中，请稍候…');
      await waitForRebuild('切换配置');
    }
    setStatus('配置已切换', result.profile.name);
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus('配置切换失败', message);
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
  const confirmed = window.confirm(
    '清空配置会删除所有订阅与本地配置，并恢复默认配置。此操作不可撤销，是否继续？',
  );
  if (!confirmed) {
    return;
  }
  startBusy('清空配置中', '正在恢复默认配置');
  try {
    const result = await api.clearProfiles();
    resetEditor();
    setStatus('配置已清空', result.profile.name);
    if (result.rebuild_scheduled) {
      updateBusyDetail('内核重启中，请稍候…');
      await waitForRebuild('清空配置');
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus('清空配置失败', message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
  }
}

async function importProfile() {
  if (!importForm.name.trim() || !importForm.url.trim()) {
    pushToast('请填写配置名称与订阅链接', 'error');
    return;
  }
  if (busy.value) {
    return;
  }
  startBusy('订阅导入中', `正在导入 ${importForm.name.trim()}`);
  try {
    const result = await api.importProfile(importForm.name.trim(), importForm.url.trim(), importForm.activate);
    importForm.url = '';
    importForm.name = '';
    setStatus('订阅导入完成', result.profile.name);
    if (result.rebuild_scheduled) {
      updateBusyDetail('内核重启中，请稍候…');
      await waitForRebuild('订阅导入');
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus('订阅导入失败', message);
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
    pushToast('请选择本地配置文件', 'error');
    return;
  }
  if (busy.value) {
    return;
  }
  const name = localForm.name.trim() || localForm.file.name.replace(/\.[^/.]+$/, '');
  startBusy('保存本地配置', `正在保存 ${name}`);
  try {
    const content = await localForm.file.text();
    if (!content.trim()) {
      throw new Error('文件内容为空');
    }
    const result = await api.saveProfile(name, content, localForm.activate);
    setStatus('本地配置已保存', result.profile.name);
    if (result.rebuild_scheduled) {
      updateBusyDetail('内核重启中，请稍候…');
      await waitForRebuild('本地配置保存');
    }
    localForm.file = null;
    localForm.name = '';
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus('本地配置保存失败', message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
  }
}

async function saveProfile() {
  if (!editor.name.trim()) {
    pushToast('配置名称不能为空', 'error');
    return;
  }
  if (!editor.content.trim()) {
    pushToast('配置内容不能为空', 'error');
    return;
  }
  if (busy.value) {
    return;
  }
  startBusy('保存配置', `正在保存 ${editor.name.trim()}`);
  try {
    const result = await api.saveProfile(editor.name.trim(), editor.content, editor.activate);
    setStatus('配置已保存', result.profile.name);
    if (result.profile.controller_url) {
      pushToast(`控制接口 ${result.profile.controller_url}`);
    }
    if (result.rebuild_scheduled) {
      updateBusyDetail('内核重启中，请稍候…');
      await waitForRebuild('保存配置');
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus('配置保存失败', message);
    pushToast(message, 'error');
  } finally {
    await refreshProfiles(true);
    endBusy();
  }
}

async function deleteProfile(name: string) {
  const confirmation = window.prompt(`危险操作：删除配置 ${name}
请输入配置名确认删除`);
  if (confirmation !== name) {
    return;
  }
  try {
    await api.deleteProfile(name);
    setStatus('配置已删除', name);
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
  startBusy('保存订阅设置', `正在更新 ${payload.name}`);
  try {
    await api.setProfileSubscription(payload.name, {
      url: payload.url,
      auto_update_enabled: payload.auto_update_enabled,
      update_interval_hours: payload.update_interval_hours ?? null,
    });
    setStatus('订阅设置已保存', payload.name);
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus('订阅设置失败', message);
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
  startBusy('订阅更新中', `正在更新 ${name}`);
  try {
    const result = await api.updateProfileNow(name);
    setStatus('订阅已更新', result.profile.name);
    if (result.rebuild_scheduled) {
      updateBusyDetail('内核重启中，请稍候…');
      await waitForRebuild('订阅更新');
    }
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus('订阅更新失败', message);
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
  setStatus('已准备新配置', '输入名称并粘贴 YAML');
}

async function saveEditorConfig() {
  try {
    await api.setEditor(editorPath.value.trim() || null);
    setStatus('编辑器设置已保存', editorPath.value || '自动检测');
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
      setStatus('已选择编辑器路径', data.editor);
    } else {
      pushToast('已取消编辑器路径选择');
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
  startBusy('切换内核版本', `正在切换到 ${version}`);
  try {
    await api.activateCoreVersion(version);
    setStatus('内核切换中', `正在切换到 ${version}`);
    updateBusyDetail('内核重启中，请稍候…');
    await waitForRebuild('切换内核');
    setStatus('已切换内核版本', version);
  } catch (err) {
    const message = (err as Error).message || String(err);
    setStatus('内核切换失败', message);
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
