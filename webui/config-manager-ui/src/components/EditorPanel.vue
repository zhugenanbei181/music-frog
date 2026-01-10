<template>
  <PanelCard>
      <PanelHeader>
        <template #title>
          <PanelTitle :text="$t('editor.title')" />
        </template>
        <template #actions>
          <button class="btn btn-outline btn-sm gap-2" @click="$emit('reset')">
            {{ $t('editor.new') }}
          </button>
        </template>
      </PanelHeader>
      <div class="space-y-4 flex-grow">
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ $t('editor.name_label') }}</span>
          </label>
          <input
            type="text"
            :value="name"
            @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
            :placeholder="$t('editor.name_placeholder')"
            class="input input-bordered w-full input-sm focus:input-primary"
          />
        </div>
        <div class="form-control h-[300px] w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ $t('editor.content_label') }}</span>
          </label>
          <textarea
            :value="content"
            @input="$emit('update:content', ($event.target as HTMLTextAreaElement).value)"
            class="textarea textarea-bordered h-full w-full textarea-sm focus:textarea-primary font-mono text-sm leading-relaxed"
            :placeholder="$t('editor.content_placeholder')"
          ></textarea>
        </div>
        <div class="form-control">
          <FormSwitch
            :model-value="activate"
            :label="$t('editor.activate_after')"
            @update:model-value="$emit('update:activate', $event)"
          />
        </div>
        <p class="help-text">
          {{ $t('editor.hint') }}
        </p>
      </div>
      <PanelFooter>
        <button class="btn btn-primary btn-sm gap-2 flex-1" @click="$emit('save')">
          {{ $t('editor.save') }}
        </button>
        <button class="btn btn-secondary btn-sm gap-2 flex-1" @click="$emit('open-external', name)">
          {{ $t('editor.external_edit') }}
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
  content: string;
  activate: boolean;
}>();

defineEmits<{
  (e: 'update:name', value: string): void;
  (e: 'update:content', value: string): void;
  (e: 'update:activate', value: boolean): void;
  (e: 'save'): void;
  (e: 'reset'): void;
  (e: 'open-external', name: string): void;
}>();
</script>
