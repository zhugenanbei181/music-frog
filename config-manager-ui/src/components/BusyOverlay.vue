<template>
  <div
    v-if="visible"
    class="fixed inset-0 z-40 flex items-center justify-center bg-ink-900/30 px-6"
  >
    <div class="panel w-full max-w-md px-6 py-4">
      <div class="flex items-center gap-3">
        <span class="h-3 w-3 rounded-full bg-accent-400 shadow-sm animate-pulse"></span>
        <div>
          <p class="text-sm font-semibold text-ink-900">{{ message }}</p>
          <p class="text-xs text-ink-500">{{ detail }}</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  visible: boolean;
  message: string;
  detail: string;
}>();
</script>
