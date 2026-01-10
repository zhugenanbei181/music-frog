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

          <div class="grid gap-6 md:grid-cols-2">
            <EditorSettingsPanel
              v-model:editor-path="editorPath"
              @pick="pickEditorPath"
              @save="saveEditorConfig"
              @reset="resetEditorConfig"
            />
            <SyncSettingsPanel
              v-model="webdav"
              @save="saveSyncConfig"
              @test="testSync"
              @sync-now="performSyncNow"
            />
          </div>

          <div class="grid gap-6 md:grid-cols-2">
            <div ref="dnsSection" id="dns">
              <DnsPanel
                v-model="dnsConfig"
                @save="saveDnsConfig"
                @refresh="refreshDnsConfig"
              />
            </div>
            <div ref="fakeIpSection" id="fake-ip">
              <FakeIpPanel
                v-model="fakeIpConfig"
                @save="saveFakeIpConfig"
                @refresh="refreshFakeIpConfig"
                @flush="flushFakeIpCache"
              />
            </div>
          </div>

          <div class="grid gap-6 md:grid-cols-2">
            <div ref="rulesSection" id="rules">
              <RulesPanel
                v-model:rules="rules"
                v-model:providers-json="ruleProvidersJson"
                @save-rules="saveRules"
                @save-providers="saveRuleProviders"
                @refresh="refreshRulesAndProviders"
              />
            </div>
            <div ref="tunSection" id="tun">
              <TunPanel
                v-model="tunConfig"
                @save="saveTunConfig"
                @refresh="refreshTunConfig"
              />
            </div>
          </div>
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
import type { ProfileInfo, WebDavConfig } from './types';
import { useAdvancedSettings } from './composables/useAdvancedSettings';
import { useBusyState } from './composables/useBusyState';
import { useCoreManager } from './composables/useCoreManager';
import { useEditorSettings } from './composables/useEditorSettings';
import { usePanelNavigator } from './composables/usePanelNavigator';
import { useProfileManager } from './composables/useProfileManager';
import { useRebuildWatcher } from './composables/useRebuildWatcher';
import { useSettings } from './composables/useSettings';
import { useToasts } from './composables/useToasts';
import { useWebDavSync } from './composables/useWebDavSync';
import StatusHeader from './components/StatusHeader.vue';
import ProfilesPanel from './components/ProfilesPanel.vue';
import ImportSubscriptionPanel from './components/ImportSubscriptionPanel.vue';
import ImportLocalPanel from './components/ImportLocalPanel.vue';
import EditorPanel from './components/EditorPanel.vue';
import CorePanel from './components/CorePanel.vue';
import EditorSettingsPanel from './components/EditorSettingsPanel.vue';
import SyncSettingsPanel from './components/SyncSettingsPanel.vue';
import DnsPanel from './components/DnsPanel.vue';
import FakeIpPanel from './components/FakeIpPanel.vue';
import RulesPanel from './components/RulesPanel.vue';
import TunPanel from './components/TunPanel.vue';
import BusyOverlay from './components/BusyOverlay.vue';
import ToastList from './components/ToastList.vue';

const { t, locale } = useI18n();

const status = reactive({
  message: t('app.ready_msg'),
  detail: t('app.ready_detail'),
});

const { toasts, pushToast } = useToasts();
const { busy, busyMessage, busyDetail, startBusy, updateBusyDetail, endBusy } = useBusyState();
const { waitForRebuild } = useRebuildWatcher(updateBusyDetail);
const { editorPath, webdav, refreshSettings } = useSettings(pushToast);
const editorSection = ref<InstanceType<typeof EditorPanel> | null>(null);
const dnsSection = ref<HTMLElement | null>(null);
const fakeIpSection = ref<HTMLElement | null>(null);
const rulesSection = ref<HTMLElement | null>(null);
const tunSection = ref<HTMLElement | null>(null);

usePanelNavigator({
  dns: dnsSection,
  'fake-ip': fakeIpSection,
  rules: rulesSection,
  tun: tunSection,
});

function scrollToEditor() {
  const element = editorSection.value?.$el as HTMLElement | undefined;
  element?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

const {
  profiles,
  profileFilter,
  importForm,
  localForm,
  editor,
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
} = useProfileManager({
  busy,
  setStatus,
  pushToast,
  waitForRebuild,
  scrollToEditor,
});

const { coreVersions, coreCurrent, refreshCoreVersions, activateCore } = useCoreManager(
  setStatus,
  pushToast,
  waitForRebuild,
  {
    busy,
    startBusy,
    updateBusyDetail,
    endBusy,
  },
);

const { saveEditorConfig, pickEditorPath, resetEditorConfig } = useEditorSettings(
  editorPath,
  setStatus,
  pushToast,
);

const { saveSyncConfig, testSync, performSyncNow } = useWebDavSync(
  webdav,
  { startBusy, endBusy },
  pushToast,
  refreshProfiles,
);
const {
  dnsConfig,
  fakeIpConfig,
  tunConfig,
  rules,
  ruleProvidersJson,
  refreshDnsConfig,
  refreshFakeIpConfig,
  refreshRulesAndProviders,
  refreshTunConfig,
  saveDnsConfig,
  saveFakeIpConfig,
  flushFakeIpCache,
  saveRuleProviders,
  saveRules,
  saveTunConfig,
} = useAdvancedSettings(pushToast, {
  busy,
  startBusy,
  updateBusyDetail,
  endBusy,
});

function setStatus(message: string, detail = '') {
  status.message = message;
  status.detail = detail || ' ';
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



async function refreshAll(silent = false) {
  await Promise.all([
    refreshProfiles(silent),
    refreshCoreVersions(silent),
    refreshSettings(),
    refreshDnsConfig(silent),
    refreshFakeIpConfig(silent),
    refreshRulesAndProviders(silent),
    refreshTunConfig(silent),
  ]);
}

onMounted(() => {
  refreshAll();
});
</script>
