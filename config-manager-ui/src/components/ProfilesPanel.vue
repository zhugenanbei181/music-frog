<template>
  <section class="panel lg:col-span-4">
    <div class="flex items-center justify-between border-b border-ink-500/10 px-5 py-4">
      <div>
        <h2 class="panel-title">现有配置</h2>
        <p class="text-xs text-ink-500">当前 {{ activeCount }} / 共 {{ profiles.length }} 份</p>
      </div>
      <div class="flex flex-wrap gap-2">
        <button class="btn btn-ghost" @click="$emit('refresh')">刷新</button>
        <button class="btn btn-danger" @click="$emit('clear')">清空配置</button>
      </div>
    </div>
    <div class="px-5 py-4">
      <input
        :value="filter"
        class="input"
        placeholder="搜索配置名称"
        type="text"
        @input="$emit('update:filter', ($event.target as HTMLInputElement).value)"
      />
      <div
        class="mt-4 space-y-3 overflow-y-auto pr-1"
        :class="profiles.length > 6 ? 'max-h-[520px]' : ''"
      >
        <div
          v-for="profile in filteredProfiles"
          :key="profile.name"
          class="rounded-2xl border border-ink-500/10 bg-white px-4 py-3"
        >
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm font-semibold text-ink-900">{{ profile.name }}</p>
              <p class="text-xs text-ink-500">{{ profile.path }}</p>
            </div>
            <span
              class="badge"
              :class="profile.active ? 'badge-active' : 'badge-idle'"
            >
              {{ profile.active ? '当前' : '可用' }}
            </span>
          </div>
          <div class="mt-3 flex flex-wrap gap-2">
            <button class="btn btn-ghost" @click="$emit('load', profile.name)">编辑</button>
            <button class="btn btn-ghost" @click="$emit('open-external', profile.name)">外部编辑</button>
            <button
              class="btn btn-primary"
              :disabled="profile.active"
              @click="$emit('switch', profile.name)"
            >
              {{ profile.active ? '已启用' : '设为当前' }}
            </button>
            <button
              class="btn btn-danger"
              :disabled="profile.active"
              @click="$emit('delete', profile.name)"
            >
              删除
            </button>
            <button
              v-if="profile.subscription_url"
              class="btn btn-ghost"
              @click="$emit('update-now', profile.name)"
            >
              立即更新
            </button>
            <button
              v-if="profile.subscription_url"
              class="btn btn-ghost"
              @click="toggleSubscription(profile.name)"
            >
              订阅设置
            </button>
          </div>
          <div
            v-if="profile.controller_url"
            class="mt-3 text-xs text-ink-500"
          >
            控制接口：{{ profile.controller_url }}
            <span v-if="profile.controller_changed" class="text-sun-500">(已更新)</span>
          </div>
          <div
            v-if="profile.subscription_url"
            class="mt-3 flex flex-wrap items-center gap-2 text-xs text-ink-500"
          >
            <span class="badge badge-idle">订阅</span>
            <span>下次更新：{{ formatNextUpdate(profile.next_update) }}</span>
            <span v-if="profile.last_updated">上次更新：{{ formatLastUpdate(profile.last_updated) }}</span>
          </div>
          <div
            v-if="profile.subscription_url && subscriptionOpen[profile.name] && subscriptionDrafts[profile.name]"
            class="mt-3 rounded-xl border border-ink-500/10 bg-ink-50/40 p-3 text-xs text-ink-700"
          >
            <div class="flex flex-wrap items-center justify-between gap-2">
              <span>订阅地址：{{ maskSubscriptionUrl(profile.subscription_url) }}</span>
              <button class="btn btn-ghost" @click="subscriptionOpen[profile.name] = false">收起</button>
            </div>
            <div class="mt-2 flex flex-wrap items-center gap-3">
              <label class="flex items-center gap-2">
                <input
                  type="checkbox"
                  class="h-4 w-4"
                  v-model="subscriptionDrafts[profile.name].autoUpdate"
                />
                启用自动更新
              </label>
              <select
                class="input w-auto"
                :disabled="!subscriptionDrafts[profile.name].autoUpdate"
                v-model.number="subscriptionDrafts[profile.name].interval"
              >
                <option :value="12">12 小时</option>
                <option :value="24">24 小时</option>
                <option :value="48">48 小时</option>
                <option :value="168">7 天</option>
              </select>
              <button
                class="btn btn-primary"
                @click="saveSubscription(profile.name, profile.subscription_url)"
              >
                保存订阅设置
              </button>
            </div>
          </div>
        </div>
        <div v-if="!profiles.length" class="rounded-2xl border border-dashed border-ink-500/20 p-6 text-sm text-ink-500">
          暂无配置，请从右侧导入或新建。
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, reactive, watch } from 'vue';
import type { ProfileInfo } from '../types';

