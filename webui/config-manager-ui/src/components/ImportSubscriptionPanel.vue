<template>
  <div class="panel p-6">
    <h2 class="panel-title">{{ $t('import_sub.title') }}</h2>
    <div class="mt-4 space-y-4">
      <div class="form-control w-full">
        <label class="label">
          <span class="label-text">{{ $t('import_sub.name_label') }}</span>
        </label>
        <input
          type="text"
          :value="name"
          @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
          :placeholder="$t('import_sub.name_placeholder')"
          class="input w-full"
        />
      </div>
      <div class="form-control w-full">
        <label class="label">
          <span class="label-text">{{ $t('import_sub.url_label') }}</span>
        </label>
        <input
          type="text"
          :value="url"
          @input="$emit('update:url', ($event.target as HTMLInputElement).value)"
          placeholder="https://..."
          class="input w-full font-mono text-sm"
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
          <span class="label-text">{{ $t('import_sub.activate_after') }}</span>
        </label>
      </div>
      <button class="btn btn-primary w-full" @click="$emit('submit')">
        {{ $t('import_sub.import_now') }}
      </button>
      <p class="text-xs text-ink-500">
        {{ $t('import_sub.hint') }}
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
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
