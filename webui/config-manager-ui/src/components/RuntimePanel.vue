<template>
  <PanelCard as="section" class="h-fit">
    <PanelHeader>
      <template #title>
        <div>
          <PanelTitle :text="$t('runtime.title')" />
          <p class="help-text">{{ $t('runtime.subtitle') }}</p>
        </div>
      </template>
      <template #actions>
        <button class="btn btn-secondary btn-sm" :disabled="loadingOverview" @click="$emit('refresh')">
          {{ $t('runtime.refresh') }}
        </button>
      </template>
    </PanelHeader>

    <div class="mt-4 rounded-xl border border-ink-500/10 bg-white/70 p-4">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <p class="label">{{ $t('runtime.runtime_status') }}</p>
          <p class="help-text">{{ runtimeStatusText }}</p>
          <p v-if="runtimeController" class="help-text">{{ runtimeController }}</p>
        </div>
        <div class="flex items-center gap-2">
          <span class="badge" :class="runtimeRunning ? 'badge-active' : 'badge-idle'">
            {{ runtimeRunning ? $t('runtime.running') : $t('runtime.stopped') }}
          </span>
          <button
            v-if="runtimeLifecycleEnabled && runtimeRunning"
            class="btn btn-secondary btn-xs"
            :disabled="runtimeActionPending || loadingRuntimeStatus"
            @click="$emit('stop-runtime')"
          >
            {{ $t('runtime.stop_runtime') }}
          </button>
          <button
            v-else-if="runtimeLifecycleEnabled"
            class="btn btn-secondary btn-xs"
            :disabled="runtimeActionPending || loadingRuntimeStatus"
            @click="$emit('start-runtime')"
          >
            {{ $t('runtime.start_runtime') }}
          </button>
        </div>
      </div>

      <div v-if="proxyControlEnabled" class="mt-4 grid gap-3 md:grid-cols-3">
        <div>
          <p class="help-text mb-1">{{ $t('runtime.mode') }}</p>
          <select
            :value="runtimeMode"
            class="select w-full"
            :disabled="runtimeActionPending || loadingRuntimeGroups || !runtimeRunning"
            @change="$emit('switch-mode', ($event.target as HTMLSelectElement).value)"
          >
            <option value="rule">{{ $t('runtime.mode_rule') }}</option>
            <option value="global">{{ $t('runtime.mode_global') }}</option>
            <option value="direct">{{ $t('runtime.mode_direct') }}</option>
            <option value="script">{{ $t('runtime.mode_script') }}</option>
          </select>
        </div>

        <div>
          <p class="help-text mb-1">{{ $t('runtime.proxy_group') }}</p>
          <select
            :value="selectedGroup"
            class="select w-full"
            :disabled="runtimeActionPending || loadingRuntimeGroups || !runtimeRunning"
            @change="$emit('update:selectedGroup', ($event.target as HTMLSelectElement).value)"
          >
            <option v-for="group in runtimeGroups" :key="group.name" :value="group.name">
              {{ group.name }}
            </option>
          </select>
        </div>

        <div>
          <p class="help-text mb-1">{{ $t('runtime.proxy_node') }}</p>
          <div class="flex items-center gap-2">
            <select
              :value="selectedProxy"
              class="select w-full"
              :disabled="runtimeActionPending || loadingRuntimeGroups || !runtimeRunning"
              @change="$emit('update:selectedProxy', ($event.target as HTMLSelectElement).value)"
            >
              <option v-for="proxy in selectedProxyOptions" :key="proxy" :value="proxy">
                {{ proxy }}
              </option>
            </select>
            <button
              class="btn btn-primary btn-xs"
              :disabled="runtimeActionPending || loadingRuntimeGroups || !runtimeRunning || !selectedProxy"
              @click="$emit('apply-proxy-selection')"
            >
              {{ $t('runtime.apply_proxy') }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <div class="mt-4 grid gap-3 md:grid-cols-3">
      <div class="rounded-xl border border-ink-500/10 bg-white/70 p-3">
        <p class="help-text">{{ $t('runtime.traffic_up_rate') }}</p>
        <p class="text-sm font-semibold text-ink-900">{{ formatRate(traffic?.up_rate) }}</p>
        <p class="help-text mt-1">
          {{ $t('runtime.traffic_peak', { value: formatRate(traffic?.up_peak) }) }}
        </p>
      </div>
      <div class="rounded-xl border border-ink-500/10 bg-white/70 p-3">
        <p class="help-text">{{ $t('runtime.traffic_down_rate') }}</p>
        <p class="text-sm font-semibold text-ink-900">{{ formatRate(traffic?.down_rate) }}</p>
        <p class="help-text mt-1">
          {{ $t('runtime.traffic_peak', { value: formatRate(traffic?.down_peak) }) }}
        </p>
      </div>
      <div class="rounded-xl border border-ink-500/10 bg-white/70 p-3">
        <p class="help-text">{{ $t('runtime.memory') }}</p>
        <p class="text-sm font-semibold text-ink-900">
          {{ formatBytes(memory?.inuse) }} / {{ formatBytes(memory?.oslimit) }}
        </p>
        <p class="help-text mt-1">
          {{ $t('runtime.connections_count', { count: traffic?.connections ?? connections.length }) }}
        </p>
      </div>
    </div>

    <div class="mt-4 rounded-xl border border-ink-500/10 bg-white/70 p-3">
      <div class="flex items-center justify-between gap-3">
        <div>
          <p class="label">{{ $t('runtime.ip') }}</p>
          <p class="help-text">{{ ipDisplay }}</p>
        </div>
        <button class="btn btn-secondary btn-xs" @click="$emit('refresh-ip')">
          {{ $t('runtime.refresh_ip') }}
        </button>
      </div>
    </div>

    <div class="mt-4 grid gap-4 xl:grid-cols-2">
      <section class="rounded-xl border border-ink-500/10 bg-white/70 p-4">
        <div class="flex items-center justify-between gap-2">
          <p class="label">{{ $t('runtime.connections') }}</p>
          <div class="flex items-center gap-2">
            <button class="btn btn-secondary btn-xs" :disabled="loadingConnections" @click="$emit('refresh-connections')">
              {{ $t('runtime.refresh') }}
            </button>
            <button
              class="btn btn-danger btn-xs"
              :disabled="closingAll || connections.length === 0"
              @click="$emit('close-all')"
            >
              {{ $t('runtime.close_all') }}
            </button>
          </div>
        </div>

        <div class="mt-3 grid gap-2 md:grid-cols-2">
          <div class="rounded-xl border border-ink-500/10 bg-white px-3 py-2">
            <p class="help-text">{{ $t('runtime.total_upload') }}</p>
            <p class="text-sm font-semibold text-ink-900">{{ formatBytes(uploadTotal) }}</p>
          </div>
          <div class="rounded-xl border border-ink-500/10 bg-white px-3 py-2">
            <p class="help-text">{{ $t('runtime.total_download') }}</p>
            <p class="text-sm font-semibold text-ink-900">{{ formatBytes(downloadTotal) }}</p>
          </div>
        </div>

        <input
          :value="connectionFilter"
          class="input mt-3"
          :placeholder="$t('runtime.filter_placeholder')"
          @input="$emit('update:connectionFilter', ($event.target as HTMLInputElement).value)"
        />

        <div class="mt-3 max-h-80 overflow-auto">
          <table v-if="filteredConnections.length > 0" class="w-full text-xs">
            <thead class="text-left text-ink-500">
              <tr>
                <th class="px-2 py-1.5">{{ $t('runtime.host') }}</th>
                <th class="px-2 py-1.5">{{ $t('runtime.process') }}</th>
                <th class="px-2 py-1.5">{{ $t('runtime.transfer') }}</th>
                <th class="px-2 py-1.5">{{ $t('runtime.action') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="connection in filteredConnections" :key="connection.id" class="border-t border-ink-500/10 align-top">
                <td class="px-2 py-2">
                  <p class="font-semibold text-ink-900 break-all">{{ connection.metadata?.host || '-' }}</p>
                  <p class="help-text">{{ connection.metadata?.sourceIP || '-' }} -> {{ connection.metadata?.destinationIP || '-' }}</p>
                  <p class="help-text">{{ connection.rule || '-' }}</p>
                </td>
                <td class="px-2 py-2">
                  <p class="text-ink-700 break-all">{{ shortProcess(connection.metadata?.processPath) }}</p>
                  <p class="help-text">{{ connection.metadata?.network || '-' }} / {{ connection.metadata?.type || '-' }}</p>
                  <p class="help-text">{{ formatTime(connection.start) }}</p>
                </td>
                <td class="px-2 py-2">
                  <p class="text-ink-700">↑ {{ formatBytes(connection.upload) }}</p>
                  <p class="text-ink-700">↓ {{ formatBytes(connection.download) }}</p>
                </td>
                <td class="px-2 py-2">
                  <button
                    class="btn btn-danger btn-xs"
                    :disabled="closingConnectionId === connection.id"
                    @click="$emit('close-one', connection.id)"
                  >
                    {{ $t('runtime.close') }}
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
          <p v-else class="empty-text py-6 text-center">{{ $t('runtime.connections_empty') }}</p>
        </div>
      </section>

      <section class="rounded-xl border border-ink-500/10 bg-white/70 p-4">
        <div class="flex items-center justify-between gap-2">
          <p class="label">{{ $t('runtime.logs') }}</p>
          <div class="flex items-center gap-2">
            <span class="badge" :class="streamConnected ? 'badge-active' : 'badge-idle'">
              {{ streamConnected ? $t('runtime.stream_connected') : $t('runtime.stream_disconnected') }}
            </span>
            <button class="btn btn-ghost btn-xs" @click="$emit('clear-logs')">
              {{ $t('runtime.clear_logs') }}
            </button>
          </div>
        </div>

        <div class="mt-3 flex items-center gap-2">
          <label class="help-text">{{ $t('runtime.log_level') }}</label>
          <select
            :value="logLevel"
            class="select max-w-[180px]"
            @change="$emit('update:logLevel', ($event.target as HTMLSelectElement).value as RuntimeLogLevel)"
          >
            <option v-for="option in logLevelOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </div>

        <div class="mt-3 h-80 overflow-auto rounded-xl bg-ink-900 p-3 font-mono text-xs text-sand-50">
          <p v-if="logs.length === 0" class="empty-text text-sand-100/80">{{ $t('runtime.logs_empty') }}</p>
          <p v-for="(line, idx) in logs" :key="idx" class="break-all leading-5">{{ line }}</p>
        </div>
      </section>

      <section class="rounded-xl border border-ink-500/10 bg-white/70 p-4 xl:col-span-2">
        <div class="flex items-center justify-between gap-2">
          <p class="label">{{ $t('runtime.delay_title') }}</p>
          <div class="flex items-center gap-2">
            <button class="btn btn-secondary btn-xs" :disabled="loadingDelays" @click="$emit('refresh-delays')">
              {{ $t('runtime.refresh') }}
            </button>
            <button
              class="btn btn-primary btn-xs"
              :disabled="testingAllDelays || loadingDelays || delayNodes.length === 0"
              @click="$emit('test-all-delays')"
            >
              {{ $t('runtime.delay_test_all') }}
            </button>
          </div>
        </div>
        <p class="help-text mt-2">
          {{ $t('runtime.delay_hint', { url: delayTestUrl, timeout: delayTimeoutMs }) }}
        </p>

        <div class="mt-3 flex items-center gap-2">
          <label class="help-text">{{ $t('runtime.delay_sort') }}</label>
          <select
            :value="delaySort"
            class="select max-w-[200px]"
            @change="$emit('update:delaySort', ($event.target as HTMLSelectElement).value as DelaySortKey)"
          >
            <option v-for="option in delaySortOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </div>

        <div class="mt-3 max-h-80 overflow-auto">
          <table v-if="delayNodes.length > 0" class="w-full text-xs">
            <thead class="text-left text-ink-500">
              <tr>
                <th class="px-2 py-1.5">{{ $t('runtime.delay_proxy') }}</th>
                <th class="px-2 py-1.5">{{ $t('runtime.delay_latest') }}</th>
                <th class="px-2 py-1.5">{{ $t('runtime.action') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="node in delayNodes" :key="node.name" class="border-t border-ink-500/10 align-top">
                <td class="px-2 py-2">
                  <p class="font-semibold text-ink-900 break-all">{{ node.name }}</p>
                  <p class="help-text">{{ node.proxy_type }}</p>
                </td>
                <td class="px-2 py-2">
                  <p class="text-ink-700">{{ formatDelay(node.delay_ms) }}</p>
                  <p class="help-text">{{ formatTime(node.tested_at || undefined) }}</p>
                </td>
                <td class="px-2 py-2">
                  <button
                    class="btn btn-secondary btn-xs"
                    :disabled="testingAllDelays || loadingDelays || testingDelayProxy === node.name"
                    @click="$emit('test-delay', node.name)"
                  >
                    {{ $t('runtime.delay_test_one') }}
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
          <p v-else class="empty-text py-6 text-center">{{ $t('runtime.delay_empty') }}</p>
        </div>
      </section>
    </div>

    <PanelFooter>
      <div class="grid gap-3 md:grid-cols-3">
        <FormSwitch
          :model-value="autoRefresh"
          :label="$t('runtime.auto_refresh')"
          :description="$t('runtime.auto_refresh_desc')"
          @update:model-value="$emit('update:autoRefresh', $event)"
        />
        <FormSwitch
          v-if="systemProxyControlEnabled"
          :model-value="systemProxyEnabled"
          :label="$t('runtime.system_proxy')"
          :description="$t('runtime.system_proxy_desc')"
          :disabled="settingSystemProxy"
          @update:model-value="$emit('toggle-system-proxy', $event)"
        />
        <FormSwitch
          v-if="autostartControlEnabled"
          :model-value="autostartEnabled"
          :label="$t('runtime.autostart')"
          :description="$t('runtime.autostart_desc')"
          :disabled="settingAutostart"
          @update:model-value="$emit('toggle-autostart', $event)"
        />
      </div>
    </PanelFooter>
  </PanelCard>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type {
  RuntimeConnection,
  RuntimeIpCheckResponse,
  RuntimeLogLevel,
  RuntimeMemoryData,
  RuntimeProxyDelayNode,
  RuntimeProxyGroupEntry,
  RuntimeTrafficSnapshot,
} from '../types';
import FormSwitch from './FormSwitch.vue';
import PanelCard from './PanelCard.vue';
import PanelFooter from './PanelFooter.vue';
import PanelHeader from './PanelHeader.vue';
import PanelTitle from './PanelTitle.vue';

type DelaySortKey = 'delay_asc' | 'delay_desc' | 'name_asc' | 'name_desc';

const props = defineProps<{
  autoRefresh: boolean;
  autostartControlEnabled: boolean;
  autostartEnabled: boolean;
  closingAll: boolean;
  closingConnectionId: string;
  connectionFilter: string;
  connections: RuntimeConnection[];
  delayNodes: RuntimeProxyDelayNode[];
  delaySort: DelaySortKey;
  delayTestUrl: string;
  delayTimeoutMs: number;
  downloadTotal: number;
  filteredConnections: RuntimeConnection[];
  ipInfo: RuntimeIpCheckResponse | null;
  loadingConnections: boolean;
  loadingDelays: boolean;
  loadingOverview: boolean;
  loadingRuntimeGroups: boolean;
  loadingRuntimeStatus: boolean;
  logLevel: RuntimeLogLevel;
  logs: string[];
  memory: RuntimeMemoryData | null;
  proxyControlEnabled: boolean;
  runtimeActionPending: boolean;
  runtimeController: string;
  runtimeGroups: RuntimeProxyGroupEntry[];
  runtimeLifecycleEnabled: boolean;
  runtimeMode: string;
  runtimeRunning: boolean;
  selectedGroup: string;
  selectedProxy: string;
  selectedProxyOptions: string[];
  settingAutostart: boolean;
  settingSystemProxy: boolean;
  streamConnected: boolean;
  systemProxyControlEnabled: boolean;
  systemProxyEnabled: boolean;
  testingAllDelays: boolean;
  testingDelayProxy: string;
  traffic: RuntimeTrafficSnapshot | null;
  uploadTotal: number;
}>();

defineEmits<{
  (e: 'update:autoRefresh', value: boolean): void;
  (e: 'update:connectionFilter', value: string): void;
  (e: 'update:delaySort', value: DelaySortKey): void;
  (e: 'update:logLevel', value: RuntimeLogLevel): void;
  (e: 'update:selectedGroup', value: string): void;
  (e: 'update:selectedProxy', value: string): void;
  (e: 'clear-logs'): void;
  (e: 'start-runtime'): void;
  (e: 'stop-runtime'): void;
  (e: 'switch-mode', mode: string): void;
  (e: 'apply-proxy-selection'): void;
  (e: 'toggle-system-proxy', enabled: boolean): void;
  (e: 'toggle-autostart', enabled: boolean): void;
  (e: 'close-all'): void;
  (e: 'close-one', id: string): void;
  (e: 'refresh'): void;
  (e: 'refresh-connections'): void;
  (e: 'refresh-delays'): void;
  (e: 'refresh-ip'): void;
  (e: 'test-all-delays'): void;
  (e: 'test-delay', proxy: string): void;
}>();

const { t } = useI18n();

const logLevelOptions = computed(() => [
  { value: 'debug' as RuntimeLogLevel, label: t('runtime.level_debug') },
  { value: 'info' as RuntimeLogLevel, label: t('runtime.level_info') },
  { value: 'warning' as RuntimeLogLevel, label: t('runtime.level_warning') },
  { value: 'error' as RuntimeLogLevel, label: t('runtime.level_error') },
  { value: 'silent' as RuntimeLogLevel, label: t('runtime.level_silent') },
]);

const delaySortOptions = computed(() => [
  { value: 'delay_asc' as DelaySortKey, label: t('runtime.delay_sort_delay_asc') },
  { value: 'delay_desc' as DelaySortKey, label: t('runtime.delay_sort_delay_desc') },
  { value: 'name_asc' as DelaySortKey, label: t('runtime.delay_sort_name_asc') },
  { value: 'name_desc' as DelaySortKey, label: t('runtime.delay_sort_name_desc') },
]);

const ipDisplay = computed(() => {
  if (!props.ipInfo || !props.ipInfo.ip) {
    return t('runtime.ip_empty');
  }
  const location = [props.ipInfo.country, props.ipInfo.region, props.ipInfo.city]
    .filter((value) => value && value.trim())
    .join(' / ');
  return location ? `${props.ipInfo.ip} (${location})` : props.ipInfo.ip;
});

const runtimeStatusText = computed(() => {
  if (props.loadingRuntimeStatus) {
    return t('runtime.status_loading');
  }
  return props.runtimeRunning ? t('runtime.status_running') : t('runtime.status_stopped');
});

function formatBytes(value?: number | null) {
  const size = Number(value || 0);
  if (!Number.isFinite(size) || size < 0) {
    return '0 B';
  }
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let current = size;
  let unitIndex = 0;
  while (current >= 1024 && unitIndex < units.length - 1) {
    current /= 1024;
    unitIndex += 1;
  }
  const display = current >= 10 || unitIndex === 0 ? current.toFixed(0) : current.toFixed(1);
  return `${display} ${units[unitIndex]}`;
}

function formatRate(value?: number | null) {
  return `${formatBytes(value)}/s`;
}

function formatDelay(value?: number | null) {
  const delay = Number(value);
  if (!Number.isFinite(delay) || delay < 0) {
    return '-';
  }
  return `${Math.round(delay)} ms`;
}

function formatTime(value?: string) {
  if (!value) {
    return '-';
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function shortProcess(value?: string) {
  if (!value) {
    return '-';
  }
  const normalized = value.replaceAll('\\\\', '/');
  const parts = normalized.split('/').filter(Boolean);
  return parts.length === 0 ? normalized : parts[parts.length - 1];
}
</script>
