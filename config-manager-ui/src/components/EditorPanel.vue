<template>
  <div class="panel h-full flex flex-col p-6">
    <div class="mb-4 flex items-center justify-between">
      <h2 class="panel-title">{{ $t('editor.title') }}</h2>
      <button class="btn btn-ghost" @click="$emit('reset')">{{ $t('editor.new') }}</button>
    </div>
    <div class="flex-1 space-y-4">
      <div class="form-control w-full">
        <label class="label">
          <span class="label-text">{{ $t('editor.name_label') }}</span>
        </label>
        <input
          type="text"
          :value="name"
          @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
          :placeholder="$t('editor.name_placeholder')"
          class="input w-full"
        />
      </div>
      <div class="form-control h-[300px] w-full">
        <label class="label">
          <span class="label-text">{{ $t('editor.content_label') }}</span>
        </label>
        <textarea
          :value="content"
          @input="$emit('update:content', ($event.target as HTMLTextAreaElement).value)"
          class="textarea h-full w-full font-mono text-sm leading-relaxed"
          :placeholder="$t('editor.content_placeholder')"
        ></textarea>
      </div>
      <div class="form-control">
        <label class="label cursor-pointer justify-start gap-2">
          <input
            type="checkbox"
            class="checkbox checkbox-primary"
            :checked="activate"
            @change="$emit('update:activate', ($event.target as HTMLInputElement).checked)"
          />
          <span class="label-text">{{ $t('editor.activate_after') }}</span>
        </label>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-primary flex-1" @click="$emit('save')">
          {{ $t('editor.save') }}
        </button>
        <button class="btn btn-secondary flex-1" @click="$emit('open-external', name)">
          {{ $t('editor.external_edit') }}
        </button>
      </div>
      <p class="text-xs text-ink-500">
        {{ $t('editor.hint') }}
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
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
