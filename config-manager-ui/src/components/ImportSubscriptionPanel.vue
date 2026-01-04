<template>
  <section class="panel px-5 py-4">
    <div class="flex items-center justify-between">
      <h2 class="panel-title">通过订阅导入</h2>
      <span class="badge badge-idle">URL</span>
    </div>
    <div class="mt-4 space-y-3">
      <div>
        <label class="label">配置名称</label>
        <input
          :value="name"
          class="input"
          placeholder="例如 main"
          @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div>
        <label class="label">订阅链接</label>
        <input
          :value="url"
          class="input"
          placeholder="https://..."
          @input="$emit('update:url', ($event.target as HTMLInputElement).value)"
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
        立即导入
      </button>
      <p class="text-xs text-ink-500">
        导入完成后会即时刷新列表，如需激活将后台重启内核。
      </p>
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  name: string;
  url: string;
  activate: boolean;
}>();

defineEmits<{
  (event: 'update:name', value: string): void;
  (event: 'update:url', value: string): void;
  (event: 'update:activate', value: boolean): void;
  (event: 'submit'): void;
}>();
</script>
