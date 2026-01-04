<template>
  <section class="panel px-5 py-4">
    <div class="flex items-center justify-between">
      <h2 class="panel-title">配置编辑器</h2>
      <button class="btn btn-ghost" @click="$emit('reset')">新建</button>
    </div>
    <div class="mt-4 space-y-3">
      <div>
        <label class="label">配置名称</label>
        <input
          :value="name"
          class="input"
          placeholder="例如 work"
          @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div>
        <label class="label">配置内容</label>
        <textarea
          :value="content"
          rows="10"
          class="textarea"
          placeholder="# 粘贴 YAML 配置"
          @input="$emit('update:content', ($event.target as HTMLTextAreaElement).value)"
        ></textarea>
      </div>
      <label class="flex items-center gap-2 text-sm text-ink-700">
        <input
          type="checkbox"
          class="h-4 w-4"
          :checked="activate"
          @change="$emit('update:activate', ($event.target as HTMLInputElement).checked)"
        />
        保存后设为当前配置
      </label>
      <div class="flex flex-wrap gap-2">
        <button class="btn btn-primary" @click="$emit('save')">保存配置</button>
        <button class="btn btn-ghost" @click="$emit('open-external', name)">
          外部编辑
        </button>
      </div>
      <p class="text-xs text-ink-500">支持外部编辑器打开当前配置文件。</p>
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  name: string;
  content: string;
  activate: boolean;
}>();

defineEmits<{
  (event: 'update:name', value: string): void;
  (event: 'update:content', value: string): void;
  (event: 'update:activate', value: boolean): void;
  (event: 'save'): void;
  (event: 'reset'): void;
  (event: 'open-external', name: string): void;
}>();
</script>
