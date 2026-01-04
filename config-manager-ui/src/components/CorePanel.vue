<template>
  <section class="panel px-5 py-4">
    <div class="flex items-center justify-between">
      <h2 class="panel-title">内核版本</h2>
      <button class="btn btn-ghost" @click="$emit('refresh')">刷新</button>
    </div>
    <div class="mt-4 space-y-2">
      <p class="text-sm text-ink-700">当前内核：{{ coreCurrent || '默认内核' }}</p>
      <div class="max-h-[280px] space-y-2 overflow-y-auto pr-1">
        <div
          v-for="version in coreVersions"
          :key="version"
          class="flex items-center justify-between rounded-xl border border-ink-500/10 bg-white px-3 py-2"
        >
          <div>
            <p class="text-sm font-semibold text-ink-900">{{ version }}</p>
            <p class="text-xs text-ink-500">{{ version === coreCurrent ? '已启用' : '可切换' }}</p>
          </div>
          <button
            class="btn btn-primary"
            :disabled="version === coreCurrent"
            @click="$emit('activate', version)"
          >
            {{ version === coreCurrent ? '当前' : '启用' }}
          </button>
        </div>
        <div v-if="!coreVersions.length" class="text-sm text-ink-500">
          尚未下载内核版本，将使用内置内核。
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  coreVersions: string[];
  coreCurrent: string | null;
}>();

defineEmits<{
  (event: 'refresh'): void;
  (event: 'activate', version: string): void;
}>();
</script>
