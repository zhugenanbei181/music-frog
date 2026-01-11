<script setup lang="ts">
defineProps<{
  modelValue: boolean;
  label?: string;
  description?: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const toggle = (event: Event) => {
  const target = event.target as HTMLInputElement;
  emit('update:modelValue', target.checked);
};
</script>

<template>
  <label class="inline-flex items-center gap-3 cursor-pointer group">
    <div
      class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200 ease-in-out focus-within:ring-2 focus-within:ring-accent-200 focus-within:ring-offset-2"
      :class="modelValue ? 'bg-accent-500' : 'bg-ink-500/20'"
    >
      <input
        type="checkbox"
        class="sr-only"
        :checked="modelValue"
        @change="toggle"
      />
      <span
        aria-hidden="true"
        class="pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
        :class="modelValue ? 'translate-x-4.5' : 'translate-x-0.5'"
      />
    </div>
    <div v-if="label || description" class="flex flex-col">
      <span v-if="label" class="text-sm font-semibold text-ink-700 select-none">{{ label }}</span>
      <span v-if="description" class="text-xs text-ink-500 select-none">{{ description }}</span>
    </div>
  </label>
</template>
