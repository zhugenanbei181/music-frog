<template>
  <PanelCard>
      <PanelHeader>
        <template #title>
          <PanelTitle :text="$t('editor_settings.title')" />
        </template>
      </PanelHeader>

      <div class="space-y-4 flex-grow">
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ $t('editor_settings.path_label') }}</span>
          </label>
          <input
            type="text"
            :value="editorPath"
            @input="$emit('update:editorPath', ($event.target as HTMLInputElement).value)"
            :placeholder="$t('editor_settings.path_placeholder')"
            class="input input-bordered w-full input-sm focus:input-primary font-mono text-sm"
          />
          <label class="label">
            <span class="label-text-alt help-text">
              {{ $t('editor_settings.hint') }}
            </span>
          </label>
        </div>
      </div>

      <PanelFooter>
        <button class="btn btn-outline btn-sm gap-2" @click="$emit('pick')">
          {{ $t('editor_settings.pick') }}
        </button>
        <button class="btn btn-secondary btn-sm gap-2" @click="$emit('reset')">
          {{ $t('editor_settings.reset') }}
        </button>
        <button class="btn btn-primary btn-sm gap-2" @click="$emit('save')">
          {{ $t('editor_settings.save') }}
        </button>
      </PanelFooter>
  </PanelCard>
</template>

<script setup lang="ts">
import PanelCard from './PanelCard.vue';
import PanelFooter from './PanelFooter.vue';
import PanelHeader from './PanelHeader.vue';
import PanelTitle from './PanelTitle.vue';

defineProps<{
  editorPath: string;
}>();

defineEmits<{
  (e: 'update:editorPath', value: string): void;
  (e: 'pick'): void;
  (e: 'reset'): void;
  (e: 'save'): void;
}>();
</script>
