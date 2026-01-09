<template>
  <div class="fixed bottom-6 right-6 z-50 space-y-2">
    <div
      v-for="toast in toasts"
      :key="toast.id"
      class="panel px-4 py-3 text-sm"
      :class="toast.tone === 'error' ? 'border-red-200 text-red-700' : 'text-ink-700'"
    >
      {{ toast.message }}
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  toasts: Array<{ id: number; message: string; tone: 'info' | 'error' }>;
}>();
</script>
