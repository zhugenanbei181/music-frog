import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import SyncSettingsPanel from '../SyncSettingsPanel.vue';

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: {
    en: {
      sync: { title: 'Sync', test_conn: 'Test', sync_now_btn: 'Sync Now' },
      common: { save: 'Save' }
    }
  }
});

describe('SyncSettingsPanel', () => {
  it('renders sync config values', () => {
    const config = {
      enabled: true,
      url: 'http://test.com',
      username: 'user1',
      password: 'pwd',
      sync_interval_mins: 30,
      sync_on_startup: true,
    };
    const wrapper = mount(SyncSettingsPanel, {
      global: { plugins: [i18n] },
      props: {
        modelValue: config,
      },
    });

    expect(wrapper.find('input[type="text"]').element.value).toBe('http://test.com');
    expect(wrapper.find('input[type="password"]').element.value).toBe('pwd');
  });

  it('emits update when fields changed', async () => {
    const wrapper = mount(SyncSettingsPanel, {
      global: { plugins: [i18n] },
      props: {
        modelValue: { enabled: false, url: '', username: '', password: '', sync_interval_mins: 60, sync_on_startup: false },
      },
    });

    await wrapper.find('input[type="text"]').setValue('new-url');
    
    expect(wrapper.emitted('update:modelValue')).toBeTruthy();
    expect(wrapper.emitted('update:modelValue')![0][0]).toMatchObject({
      url: 'new-url'
    });
  });

  it('emits test and sync-now events', async () => {
    const wrapper = mount(SyncSettingsPanel, {
      global: { plugins: [i18n] },
      props: { 
        modelValue: { enabled: true, url: 'http://ok.com', username: '', password: '', sync_interval_mins: 60, sync_on_startup: false } 
      },
    });

    await wrapper.findAll('button').find(b => b.text().includes('Test'))?.trigger('click');
    expect(wrapper.emitted('test')).toBeTruthy();

    await wrapper.findAll('button').find(b => b.text().includes('Sync Now'))?.trigger('click');
    expect(wrapper.emitted('sync-now')).toBeTruthy();
  });
});
