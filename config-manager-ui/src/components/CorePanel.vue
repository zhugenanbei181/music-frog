<template>
  <div class="panel h-full flex flex-col p-6">
    <div class="mb-4 flex items-center justify-between">
      <h2 class="panel-title">{{ $t('core.title') }}</h2>
      <button class="btn btn-ghost" @click="$emit('refresh')">{{ $t('core.refresh') }}</button>
    </div>
    
    <div class="mb-4 rounded-lg bg-sand-50 p-3 text-center">
      <p class="text-sm text-ink-700">
        {{ coreCurrent ? $t('core.current', { version: coreCurrent }) : $t('core.default') }}
      </p>
    </div>

    <div class="flex-1 overflow-y-auto max-h-[300px]">
      <ul class="space-y-2">
        <li
          v-for="version in coreVersions"
          :key="version"
          class="flex items-center justify-between rounded-lg border border-sand-200 p-3 hover:border-primary-200"
        >
          <div>
            <p class="font-mono text-sm font-medium">{{ version }}</p>
            <p class="text-xs text-ink-500">{{ version === coreCurrent ? $t('core.active') : $t('core.switchable') }}</p>
          </div>
          <button
            class="btn btn-xs"
            :class="version === coreCurrent ? 'btn-secondary' : 'btn-primary'"
            :disabled="version === coreCurrent"
            @click="$emit('activate', version)"
          >
            {{ version === coreCurrent ? $t('core.status_current') : $t('core.status_use') }}
          </button>
        </li>
      </ul>
      <div v-if="coreVersions.length === 0" class="py-4 text-center text-xs text-ink-500">
        {{ $t('core.empty') }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  coreVersions: string[];
  coreCurrent: string | null;
}>();

defineEmits<{
  (e: 'refresh'): void;
  (e: 'activate', version: string): void;
}>();
</script>
