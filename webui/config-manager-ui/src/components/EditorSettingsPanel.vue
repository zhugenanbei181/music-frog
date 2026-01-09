<template>
  <div class="panel p-6">
    <div class="flex items-center justify-between">
      <h2 class="panel-title">{{ $t('editor_settings.title') }}</h2>
      <div class="flex gap-2">
        <button class="btn btn-ghost" @click="$emit('pick')">{{ $t('editor_settings.pick') }}</button>
        <button class="btn btn-ghost" @click="$emit('reset')">{{ $t('editor_settings.reset') }}</button>
        <button class="btn btn-primary" @click="$emit('save')">{{ $t('editor_settings.save') }}</button>
      </div>
    </div>
    <div class="mt-4">
      <div class="form-control w-full">
        <label class="label">
          <span class="label-text">{{ $t('editor_settings.path_label') }}</span>
        </label>
        <input
          type="text"
          :value="editorPath"
          @input="$emit('update:editorPath', ($event.target as HTMLInputElement).value)"
          :placeholder="$t('editor_settings.path_placeholder')"
          class="input w-full font-mono text-sm"
        />
        <label class="label">
          <span class="label-text-alt text-ink-500">
            {{ $t('editor_settings.hint') }}
          </span>
        </label>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
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