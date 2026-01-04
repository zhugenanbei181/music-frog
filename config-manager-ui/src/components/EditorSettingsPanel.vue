<template>
  <section class="panel px-5 py-4">
    <div class="flex items-center justify-between">
      <h2 class="panel-title">外部编辑器设置</h2>
      <div class="flex gap-2">
        <button class="btn btn-ghost" @click="$emit('pick')">选择路径</button>
        <button class="btn btn-ghost" @click="$emit('reset')">恢复默认</button>
        <button class="btn btn-primary" @click="$emit('save')">保存</button>
      </div>
    </div>
    <div class="mt-4 grid gap-3 md:grid-cols-[2fr_1fr]">
      <div>
        <label class="label">编辑器路径</label>
        <input
          :value="editorPath"
          class="input"
          placeholder="例如 C:\\Program Files\\Microsoft VS Code\\Code.exe"
          @input="$emit('update:editorPath', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div class="rounded-xl border border-ink-500/10 bg-sand-100 p-3 text-xs text-ink-500">
        留空将自动定位 VSCode（安装目录或 PATH），未检测到时回退记事本。
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  editorPath: string;
}>();

defineEmits<{
  (event: 'pick'): void;
  (event: 'update:editorPath', value: string): void;
  (event: 'reset'): void;
  (event: 'save'): void;
}>();
</script>
