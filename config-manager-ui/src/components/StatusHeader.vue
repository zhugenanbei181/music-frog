<template>
  <header class="panel px-6 py-5">
    <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
      <div class="space-y-1">
        <p class="text-sm uppercase tracking-[0.2em] text-ink-500">Mihomo Despicable Infiltrator</p>
        <h1 class="text-2xl font-semibold text-ink-900">配置管理控制台</h1>
        <p class="text-sm text-ink-500">
          订阅导入、配置编辑、内核切换统一在此完成，状态与结果会即时反馈。
        </p>
      </div>
      <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div class="rounded-2xl border border-ink-500/10 bg-sand-100 px-4 py-3 text-sm">
          <p class="font-semibold text-ink-900">{{ statusMessage }}</p>
          <p class="text-xs text-ink-500">{{ statusDetail }}</p>
        </div>
        <button class="btn btn-ghost" @click="$emit('refresh')">刷新状态</button>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
defineProps<{
  statusMessage: string;
  statusDetail: string;
}>();

defineEmits<{
  (event: 'refresh'): void;
}>();
</script>
