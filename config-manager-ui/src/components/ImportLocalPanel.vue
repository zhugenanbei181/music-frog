<template>
  <section class="panel px-5 py-4">
    <div class="flex items-center justify-between">
      <h2 class="panel-title">从本地文件导入</h2>
      <span class="badge badge-idle">文件</span>
    </div>
    <div class="mt-4 space-y-3">
      <div>
        <label class="label">选择文件</label>
        <input type="file" class="input" @change="onFileChange" />
      </div>
      <div>
        <label class="label">配置名称</label>
        <input
          :value="name"
          class="input"
          placeholder="例如 backup"
          @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <label class="flex items-center gap-2 text-sm text-ink-700">
        <input
          type="checkbox"
          class="h-4 w-4"
          :checked="activate"
          @change="$emit('update:activate', ($event.target as HTMLInputElement).checked)"
        />
        导入后设为当前配置
      </label>
      <button class="btn btn-primary w-full" @click="$emit('submit')">
        保存本地配置
      </button>
      <p class="text-xs text-ink-500">支持直接选择 YAML 文件。</p>
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  name: string;
  activate: boolean;
}>();

const emit = defineEmits<{
  (event: 'update:name', value: string): void;
  (event: 'update:activate', value: boolean): void;
  (event: 'file-change', file: File | null): void;
  (event: 'submit'): void;
}>();

function onFileChange(event: Event) {
  const input = event.target as HTMLInputElement;
  emit('file-change', input.files?.[0] || null);
}
</script>
