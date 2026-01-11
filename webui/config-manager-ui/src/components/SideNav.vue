<template>
  <PanelCard as="aside" class="h-fit">
    <PanelHeader>
      <template #title>
        <PanelTitle :text="title" />
      </template>
    </PanelHeader>
    <nav class="flex gap-2 overflow-x-auto pb-1 lg:flex-col lg:overflow-visible lg:pb-0">
      <button
        v-for="item in items"
        :key="item.id"
        class="btn btn-sm whitespace-nowrap lg:justify-start"
        :class="item.id === modelValue ? 'btn-primary' : 'btn-secondary'"
        @click="$emit('update:modelValue', item.id)"
      >
        {{ item.label }}
      </button>
    </nav>
  </PanelCard>
</template>

<script setup lang="ts">
import PanelCard from './PanelCard.vue';
import PanelHeader from './PanelHeader.vue';
import PanelTitle from './PanelTitle.vue';

defineProps<{
  title: string;
  modelValue: string;
  items: Array<{ id: string; label: string }>;
}>();

defineEmits<{
  (e: 'update:modelValue', value: string): void;
}>();
</script>
