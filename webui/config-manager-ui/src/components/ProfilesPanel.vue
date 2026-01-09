<template>
  <section class="lg:col-span-4 panel h-fit">
    <div class="flex items-center justify-between border-b border-sand-200 px-5 py-4">
      <div>
        <h2 class="panel-title">{{ $t('profiles.title') }}</h2>
        <p class="text-xs text-ink-500">{{ $t('profiles.count', { active: activeCount, total: profiles.length }) }}</p>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-ghost" @click="$emit('refresh')">{{ $t('profiles.refresh') }}</button>
        <button class="btn btn-danger" @click="$emit('clear')">{{ $t('profiles.clear') }}</button>
      </div>
    </div>

    <div class="p-4">
      <input
        type="text"
        v-model="filterModel"
        :placeholder="$t('profiles.search_placeholder')"
        class="input w-full"
      />
    </div>

    <div class="max-h-[600px] overflow-y-auto p-4 pt-0">
      <ul class="space-y-3">
        <li
          v-for="profile in filteredProfiles"
          :key="profile.name"
          class="rounded-xl border p-4 transition-all"
          :class="
            profile.active
              ? 'border-primary-500 bg-primary-50 shadow-sm'
              : 'border-sand-200 bg-white hover:border-primary-200'
          "
        >
          <div class="mb-3 flex items-start justify-between">
            <div>
              <div class="flex items-center gap-2">
                <h3 class="font-bold text-ink-900">{{ profile.name }}</h3>
                <span
                  class="badge"
                  :class="profile.active ? 'badge-active' : 'badge-idle'"
                >
                  {{ profile.active ? $t('profiles.current') : $t('profiles.available') }}
                </span>
              </div>
              <div class="mt-2 flex gap-2">
                <button class="btn btn-ghost" @click="$emit('load', profile.name)">{{ $t('profiles.edit') }}</button>
                <button class="btn btn-ghost" @click="$emit('open-external', profile.name)">{{ $t('profiles.external_edit') }}</button>
              </div>
            </div>
            <div class="flex flex-col gap-2">
              <button
                class="btn btn-sm"
                :class="profile.active ? 'btn-primary' : 'btn-secondary'"
                :disabled="profile.active"
                @click="$emit('switch', profile.name)"
              >
                {{ profile.active ? $t('profiles.active') : $t('profiles.set_active') }}
              </button>
              <button
                class="btn btn-sm btn-danger"
                v-if="!profile.active"
                @click="$emit('delete', profile.name)"
              >
                {{ $t('profiles.delete') }}
              </button>
            </div>
          </div>

          <div v-if="profile.active" class="mt-3 border-t border-primary-200/50 pt-3">
            <button
              class="btn btn-sm btn-primary w-full"
              @click="$emit('update-now', profile.name)"
              v-if="profile.subscription_url"
            >
              {{ $t('profiles.update_now') }}
            </button>
            
            <button
              class="btn btn-sm btn-ghost w-full mt-2"
              @click="toggleSubscription(profile.name)"
              v-if="profile.subscription_url"
            >
              {{ $t('profiles.settings') }}
            </button>

            <div class="mt-2 text-xs text-ink-500">
              <p v-if="profile.controller_url">
                {{ $t('profiles.controller', { url: profile.controller_url }) }}
                <span v-if="profile.controller_changed" class="text-sun-500">{{ $t('profiles.controller_updated') }}</span>
              </p>
            </div>
          </div>

          <div v-if="profile.subscription_url" class="mt-3 text-xs text-ink-500">
            <div class="flex items-center gap-2">
              <span class="badge badge-idle">{{ $t('profiles.subscription') }}</span>
              <span>{{ $t('profiles.next_update', { time: formatNextUpdate(profile.next_update) }) }}</span>
            </div>
            <p class="mt-1 pl-1">
              <span v-if="profile.last_updated">{{ $t('profiles.last_update', { time: formatLastUpdate(profile.last_updated) }) }}</span>
            </p>
          </div>

          <!-- Subscription Settings Form -->
          <div v-if="subscriptionOpen[profile.name]" class="mt-4 rounded-lg bg-sand-50 p-3">
            <div class="mb-2 flex items-center justify-between">
              <span class="text-xs font-mono text-ink-400 break-all">{{ $t('profiles.sub_url', { url: maskSubscriptionUrl(profile.subscription_url) }) }}</span>
              <button class="btn btn-ghost" @click="subscriptionOpen[profile.name] = false">{{ $t('profiles.collapse') }}</button>
            </div>
            
            <div class="space-y-3">
              <div class="form-control">
                <label class="label cursor-pointer justify-start gap-2">
                  <input 
                    type="checkbox" 
                    class="checkbox checkbox-sm" 
                    :checked="profile.auto_update_enabled"
                    @change="(e) => updateSubField(profile.name, 'auto_update_enabled', (e.target as HTMLInputElement).checked)"
                  />
                  <span class="label-text">{{ $t('profiles.enable_auto_update') }}</span>
                </label>
              </div>

              <div class="form-control" v-if="getEditState(profile).auto_update_enabled">
                <select 
                  class="select select-sm w-full"
                  :value="getEditState(profile).update_interval_hours"
                  @change="(e) => updateSubField(profile.name, 'update_interval_hours', Number((e.target as HTMLSelectElement).value))"
                >
                  <option :value="12">{{ $t('profiles.hours_12') }}</option>
                  <option :value="24">{{ $t('profiles.hours_24') }}</option>
                  <option :value="48">{{ $t('profiles.hours_48') }}</option>
                  <option :value="168">{{ $t('profiles.days_7') }}</option>
                </select>
              </div>

              <button 
                class="btn btn-sm btn-primary w-full"
                @click="saveSubscription(profile)"
              >
                {{ $t('profiles.save_settings') }}
              </button>
            </div>
          </div>
        </li>
      </ul>
      <div v-if="profiles.length === 0" class="py-8 text-center text-ink-400">
        {{ $t('profiles.empty') }}
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, reactive } from 'vue';
import { useI18n } from 'vue-i18n';
import type { ProfileInfo } from '../types';

