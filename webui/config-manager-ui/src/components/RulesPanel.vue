<template>
  <PanelCard>
      <PanelHeader>
        <template #title>
          <PanelTitle :text="t('rules.title')" />
        </template>
      </PanelHeader>

      <div class="space-y-4 grow overflow-auto pr-1">
        <div class="form-control w-full">
          <label class="label py-1">
            <span class="label-text font-medium">{{ t('rules.providers_title') }}</span>
          </label>
          <textarea
            class="textarea textarea-bordered w-full textarea-sm focus:textarea-primary font-mono"
            rows="6"
            :placeholder="t('rules.providers_placeholder')"
            :value="providersJson"
            @input="emit('update:providersJson', ($event.target as HTMLTextAreaElement).value)"
          />
          <p class="help-text mt-2">{{ t('rules.providers_hint') }}</p>
        </div>

        <div class="divider my-0"></div>

      <div>
        <div class="flex items-center justify-between mb-2">
          <span class="label-text font-medium">{{ t('rules.rules_title') }}</span>
          <button class="btn btn-outline btn-sm" @click="addRule">{{ t('rules.add_rule') }}</button>
        </div>

        <div v-if="rules.length === 0" class="empty-text">
          {{ t('rules.empty') }}
        </div>
        <div
          v-else
          ref="listContainer"
          class="max-h-80 overflow-y-auto pr-1"
        >
          <div :style="{ height: `${totalHeight}px`, position: 'relative' }">
            <div :style="{ transform: `translateY(${offsetY}px)` }">
              <div
                v-for="item in visibleRules"
                :key="`${item.entry.rule}-${item.index}`"
                class="flex h-11 items-center gap-2 rounded-xl border border-sand-200 bg-white px-3 py-2 transition-colors hover:border-primary-200"
              >
                <FormSwitch
                  :model-value="item.entry.enabled"
                  @update:model-value="(val) => toggleRule(item.index, val)"
                />
                <input
                  type="text"
                  class="input input-bordered input-sm flex-1 focus:input-primary"
                  :placeholder="t('rules.rule_placeholder')"
                  :value="item.entry.rule"
                  @input="updateRule(item.index, ($event.target as HTMLInputElement).value)"
                />
                <div class="flex gap-1">
                  <button
                    class="btn btn-ghost btn-xs"
                    @click="moveRule(item.index, -1)"
                    :disabled="item.index === 0"
                  >
                    ↑
                  </button>
                  <button
                    class="btn btn-ghost btn-xs"
                    @click="moveRule(item.index, 1)"
                    :disabled="item.index === rules.length - 1"
                  >
                    ↓
                  </button>
                  <button class="btn btn-danger btn-xs" @click="removeRule(item.index)">✕</button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
      </div>

      <PanelFooter>
        <button class="btn btn-outline btn-sm gap-2" @click="$emit('refresh')">
          {{ t('common.refresh') }}
        </button>
        <button class="btn btn-secondary btn-sm gap-2" @click="$emit('save-providers')">
          {{ t('rules.save_providers') }}
        </button>
        <button class="btn btn-primary btn-sm gap-2" @click="$emit('save-rules')">
          {{ t('rules.save_rules') }}
        </button>
      </PanelFooter>
  </PanelCard>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import FormSwitch from './FormSwitch.vue';
import PanelCard from './PanelCard.vue';
import PanelFooter from './PanelFooter.vue';
import PanelHeader from './PanelHeader.vue';
import PanelTitle from './PanelTitle.vue';
import type { RuleEntry } from '../types';

const { t } = useI18n();

const props = defineProps<{
  rules: RuleEntry[];
  providersJson: string;
}>();

const emit = defineEmits<{
  (e: 'update:rules', value: RuleEntry[]): void;
  (e: 'update:providersJson', value: string): void;
  (e: 'save-rules'): void;
  (e: 'save-providers'): void;
  (e: 'refresh'): void;
}>();

const listContainer = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewportHeight = ref(0);
const ROW_HEIGHT = 44;
const OVERSCAN = 6;

const totalHeight = computed(() => props.rules.length * ROW_HEIGHT);
const startIndex = computed(() =>
  Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - OVERSCAN),
);
const endIndex = computed(() => {
  const safeHeight = Math.max(viewportHeight.value, ROW_HEIGHT);
  return Math.min(
    props.rules.length,
    Math.ceil((scrollTop.value + safeHeight) / ROW_HEIGHT) + OVERSCAN,
  );
});
const visibleRules = computed(() => {
  const start = startIndex.value;
  const end = endIndex.value;
  return props.rules.slice(start, end).map((entry, idx) => ({
    entry,
    index: start + idx,
  }));
});
const offsetY = computed(() => startIndex.value * ROW_HEIGHT);

function onScroll() {
  if (!listContainer.value) {
    return;
  }
  scrollTop.value = listContainer.value.scrollTop;
  viewportHeight.value = listContainer.value.clientHeight;
}

let resizeObserver: ResizeObserver | null = null;

onMounted(() => {
  onScroll();
  if (listContainer.value) {
    listContainer.value.addEventListener('scroll', onScroll, { passive: true });
    if (typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(onScroll);
      resizeObserver.observe(listContainer.value);
    }
  }
});

onBeforeUnmount(() => {
  if (listContainer.value) {
    listContainer.value.removeEventListener('scroll', onScroll);
  }
  resizeObserver?.disconnect();
  resizeObserver = null;
});

function addRule() {
  emit('update:rules', [...props.rules, { rule: '', enabled: true }]);
}

function updateRule(index: number, value: string) {
  const next = props.rules.map((entry, idx) => (idx === index ? { ...entry, rule: value } : entry));
  emit('update:rules', next);
}

function toggleRule(index: number, enabled: boolean) {
  const next = props.rules.map((entry, idx) => (idx === index ? { ...entry, enabled } : entry));
  emit('update:rules', next);
}

function removeRule(index: number) {
  const next = props.rules.filter((_entry, idx) => idx !== index);
  emit('update:rules', next);
}

function moveRule(index: number, direction: number) {
  const target = index + direction;
  if (target < 0 || target >= props.rules.length) {
    return;
  }
  const next = [...props.rules];
  const temp = next[index];
  next[index] = next[target];
  next[target] = temp;
  emit('update:rules', next);
}
</script>
