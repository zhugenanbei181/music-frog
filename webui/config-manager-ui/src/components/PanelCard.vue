<template>
  <component :is="as" class="panel h-full flex flex-col">
    <div class="p-6 flex flex-col h-full">
      <slot />
    </div>
  </component>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    as?: string;
  }>(),
  {
    as: 'div',
  },
);
</script>
