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
        <div class="rounded-2xl border border-ink-500/10 bg-white/80 px-4 py-3 text-sm">
          <p class="font-semibold text-ink-900">{{ statusMessage }}</p>
          <p class="help-text">{{ statusDetail }}</p>
        </div>
        <button class="btn btn-outline btn-sm gap-2" @click="$emit('toggle-lang')">
          <span class="text-lg">🌐</span>
        </button>
        <button class="btn btn-outline btn-sm gap-2" @click="$emit('refresh')">{{ $t('header.refresh') }}</button>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
defineProps<{
  statusMessage: string;
  statusDetail: string;
}>();

defineEmits<{
  (event: 'refresh'): void;
  (event: 'toggle-lang'): void;
}>();
</script>
