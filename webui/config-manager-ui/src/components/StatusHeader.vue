<template>
  <header class="panel px-6 py-5">
    <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
      <div class="space-y-1">
        <p class="text-sm uppercase tracking-[0.2em] text-ink-500">MusicFrog Despicable Infiltrator</p>
        <h1 class="text-2xl font-semibold text-ink-900">{{ $t('header.title') }}</h1>
        <p class="help-text">
          {{ $t('header.subtitle') }}
        </p>
      </div>
      <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div class="panel px-4 py-3 text-sm">
          <p class="font-semibold text-ink-900">{{ statusMessage }}</p>
          <p class="help-text">{{ statusDetail }}</p>
        </div>
        <div class="grid gap-2 sm:grid-cols-2">
          <SelectMenu
            class="min-w-[170px]"
            :label="$t('header.language')"
            :model-value="languageValue"
            :options="languageOptions"
            @update:modelValue="$emit('update:language', $event)"
          />
          <SelectMenu
            class="min-w-[170px]"
            :label="$t('header.theme')"
            :model-value="themeValue"
            :options="themeOptions"
            @update:modelValue="$emit('update:theme', $event)"
          />
        </div>
        <div class="flex flex-wrap gap-2">
        <button
          v-if="hasPendingUpdates"
          class="btn btn-primary btn-sm gap-2"
          @click="$emit('refresh-updates')"
        >
          {{ $t('header.refresh_updates') }}
        </button>
        <button class="btn btn-outline btn-sm gap-2" @click="$emit('refresh')">{{ $t('header.refresh') }}</button>
        </div>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
import SelectMenu from './SelectMenu.vue';

defineProps<{
  statusMessage: string;
  statusDetail: string;
  languageValue: string;
  themeValue: string;
  languageOptions: Array<{ value: string; label: string }>;
  themeOptions: Array<{ value: string; label: string }>;
  hasPendingUpdates?: boolean;
}>();

defineEmits<{
  (event: 'refresh'): void;
  (event: 'refresh-updates'): void;
  (event: 'update:language', value: string): void;
  (event: 'update:theme', value: string): void;
}>();
</script>