const props = defineProps<{
  profiles: ProfileInfo[];
  activeCount: number;
  filter: string;
}>();

const emit = defineEmits<{
  (event: 'refresh'): void;
  (event: 'clear'): void;
  (event: 'update:filter', value: string): void;
  (event: 'load', name: string): void;
  (event: 'open-external', name: string): void;
  (event: 'switch', name: string): void;
  (event: 'delete', name: string): void;
  (event: 'update-subscription', payload: { name: string; url: string; auto_update_enabled: boolean; update_interval_hours?: number | null }): void;
  (event: 'update-now', name: string): void;
}>();

const filteredProfiles = computed(() => {
  const keyword = props.filter.trim().toLowerCase();
  if (!keyword) return props.profiles;
  return props.profiles.filter((profile) => profile.name.toLowerCase().includes(keyword));
});

const subscriptionDrafts = reactive<Record<string, { autoUpdate: boolean; interval: number }>>({});
const subscriptionOpen = reactive<Record<string, boolean>>({});

watch(
  () => props.profiles,
  (profiles) => {
    profiles.forEach((profile) => {
      if (!profile.subscription_url) {
        return;
      }
      subscriptionDrafts[profile.name] = {
        autoUpdate: Boolean(profile.auto_update_enabled),
        interval: profile.update_interval_hours || 12,
      };
      if (subscriptionOpen[profile.name] === undefined) {
        subscriptionOpen[profile.name] = false;
      }
    });
  },
  { immediate: true },
);

function toggleSubscription(name: string) {
  subscriptionOpen[name] = !subscriptionOpen[name];
}

function saveSubscription(name: string, url: string | null | undefined) {
  if (!url) {
    return;
  }
  const draft = subscriptionDrafts[name];
  if (!draft) {
    return;
  }
  const payload = {
    name,
    url,
    auto_update_enabled: draft.autoUpdate,
    update_interval_hours: draft.autoUpdate ? draft.interval : null,
  };
  emit('update-subscription', payload);
}

function maskSubscriptionUrl(url: string | null | undefined) {
  if (!url) return '';
  const idx = url.indexOf('link/');
  if (idx === -1) return url;
  const start = idx + 5;
  const end = url.indexOf('?', start);
  const finalEnd = end === -1 ? url.length : end;
  return `${url.slice(0, start)}***${url.slice(finalEnd)}`;
}

function formatNextUpdate(value: string | null | undefined) {
  if (!value) return '未设置';
  const ts = Date.parse(value);
  if (Number.isNaN(ts)) return value;
  const diffMs = ts - Date.now();
  if (diffMs <= 0) return '即将执行';
  const minutes = Math.max(1, Math.round(diffMs / 60000));
  if (minutes < 60) return `${minutes} 分钟后`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} 小时后`;
  const days = Math.round(hours / 24);
  return `${days} 天后`;
}

function formatLastUpdate(value: string | null | undefined) {
  if (!value) return '未知';
  const ts = Date.parse(value);
  if (Number.isNaN(ts)) return value;
  return new Date(ts).toLocaleString();
}
</script>
