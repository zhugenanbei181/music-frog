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
          </div>
          <div
            v-if="profile.controller_url"
            class="mt-3 text-xs text-ink-500"
          >
            控制接口：{{ profile.controller_url }}
            <span v-if="profile.controller_changed" class="text-sun-500">(已更新)</span>
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
import { computed } from 'vue';
import type { ProfileInfo } from '../types';

const props = defineProps<{
  profiles: ProfileInfo[];
  activeCount: number;
  filter: string;
}>();

defineEmits<{
  (event: 'refresh'): void;
  (event: 'clear'): void;
  (event: 'update:filter', value: string): void;
  (event: 'load', name: string): void;
  (event: 'open-external', name: string): void;
  (event: 'switch', name: string): void;
  (event: 'delete', name: string): void;
}>();

const filteredProfiles = computed(() => {
  const keyword = props.filter.trim().toLowerCase();
  if (!keyword) return props.profiles;
  return props.profiles.filter((profile) => profile.name.toLowerCase().includes(keyword));
});
</script>