const props = defineProps<{
  profiles: ProfileInfo[];
  activeCount: number;
  filter: string;
}>();

const emit = defineEmits<{
  (e: 'update:filter', value: string): void;
  (e: 'refresh'): void;
  (e: 'clear'): void;
  (e: 'load', name: string): void;
  (e: 'open-external', name: string): void;
  (e: 'switch', name: string): void;
  (e: 'delete', name: string): void;
  (e: 'update-subscription', payload: { name: string, url: string, auto_update_enabled: boolean, update_interval_hours?: number | null }): void;
  (e: 'update-now', name: string): void;
}>();

const { t } = useI18n();

const filterModel = computed({
  get: () => props.filter,
  set: (val) => emit('update:filter', val),
});

const filteredProfiles = computed(() => {
  if (!filterModel.value) return props.profiles;
  const lower = filterModel.value.toLowerCase();
  return props.profiles.filter((p) => p.name.toLowerCase().includes(lower));
});

function formatNextUpdate(value?: string | null) {
  if (!value) return t('profiles.time_not_set');
  const date = new Date(value);
  const now = new Date();
  const diffMs = date.getTime() - now.getTime();
  if (diffMs <= 0) return t('profiles.time_soon');
  
  const minutes = Math.ceil(diffMs / 60000);
  if (minutes < 60) return t('profiles.time_mins', { m: minutes });
  
  const hours = Math.ceil(minutes / 60);
  if (hours < 24) return t('profiles.time_hours', { h: hours });
  
  const days = Math.ceil(hours / 24);
  return t('profiles.time_days', { d: days });
}

function formatLastUpdate(value?: string | null) {
  if (!value) return t('profiles.time_unknown');
  return new Date(value).toLocaleString();
}

function maskSubscriptionUrl(url?: string | null) {
  if (!url) return '';
  if (url.length < 20) return url;
  return url.substring(0, 8) + '...' + url.substring(url.length - 8);
}

const subscriptionOpen = reactive<Record<string, boolean>>({});
const editState = reactive<Record<string, { auto_update_enabled: boolean, update_interval_hours: number }>>({});

function toggleSubscription(name: string) {
  subscriptionOpen[name] = !subscriptionOpen[name];
  if (subscriptionOpen[name]) {
    const profile = props.profiles.find(p => p.name === name);
    if (profile) {
      editState[name] = {
        auto_update_enabled: profile.auto_update_enabled,
        update_interval_hours: profile.update_interval_hours || 24,
      };
    }
  }
}

function getEditState(profile: ProfileInfo) {
  if (!editState[profile.name]) {
    return {
      auto_update_enabled: profile.auto_update_enabled,
      update_interval_hours: profile.update_interval_hours || 24,
    };
  }
  return editState[profile.name];
}

function updateSubField(name: string, field: 'auto_update_enabled' | 'update_interval_hours', value: any) {
  if (!editState[name]) return;
  // @ts-ignore
  editState[name][field] = value;
}

function saveSubscription(profile: ProfileInfo) {
  const state = editState[profile.name];
  if (!state || !profile.subscription_url) return;
  emit('update-subscription', {
    name: profile.name,
    url: profile.subscription_url,
    auto_update_enabled: state.auto_update_enabled,
    update_interval_hours: state.update_interval_hours,
  });
  subscriptionOpen[profile.name] = false;
}
</script>