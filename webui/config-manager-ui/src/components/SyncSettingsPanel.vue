<template>
  <div class="card bg-base-100 shadow-xl border border-base-content/10 h-full flex flex-col">
    <div class="card-body p-6 flex flex-col h-full">
      <div class="flex items-center justify-between mb-4">
        <h2 class="card-title text-xl font-bold flex items-center gap-2">
          <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 text-primary" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 19l3 3m0 0l3-3m-3 3V10" />
          </svg>
          {{ t('sync.title') }}
        </h2>
        <input 
          type="checkbox" 
          class="toggle toggle-primary" 
          :checked="modelValue.enabled" 
          @change="updateField('enabled', ($event.target as HTMLInputElement).checked)"
        />
      </div>

      <div class="space-y-4 flex-grow overflow-auto pr-1" v-if="modelValue.enabled">
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('sync.url') }}</span>
          </label>
          <input 
            type="text" 
            :placeholder="t('sync.url_placeholder')" 
            class="input input-bordered w-full input-sm focus:input-primary"
            :value="modelValue.url"
            @input="updateField('url', ($event.target as HTMLInputElement).value)"
          />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="form-control w-full">
            <label class="label py-1">
              <span class="label-text font-medium">{{ t('sync.username') }}</span>
            </label>
            <input 
              type="text" 
              class="input input-bordered w-full input-sm focus:input-primary"
              :value="modelValue.username"
              @input="updateField('username', ($event.target as HTMLInputElement).value)"
            />
          </div>
          <div class="form-control w-full">
            <label class="label py-1">
              <span class="label-text font-medium">{{ t('sync.password') }}</span>
            </label>
            <input 
              type="password" 
              class="input input-bordered w-full input-sm focus:input-primary"
              :value="modelValue.password"
              @input="updateField('password', ($event.target as HTMLInputElement).value)"
            />
          </div>
        </div>

        <div class="grid grid-cols-2 gap-4 items-end">
          <div class="form-control w-full">
            <label class="label py-1">
              <span class="label-text font-medium">{{ t('sync.interval') }} ({{ t('sync.mins') }})</span>
            </label>
            <input 
              type="number" 
              class="input input-bordered w-full input-sm focus:input-primary"
              :value="modelValue.sync_interval_mins"
              @input="updateField('sync_interval_mins', parseInt(($event.target as HTMLInputElement).value))"
            />
          </div>
          <div class="form-control">
            <label class="label cursor-pointer justify-start gap-3 py-2">
              <input 
                type="checkbox" 
                class="checkbox checkbox-primary checkbox-sm" 
                :checked="modelValue.sync_on_startup"
                @change="updateField('sync_on_startup', ($event.target as HTMLInputElement).checked)"
              />
              <span class="label-text">{{ t('sync.sync_on_startup') }}</span>
            </label>
          </div>
        </div>
      </div>

      <div class="mt-auto pt-6 flex flex-wrap gap-2 justify-end border-t border-base-content/5">
        <button 
          class="btn btn-outline btn-sm gap-2" 
          @click="$emit('test')"
          :disabled="!modelValue.url"
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          {{ t('sync.test_conn') }}
        </button>
        <button 
          class="btn btn-primary btn-sm gap-2" 
          @click="$emit('save')"
        >
          {{ t('common.save') }}
        </button>
        <button 
          class="btn btn-secondary btn-sm gap-2" 
          @click="$emit('sync-now')"
          :disabled="!modelValue.enabled || !modelValue.url"
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
          {{ t('sync.sync_now_btn') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { WebDavConfig } from '../types';

const { t } = useI18n();

const props = defineProps<{
  modelValue: WebDavConfig;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: WebDavConfig): void;
  (e: 'save'): void;
  (e: 'test'): void;
  (e: 'sync-now'): void;
}>();

function updateField<K extends keyof WebDavConfig>(field: K, value: WebDavConfig[K]) {
  emit('update:modelValue', {
    ...props.modelValue,
    [field]: value
  });
}
</script>
