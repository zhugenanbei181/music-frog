import { describe, expect, it, vi } from 'vitest';
import { ref } from 'vue';
import { useAdvancedSettings } from '../useAdvancedSettings';
import { api } from '../../api';

vi.mock('../../api', () => ({
  api: {
    getDnsConfig: vi.fn(),
    saveDnsConfig: vi.fn(),
    getFakeIpConfig: vi.fn(),
    saveFakeIpConfig: vi.fn(),
    getTunConfig: vi.fn(),
    saveTunConfig: vi.fn(),
    getRules: vi.fn(),
    saveRules: vi.fn(),
    getRuleProviders: vi.fn(),
    saveRuleProviders: vi.fn(),
    flushFakeIpCache: vi.fn(),
  },
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('../useRebuildWatcher', () => ({
  useRebuildWatcher: () => ({
    waitForRebuild: vi.fn().mockResolvedValue(undefined),
  }),
}));

function buildBusy() {
  return {
    busy: ref(false),
    startBusy: vi.fn(),
    updateBusyDetail: vi.fn(),
    endBusy: vi.fn(),
  };
}

describe('useAdvancedSettings', () => {
  it('refreshes DNS config and sets dirty to false', async () => {
    const pushToast = vi.fn();
    const busy = buildBusy();
    const dnsData = { enable: true, nameserver: ['8.8.8.8'] };
    vi.mocked(api.getDnsConfig).mockResolvedValue(dnsData);

    const { dnsConfig, dirty, refreshDnsConfig } = useAdvancedSettings(pushToast, busy);
    
    await refreshDnsConfig();

    expect(dnsConfig.value).toEqual(dnsData);
    expect(dirty.dns).toBe(false);
  });

  it('marks dns as dirty when modified', async () => {
    const pushToast = vi.fn();
    const busy = buildBusy();
    const { dnsConfig, dirty } = useAdvancedSettings(pushToast, busy);

    dnsConfig.value.enable = true;
    
    // We need to wait for watcher if it was async, but it's flush: 'sync'
    expect(dirty.dns).toBe(true);
  });

  it('saves DNS config and waits for rebuild', async () => {
    const pushToast = vi.fn();
    const busy = buildBusy();
    vi.mocked(api.getDnsConfig).mockResolvedValue({});
    
    const { dnsConfig, saveDnsConfig } = useAdvancedSettings(pushToast, busy);
    dnsConfig.value.enable = true;

    await saveDnsConfig();

    expect(api.saveDnsConfig).toHaveBeenCalledWith({ enable: true });
    expect(pushToast).toHaveBeenCalledWith('dns.save_success');
    expect(busy.startBusy).toHaveBeenCalled();
    expect(busy.endBusy).toHaveBeenCalled();
  });

  it('validates rule providers before saving', async () => {
    const pushToast = vi.fn();
    const busy = buildBusy();
    const { ruleProvidersJson, saveRuleProviders } = useAdvancedSettings(pushToast, busy);

    // Invalid JSON
    ruleProvidersJson.value = '{ invalid }';
    await saveRuleProviders();
    expect(pushToast).toHaveBeenCalledWith(expect.stringContaining('JSON'), 'error');

    // Not an object
    ruleProvidersJson.value = '[]';
    await saveRuleProviders();
    expect(pushToast).toHaveBeenCalledWith('rules.invalid_providers', 'error');

    // Invalid provider item
    ruleProvidersJson.value = JSON.stringify({
        'my-provider': { type: '' } // empty type
    });
    await saveRuleProviders();
    expect(pushToast).toHaveBeenCalledWith('rules.invalid_provider_items', 'error');
  });
});
