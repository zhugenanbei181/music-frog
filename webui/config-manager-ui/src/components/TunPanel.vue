<template>
  <PanelCard>
      <PanelHeader>
        <template #title>
          <PanelTitle :text="t('tun.title')" />
        </template>
        <template #actions>
          <FormSwitch
            :model-value="modelValue.enable ?? false"
            @update:model-value="updateField('enable', $event)"
          />
        </template>
      </PanelHeader>

      <div class="space-y-4 flex-grow overflow-auto pr-1">
        <div class="grid grid-cols-2 gap-4">
          <div class="form-control w-full">
            <label class="label py-1">
              <span class="label-text font-medium">{{ t('tun.stack') }}</span>
            </label>
            <select
              class="select select-bordered select-sm focus:select-primary"
              :value="modelValue.stack || ''"
              @change="updateField('stack', normalizeOptional(($event.target as HTMLSelectElement).value))"
            >
              <option value="">{{ t('tun.stack_auto') }}</option>
              <option value="system">{{ t('tun.stack_system') }}</option>
              <option value="gvisor">{{ t('tun.stack_gvisor') }}</option>
            </select>
          </div>
          <div class="form-control w-full">
            <label class="label py-1">
              <span class="label-text font-medium">{{ t('tun.mtu') }}</span>
            </label>
            <input
              type="number"
              class="input input-bordered w-full input-sm focus:input-primary"
              :value="modelValue.mtu ?? ''"
              @input="updateField('mtu', parseNumber(($event.target as HTMLInputElement).value))"
            />
          </div>
        </div>

        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('tun.dns_hijack') }}</span>
          </label>
          <textarea
            class="textarea textarea-bordered w-full textarea-sm focus:textarea-primary"
            rows="3"
            :placeholder="t('tun.dns_hijack_placeholder')"
            :value="toText(modelValue.dns_hijack)"
            @input="updateField('dns_hijack', parseLines(($event.target as HTMLTextAreaElement).value))"
          />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <FormSwitch
            :model-value="modelValue.auto_route ?? false"
            :label="t('tun.auto_route')"
            @update:model-value="updateField('auto_route', $event)"
          />
          <FormSwitch
            :model-value="modelValue.auto_detect_interface ?? false"
            :label="t('tun.auto_detect_interface')"
            @update:model-value="updateField('auto_detect_interface', $event)"
          />
        </div>

        <FormSwitch
          :model-value="modelValue.strict_route ?? false"
          :label="t('tun.strict_route')"
          @update:model-value="updateField('strict_route', $event)"
        />
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
import type { TunConfig } from '../types';

const { t } = useI18n();
const { parseLines, toText, normalizeOptional, parseNumber } = useFormUtils();

const props = defineProps<{
  modelValue: TunConfig;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: TunConfig): void;
  (e: 'save'): void;
  (e: 'refresh'): void;
}>();

function updateField<K extends keyof TunConfig>(field: K, value: TunConfig[K]) {
  emit('update:modelValue', {
    ...props.modelValue,
    [field]: value,
  });
}

</script>
