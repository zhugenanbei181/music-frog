<template>
  <div class="min-h-screen px-6 py-8 lg:px-10">
    <div :class="busy ? 'pointer-events-none opacity-60' : ''" :aria-busy="busy">
      <StatusHeader
        :status-message="status.message"
        :status-detail="status.detail"
        :has-pending-updates="hasPendingUpdates"
        :language-value="languageSetting"
        :theme-value="themeSetting"
        :language-options="languageOptions"
        :theme-options="themeOptions"
        @refresh="refreshAll"
        @refresh-updates="refreshPendingUpdates"
        @update:language="updateLanguageSetting"
        @update:theme="updateThemeSetting"
      />

      <main class="mt-6 grid gap-6 lg:grid-cols-12">
        <SideNav
          class="lg:col-span-3"
          v-model="activeSection"
          :title="t('nav.title')"
          :items="navItems"
        />

        <section class="lg:col-span-9">
          <section v-if="activeSection === 'profiles'" class="grid gap-6 lg:grid-cols-2">
            <ProfilesPanel
              class="lg:col-span-2"
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

            <EditorPanel
              ref="editorSection"
              class="lg:col-span-2"
              v-model:name="editor.name"
              v-model:content="editor.content"
              v-model:activate="editor.activate"
              @save="saveProfile"
              @reset="resetEditor"
              @open-external="openExternal"
            />
            <EditorSettingsPanel
              class="lg:col-span-2"
              v-model:editor-path="editorPath"
              @pick="pickEditorPath"
              @save="saveEditorConfig"
              @reset="resetEditorConfig"
            />
          </section>

          <section v-else-if="activeSection === 'webdav'" class="grid gap-6 lg:grid-cols-2">
            <SyncSettingsPanel
              class="lg:col-span-2"
              v-model="webdav"
              @save="saveSyncConfig"
              @test="testSync"
              @sync-now="performSyncNow"
            />
          </section>

          <section v-else-if="activeSection === 'network'" class="grid gap-6 md:grid-cols-2">
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
          </section>

          <section v-else-if="activeSection === 'core'" class="grid gap-6 md:grid-cols-2">
            <CorePanel
              :core-versions="coreVersions"
              :core-current="coreCurrent"
              @refresh="refreshCoreVersions"
              @activate="activateCore"
            />
            <div ref="tunSection" id="tun">
              <TunPanel
                v-model="tunConfig"
                @save="saveTunConfig"
                @refresh="refreshTunConfig"
              />
            </div>
          </section>

          <section v-else-if="activeSection === 'rules'" class="grid gap-6">
            <div ref="rulesSection" id="rules">
              <RulesPanel
                v-model:rules="rules"
                v-model:providers-json="ruleProvidersJson"
                @save-rules="saveRules"
                @save-providers="saveRuleProviders"
                @refresh="refreshRulesAndProviders"
              />
            </div>
          </section>
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
import { useAdvancedSettings } from './composables/useAdvancedSettings';
import { useBusyState } from './composables/useBusyState';
import { useCoreManager } from './composables/useCoreManager';
import { useAdminEventStream } from './composables/useAdminEventStream';
import { useEditorSettings } from './composables/useEditorSettings';
import { usePanelNavigator } from './composables/usePanelNavigator';
import { useProfileManager } from './composables/useProfileManager';
import { useRebuildWatcher } from './composables/useRebuildWatcher';
import { useSettings } from './composables/useSettings';
import { useToasts } from './composables/useToasts';
import { useWebDavSync } from './composables/useWebDavSync';
import StatusHeader from './components/StatusHeader.vue';
import SideNav from './components/SideNav.vue';
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

const { t } = useI18n();

const status = reactive({
  message: t('app.ready_msg'),
  detail: t('app.ready_detail'),
});

const { toasts, pushToast } = useToasts();
const { busy, busyMessage, busyDetail, startBusy, updateBusyDetail, endBusy } = useBusyState();
const { waitForRebuild } = useRebuildWatcher(updateBusyDetail);
const {
  languageSetting,
  themeSetting,
  editorPath,
  webdav,
  refreshSettings,
  dirty: settingsDirty,
} = useSettings(pushToast);
const editorSection = ref<InstanceType<typeof EditorPanel> | null>(null);
const dnsSection = ref<HTMLElement | null>(null);
const fakeIpSection = ref<HTMLElement | null>(null);
const rulesSection = ref<HTMLElement | null>(null);
const tunSection = ref<HTMLElement | null>(null);
const activeSection = ref('profiles');

const navItems = computed(() => [
  { id: 'profiles', label: t('nav.profiles') },
  { id: 'webdav', label: t('nav.webdav') },
  { id: 'network', label: t('nav.network') },
  { id: 'core', label: t('nav.core') },
  { id: 'rules', label: t('nav.rules') },
]);

const languageOptions = computed(() => [
  { value: 'system', label: t('header.language_system') },
  { value: 'zh-CN', label: t('header.language_zh') },
  { value: 'en-US', label: t('header.language_en') },
]);

const themeOptions = computed(() => [
  { value: 'system', label: t('header.theme_system') },
  { value: 'light', label: t('header.theme_light') },
  { value: 'dark', label: t('header.theme_dark') },
]);

const anchorSectionMap: Record<string, string> = {
  dns: 'network',
  'fake-ip': 'network',
  tun: 'core',
  rules: 'rules',
};

usePanelNavigator(
  {
    dns: dnsSection,
    'fake-ip': fakeIpSection,
    rules: rulesSection,
    tun: tunSection,
  },
  {
    onActivate(anchor) {
      const section = anchorSectionMap[anchor];
      if (section) {
        activeSection.value = section;
      }
    },
  },
);

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
} = useProfileManager({
  busy: {
    busy,
    startBusy,
    updateBusyDetail,
    endBusy,
  },
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
  refreshSettings,
);

const { saveSyncConfig, testSync, performSyncNow } = useWebDavSync(
  webdav,
  { startBusy, endBusy },
  pushToast,
  refreshProfiles,
  refreshSettings,
);
const {
  dnsConfig,
  fakeIpConfig,
  tunConfig,
  rules,
  ruleProvidersJson,
  dirty: advancedDirty,
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

const hasUnsavedChanges = computed(() =>
  editorDirty.value ||
  settingsDirty.editorPath ||
  settingsDirty.webdav ||
  advancedDirty.dns ||
  advancedDirty.fakeIp ||
  advancedDirty.tun ||
  advancedDirty.rules ||
  advancedDirty.ruleProviders,
);

const {
  hasPendingUpdates,
  refreshPendingUpdates,
  clearPendingUpdates,
} = useAdminEventStream({
  busy,
  hasUnsavedChanges,
  refresh: () => refreshAll(true),
});

async function updateLanguageSetting(value: string) {
  if (value === languageSetting.value) {
    return;
  }
  const previous = languageSetting.value;
  languageSetting.value = value;
  try {
    await api.saveAppSettings({ language: value });
    setStatus(t('app.ready_msg'), t('app.ready_detail'));
  } catch (err) {
    languageSetting.value = previous;
    pushToast(`Failed to save language setting: ${err}`, 'error');
  }
}

async function updateThemeSetting(value: string) {
  if (value === themeSetting.value) {
    return;
  }
  const previous = themeSetting.value;
  themeSetting.value = value;
  try {
    await api.saveAppSettings({ theme: value });
  } catch (err) {
    themeSetting.value = previous;
    pushToast(`Failed to save theme setting: ${err}`, 'error');
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
  clearPendingUpdates();
}

onMounted(() => {
  refreshAll();
});
</script>
