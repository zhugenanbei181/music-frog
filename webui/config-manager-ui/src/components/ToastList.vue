<template>
  <div class="fixed bottom-6 right-6 z-50 space-y-2">
    <div
      v-for="toast in toasts"
      :key="toast.id"
      class="panel px-4 py-3 text-sm"
      :class="toneClass(toast.tone)"
    >
      {{ toast.message }}
    </div>
  </div>
</template>

<script setup lang="ts">
type ToastTone = 'info' | 'error' | 'success' | 'warning';

defineProps<{
  toasts: Array<{ id: number; message: string; tone: ToastTone }>;
}>();

function toneClass(tone: ToastTone) {
  switch (tone) {
    case 'error':
      return 'border-red-200 text-red-700';
    case 'success':
      return 'border-emerald-200 text-emerald-700';
    case 'warning':
      return 'border-amber-200 text-amber-700';
    default:
      return 'text-ink-700';
  }
}
</script>
