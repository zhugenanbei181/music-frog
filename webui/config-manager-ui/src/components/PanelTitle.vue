<template>
  <div class="flex items-center gap-2">
    <span v-if="$slots.icon" :class="iconClass">
      <slot name="icon" />
    </span>
    <span :class="[sizeClass, 'font-bold', toneClass]">{{ text }}</span>
  </div>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    text: string;
    toneClass?: string;
    sizeClass?: string;
    iconClass?: string;
  }>(),
  {
    toneClass: 'text-ink-900',
    sizeClass: 'text-xl',
    iconClass: 'text-primary',
  },
);
</script>
