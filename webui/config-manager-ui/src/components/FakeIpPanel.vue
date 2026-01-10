<template>
  <PanelCard>
      <PanelHeader>
        <template #title>
          <PanelTitle :text="t('fake_ip.title')" />
        </template>
      </PanelHeader>

      <div class="space-y-4 flex-grow overflow-auto pr-1">
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('fake_ip.range') }}</span>
          </label>
          <input
            type="text"
            class="input input-bordered w-full input-sm focus:input-primary"
            :placeholder="t('fake_ip.range_placeholder')"
            :value="modelValue.fake_ip_range || ''"
            @input="updateField('fake_ip_range', normalizeOptional(($event.target as HTMLInputElement).value))"
          />
        </div>

        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('fake_ip.filter') }}</span>
          </label>
          <textarea
            class="textarea textarea-bordered w-full textarea-sm focus:textarea-primary"
            rows="3"
            :placeholder="t('fake_ip.filter_placeholder')"
            :value="toText(modelValue.fake_ip_filter)"
            @input="updateField('fake_ip_filter', parseLines(($event.target as HTMLTextAreaElement).value))"
          />
        </div>

        <FormSwitch
          :model-value="modelValue.store_fake_ip ?? false"
          :label="t('fake_ip.store')"
          @update:model-value="updateField('store_fake_ip', $event)"
        />
      </div>

      <PanelFooter>
        <button class="btn btn-outline btn-sm gap-2" @click="$emit('refresh')">
          {{ t('common.refresh') }}
        </button>
        <button class="btn btn-secondary btn-sm gap-2" @click="$emit('flush')">
          {{ t('fake_ip.flush') }}
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
import type { FakeIpConfig } from '../types';

const { t } = useI18n();
const { parseLines, toText, normalizeOptional } = useFormUtils();

const props = defineProps<{
  modelValue: FakeIpConfig;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: FakeIpConfig): void;
  (e: 'save'): void;
  (e: 'refresh'): void;
  (e: 'flush'): void;
}>();

function updateField<K extends keyof FakeIpConfig>(field: K, value: FakeIpConfig[K]) {
  emit('update:modelValue', {
    ...props.modelValue,
    [field]: value,
  });
}

</script>
