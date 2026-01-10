import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../api';
import type { DnsConfig, FakeIpConfig, RuleEntry, RuleProvider, TunConfig } from '../types';
import type { ToastTone } from './useToasts';
import { useRebuildWatcher } from './useRebuildWatcher';

type BusyActions = {
  busy: { value: boolean };
  startBusy: (message: string, detail: string) => void;
  updateBusyDetail: (detail: string) => void;
  endBusy: () => void;
};

export function useAdvancedSettings(pushToast: (message: string, tone?: ToastTone) => void, busy: BusyActions) {
  const { t } = useI18n();
  const { waitForRebuild } = useRebuildWatcher(busy.updateBusyDetail);

  const dnsConfig = ref<DnsConfig>({});
  const fakeIpConfig = ref<FakeIpConfig>({});
  const tunConfig = ref<TunConfig>({});
  const rules = ref<RuleEntry[]>([]);
  const ruleProvidersJson = ref('{}');

  function isPlainObject(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
  }

  function isRuleProvider(value: Record<string, unknown>): value is RuleProvider {
    if (typeof value.type !== 'string' || value.type.trim().length === 0) {
      return false;
    }
    if (value.behavior !== undefined && typeof value.behavior !== 'string') {
      return false;
    }
    if (value.path !== undefined && typeof value.path !== 'string') {
      return false;
    }
    if (value.url !== undefined && typeof value.url !== 'string') {
      return false;
    }
    if (value.interval !== undefined && (typeof value.interval !== 'number' || !Number.isFinite(value.interval))) {
      return false;
    }
    if (value.format !== undefined && typeof value.format !== 'string') {
      return false;
    }
    return true;
  }

  function areRuleProviders(value: Record<string, unknown>): value is Record<string, RuleProvider> {
    return Object.values(value).every((entry) => isPlainObject(entry) && isRuleProvider(entry));
  }

  async function refreshDnsConfig(silent = false) {
    try {
      const data = await api.getDnsConfig();
      dnsConfig.value = data || {};
    } catch (err) {
      const message = (err as Error).message || String(err);
      if (!silent) {
        pushToast(message, 'error');
      }
    }
  }

  async function refreshFakeIpConfig(silent = false) {
    try {
      const data = await api.getFakeIpConfig();
      fakeIpConfig.value = data || {};
    } catch (err) {
      const message = (err as Error).message || String(err);
      if (!silent) {
        pushToast(message, 'error');
      }
    }
  }

  async function refreshRuleProviders(silent = false) {
    try {
      const data = await api.getRuleProviders();
      ruleProvidersJson.value = JSON.stringify(data.providers || {}, null, 2);
    } catch (err) {
      const message = (err as Error).message || String(err);
      if (!silent) {
        pushToast(message, 'error');
      }
    }
  }

  async function refreshRules(silent = false) {
    try {
      const data = await api.getRules();
      rules.value = data.rules || [];
    } catch (err) {
      const message = (err as Error).message || String(err);
      if (!silent) {
        pushToast(message, 'error');
      }
    }
  }

  async function refreshRulesAndProviders(silent = false) {
    await Promise.all([refreshRules(silent), refreshRuleProviders(silent)]);
  }

  async function refreshTunConfig(silent = false) {
    try {
      const data = await api.getTunConfig();
      tunConfig.value = data || {};
    } catch (err) {
      const message = (err as Error).message || String(err);
      if (!silent) {
        pushToast(message, 'error');
      }
    }
  }

  async function saveDnsConfig() {
    if (busy.busy.value) {
      return;
    }
    busy.startBusy(t('dns.saving'), t('dns.saving_detail'));
    try {
      await api.saveDnsConfig(dnsConfig.value);
      await waitForRebuild(t('dns.saving'));
      pushToast(t('dns.save_success'));
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    } finally {
      await refreshDnsConfig(true);
      busy.endBusy();
    }
  }

  async function saveFakeIpConfig() {
    if (busy.busy.value) {
      return;
    }
    busy.startBusy(t('fake_ip.saving'), t('fake_ip.saving_detail'));
    try {
      await api.saveFakeIpConfig(fakeIpConfig.value);
      await waitForRebuild(t('fake_ip.saving'));
      pushToast(t('fake_ip.save_success'));
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    } finally {
      await refreshFakeIpConfig(true);
      busy.endBusy();
    }
  }

  async function flushFakeIpCache() {
    if (busy.busy.value) {
      return;
    }
    busy.startBusy(t('fake_ip.flushing'), t('fake_ip.flushing_detail'));
    try {
      const result = await api.flushFakeIpCache();
      if (result.removed) {
        pushToast(t('fake_ip.flush_done'));
      } else {
        pushToast(t('fake_ip.flush_none'));
      }
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    } finally {
      busy.endBusy();
    }
  }

  async function saveRuleProviders() {
    if (busy.busy.value) {
      return;
    }
    busy.startBusy(t('rules.saving_providers'), t('rules.saving_detail'));
    try {
      const trimmed = ruleProvidersJson.value.trim();
      const parsed: unknown = trimmed ? JSON.parse(trimmed) : {};
      if (!isPlainObject(parsed)) {
        throw new Error(t('rules.invalid_providers'));
      }
      if (!areRuleProviders(parsed)) {
        throw new Error(t('rules.invalid_provider_items'));
      }
      await api.saveRuleProviders({ providers: parsed });
      await waitForRebuild(t('rules.saving_providers'));
      pushToast(t('rules.save_providers_success'));
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    } finally {
      await refreshRuleProviders(true);
      busy.endBusy();
    }
  }

  async function saveRules() {
    if (busy.busy.value) {
      return;
    }
    busy.startBusy(t('rules.saving_rules'), t('rules.saving_detail'));
    try {
      await api.saveRules({ rules: rules.value });
      await waitForRebuild(t('rules.saving_rules'));
      pushToast(t('rules.save_rules_success'));
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    } finally {
      await refreshRules(true);
      busy.endBusy();
    }
  }

  async function saveTunConfig() {
    if (busy.busy.value) {
      return;
    }
    busy.startBusy(t('tun.saving'), t('tun.saving_detail'));
    try {
      await api.saveTunConfig(tunConfig.value);
      await waitForRebuild(t('tun.saving'));
      pushToast(t('tun.save_success'));
    } catch (err) {
      const message = (err as Error).message || String(err);
      pushToast(message, 'error');
    } finally {
      await refreshTunConfig(true);
      busy.endBusy();
    }
  }

  return {
    dnsConfig,
    fakeIpConfig,
    tunConfig,
    rules,
    ruleProvidersJson,
    refreshDnsConfig,
    refreshFakeIpConfig,
    refreshRuleProviders,
    refreshRules,
    refreshRulesAndProviders,
    refreshTunConfig,
    saveDnsConfig,
    saveFakeIpConfig,
    flushFakeIpCache,
    saveRuleProviders,
    saveRules,
    saveTunConfig,
  };
}
