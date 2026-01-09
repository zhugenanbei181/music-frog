<template>
  <div class="panel p-6">
    <div class="flex items-center justify-between">
      <h2 class="panel-title">{{ $t('import_local.title') }}</h2>
      <span class="badge badge-idle">{{ $t('import_local.badge') }}</span>
    </div>
    <div class="mt-4 space-y-4">
      <div class="form-control w-full">
        <label class="label">
          <span class="label-text">{{ $t('import_local.file_label') }}</span>
        </label>
        <input
          type="file"
          accept=".yaml,.yml,.txt"
          @change="onFileChange"
          class="file-input w-full"
        />
      </div>
      <div class="form-control w-full">
        <label class="label">
          <span class="label-text">{{ $t('import_local.name_label') }}</span>
        </label>
        <input
          type="text"
          :value="name"
          @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
          :placeholder="$t('import_local.name_placeholder')"
          class="input w-full"
        />
      </div>
      <div class="form-control">
        <label class="label cursor-pointer justify-start gap-2">
          <input
            type="checkbox"
            class="checkbox checkbox-primary"
            :checked="activate"
            @change="$emit('update:activate', ($event.target as HTMLInputElement).checked)"
          />
          <span class="label-text">{{ $t('import_local.activate_after') }}</span>
        </label>
      </div>
      <button class="btn btn-primary w-full" @click="$emit('submit')">
        {{ $t('import_local.save') }}
      </button>
      <p class="text-xs text-ink-500">{{ $t('import_local.hint') }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
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
