<template>
  <PanelCard>
      <PanelHeader>
        <template #title>
          <PanelTitle :text="$t('import_local.title')" />
        </template>
        <template #actions>
          <span class="badge badge-idle">{{ $t('import_local.badge') }}</span>
        </template>
      </PanelHeader>
      <div class="space-y-4 grow">
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ $t('import_local.file_label') }}</span>
          </label>
          <input
            type="file"
            accept=".yaml,.yml,.txt"
            @change="onFileChange"
            class="file-input file-input-bordered file-input-sm w-full"
          />
        </div>
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ $t('import_local.name_label') }}</span>
          </label>
          <input
            type="text"
            :value="name"
            @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
            :placeholder="$t('import_local.name_placeholder')"
            class="input input-bordered w-full input-sm focus:input-primary"
          />
        </div>
        <div class="form-control">
          <FormSwitch
            :model-value="activate"
            :label="$t('import_local.activate_after')"
            @update:model-value="$emit('update:activate', $event)"
          />
        </div>
        <p class="help-text">{{ $t('import_local.hint') }}</p>
      </div>
      <PanelFooter>
        <button class="btn btn-primary btn-sm gap-2 w-full" @click="$emit('submit')">
          {{ $t('import_local.save') }}
        </button>
      </PanelFooter>
  </PanelCard>
</template>

<script setup lang="ts">
import FormSwitch from './FormSwitch.vue';
import PanelCard from './PanelCard.vue';
import PanelFooter from './PanelFooter.vue';
import PanelHeader from './PanelHeader.vue';
import PanelTitle from './PanelTitle.vue';

defineProps<{
  name: string;
  activate: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:name', value: string): void;
  (e: 'update:activate', value: boolean): void;
  (e: 'file-change', file: File | null): void;
  (e: 'submit'): void;
}>();

function onFileChange(event: Event) {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0] || null;
  emit('file-change', file);
}
</script>
