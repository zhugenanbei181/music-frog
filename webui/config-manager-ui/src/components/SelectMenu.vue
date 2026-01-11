<template>
  <div class="flex flex-col gap-1">
    <span class="text-[11px] font-semibold uppercase tracking-[0.25em] text-ink-500">
      {{ label }}
    </span>
    <select
      class="select"
      :value="modelValue"
      :aria-label="label"
      @change="onChange"
    >
      <option v-for="option in options" :key="option.value" :value="option.value">
        {{ option.label }}
      </option>
    </select>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  label: string;
  modelValue: string;
  options: Array<{ value: string; label: string }>;
}>();

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void;
}>();

function onChange(event: Event) {
  const target = event.target as HTMLSelectElement | null;
  emit('update:modelValue', target?.value ?? '');
}
</script>
