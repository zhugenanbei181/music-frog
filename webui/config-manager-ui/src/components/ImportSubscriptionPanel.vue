<template>
  <PanelCard>
      <PanelHeader>
        <template #title>
          <PanelTitle :text="$t('import_sub.title')" />
        </template>
      </PanelHeader>
      <div class="space-y-4 flex-grow">
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ $t('import_sub.name_label') }}</span>
          </label>
          <input
            type="text"
            :value="name"
            @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
            :placeholder="$t('import_sub.name_placeholder')"
            class="input input-bordered w-full input-sm focus:input-primary"
          />
        </div>
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ $t('import_sub.url_label') }}</span>
          </label>
          <input
            type="text"
            :value="url"
            @input="$emit('update:url', ($event.target as HTMLInputElement).value)"
            :placeholder="$t('import_sub.url_placeholder')"
            class="input input-bordered w-full input-sm focus:input-primary font-mono text-sm"
          />
        </div>
        <div class="form-control">
          <FormSwitch
            :model-value="activate"
            :label="$t('import_sub.activate_after')"
            @update:model-value="$emit('update:activate', $event)"
          />
        </div>
        <p class="help-text">
          {{ $t('import_sub.hint') }}
        </p>
      </div>
      <PanelFooter>
        <button class="btn btn-primary btn-sm gap-2 w-full" @click="$emit('submit')">
          {{ $t('import_sub.import_now') }}
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
  url: string;
  activate: boolean;
}>();

defineEmits<{
  (e: 'update:name', value: string): void;
  (e: 'update:url', value: string): void;
  (e: 'update:activate', value: boolean): void;
  (e: 'submit'): void;
}>();
</script>
