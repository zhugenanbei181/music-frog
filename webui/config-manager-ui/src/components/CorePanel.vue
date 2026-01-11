<template>
  <PanelCard>
      <PanelHeader>
        <template #title>
          <PanelTitle :text="$t('core.title')" />
        </template>
        <template #actions>
          <button class="btn btn-outline btn-sm gap-2" @click="$emit('refresh')">
            {{ $t('core.refresh') }}
          </button>
        </template>
      </PanelHeader>

      <div class="mb-4 rounded-xl bg-sand-50 p-3 text-center">
        <p class="text-sm text-ink-700">
          {{ coreCurrent ? $t('core.current', { version: coreCurrent }) : $t('core.default') }}
        </p>
      </div>

      <div class="flex-1 overflow-y-auto max-h-75">
        <ul class="space-y-2">
          <li
            v-for="version in coreVersions"
            :key="version"
            class="flex items-center justify-between rounded-xl border border-sand-200 bg-white p-3 transition-colors hover:border-primary-200"
          >
            <div>
              <p class="font-mono text-sm font-medium">{{ version }}</p>
              <p class="text-xs text-ink-500">
                {{ version === coreCurrent ? $t('core.active') : $t('core.switchable') }}
              </p>
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
        <div v-if="coreVersions.length === 0" class="py-4 text-center empty-text">
          {{ $t('core.empty') }}
        </div>
      </div>
  </PanelCard>
</template>

<script setup lang="ts">
import PanelCard from './PanelCard.vue';
import PanelHeader from './PanelHeader.vue';
import PanelTitle from './PanelTitle.vue';

defineProps<{
  coreVersions: string[];
  coreCurrent: string | null;
}>();

defineEmits<{
  (e: 'refresh'): void;
  (e: 'activate', version: string): void;
}>();
</script>
