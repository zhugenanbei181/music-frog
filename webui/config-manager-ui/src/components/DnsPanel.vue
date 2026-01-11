<template>
  <PanelCard>
      <PanelHeader>
        <template #title>
          <PanelTitle :text="t('dns.title')" />
        </template>
        <template #actions>
          <FormSwitch
            :model-value="modelValue.enable ?? false"
            @update:model-value="updateField('enable', $event)"
          />
        </template>
      </PanelHeader>

      <div class="space-y-4 grow overflow-auto pr-1">
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('dns.nameserver') }}</span>
          </label>
          <textarea
            class="textarea textarea-bordered w-full textarea-sm focus:textarea-primary"
            rows="3"
            :placeholder="t('dns.nameserver_placeholder')"
            :value="toText(modelValue.nameserver)"
            @input="updateField('nameserver', parseLines(($event.target as HTMLTextAreaElement).value))"
          />
        </div>

        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('dns.fallback') }}</span>
          </label>
          <textarea
            class="textarea textarea-bordered w-full textarea-sm focus:textarea-primary"
            rows="2"
            :placeholder="t('dns.fallback_placeholder')"
            :value="toText(modelValue.fallback)"
            @input="updateField('fallback', parseLines(($event.target as HTMLTextAreaElement).value))"
          />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="form-control w-full">
            <label class="label py-1">
              <span class="label-text font-medium">{{ t('dns.enhanced_mode') }}</span>
            </label>
            <select
              class="select select-bordered select-sm focus:select-primary"
              :value="modelValue.enhanced_mode || ''"
              @change="updateField('enhanced_mode', normalizeOptional(($event.target as HTMLSelectElement).value))"
            >
              <option value="">{{ t('dns.enhanced_mode_auto') }}</option>
              <option value="fake-ip">fake-ip</option>
              <option value="redir-host">redir-host</option>
            </select>
          </div>
          <div class="form-control w-full">
            <label class="label py-1">
              <span class="label-text font-medium">{{ t('dns.fake_ip_range') }}</span>
            </label>
            <input
              type="text"
              class="input input-bordered w-full input-sm focus:input-primary"
              :placeholder="t('dns.fake_ip_range_placeholder')"
              :value="modelValue.fake_ip_range || ''"
              @input="updateField('fake_ip_range', normalizeOptional(($event.target as HTMLInputElement).value))"
            />
          </div>
        </div>

        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('dns.fake_ip_filter') }}</span>
          </label>
          <textarea
            class="textarea textarea-bordered w-full textarea-sm focus:textarea-primary"
            rows="2"
            :placeholder="t('dns.fake_ip_filter_placeholder')"
            :value="toText(modelValue.fake_ip_filter)"
            @input="updateField('fake_ip_filter', parseLines(($event.target as HTMLTextAreaElement).value))"
          />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <FormSwitch
            :model-value="modelValue.ipv6 ?? false"
            :label="t('dns.ipv6')"
            @update:model-value="updateField('ipv6', $event)"
          />
          <FormSwitch
            :model-value="modelValue.cache ?? false"
            :label="t('dns.cache')"
            @update:model-value="updateField('cache', $event)"
          />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <FormSwitch
            :model-value="modelValue.use_hosts ?? false"
            :label="t('dns.use_hosts')"
            @update:model-value="updateField('use_hosts', $event)"
          />
          <FormSwitch
            :model-value="modelValue.use_system_hosts ?? false"
            :label="t('dns.use_system_hosts')"
            @update:model-value="updateField('use_system_hosts', $event)"
          />
        </div>

        <FormSwitch
          :model-value="modelValue.respect_rules ?? false"
          :label="t('dns.respect_rules')"
          @update:model-value="updateField('respect_rules', $event)"
        />

        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('dns.proxy_server_nameserver') }}</span>
          </label>
          <textarea
            class="textarea textarea-bordered w-full textarea-sm focus:textarea-primary"
            rows="2"
            :value="toText(modelValue.proxy_server_nameserver)"
            @input="updateField('proxy_server_nameserver', parseLines(($event.target as HTMLTextAreaElement).value))"
          />
        </div>

        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('dns.direct_nameserver') }}</span>
          </label>
          <textarea
            class="textarea textarea-bordered w-full textarea-sm focus:textarea-primary"
            rows="2"
            :value="toText(modelValue.direct_nameserver)"
            @input="updateField('direct_nameserver', parseLines(($event.target as HTMLTextAreaElement).value))"
          />
        </div>
      </div>

      <PanelFooter>
        <button class="btn btn-outline btn-sm gap-2" @click="$emit('refresh')">
          {{ t('common.refresh') }}
        </button>
        <button class="btn btn-primary btn-sm gap-2" @click="$emit('save')">
          {{ t('common.save') }}
        </button>
      </PanelFooter>
  </PanelCard>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { useFormUtils } from '../composables/useFormUtils';
import FormSwitch from './FormSwitch.vue';
import PanelCard from './PanelCard.vue';
import PanelFooter from './PanelFooter.vue';
import PanelHeader from './PanelHeader.vue';
import PanelTitle from './PanelTitle.vue';
import type { DnsConfig } from '../types';

const { t } = useI18n();
const { parseLines, toText, normalizeOptional } = useFormUtils();

const props = defineProps<{
  modelValue: DnsConfig;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: DnsConfig): void;
  (e: 'save'): void;
  (e: 'refresh'): void;
}>();

function updateField<K extends keyof DnsConfig>(field: K, value: DnsConfig[K]) {
  emit('update:modelValue', {
    ...props.modelValue,
    [field]: value,
  });
}

</script>
