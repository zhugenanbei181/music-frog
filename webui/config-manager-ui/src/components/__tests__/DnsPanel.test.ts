import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import DnsPanel from '../DnsPanel.vue';

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: {
    en: {
      dns: { title: 'DNS' },
      common: { save: 'Save', refresh: 'Refresh' }
    }
  }
});

describe('DnsPanel', () => {
  it('renders dns config values', () => {
    const config = {
      enable: true,
      nameserver: ['8.8.8.8', '1.1.1.1'],
      enhanced_mode: 'fake-ip',
    };
    const wrapper = mount(DnsPanel, {
      global: { plugins: [i18n] },
      props: {
        modelValue: config,
      },
    });

    expect(wrapper.find('textarea').element.value).toContain('8.8.8.8');
    expect(wrapper.find('select').element.value).toBe('fake-ip');
  });

  it('emits update when fields changed', async () => {
    const wrapper = mount(DnsPanel, {
      global: { plugins: [i18n] },
      props: {
        modelValue: {},
      },
    });

    const textarea = wrapper.find('textarea');
    await textarea.setValue('1.2.3.4');
    
    expect(wrapper.emitted('update:modelValue')).toBeTruthy();
    expect(wrapper.emitted('update:modelValue')![0][0]).toMatchObject({
      nameserver: ['1.2.3.4']
    });
  });

  it('emits save and refresh events', async () => {
    const wrapper = mount(DnsPanel, {
      global: { plugins: [i18n] },
      props: { modelValue: {} },
    });

    await wrapper.findAll('button').find(b => b.text() === 'Save')?.trigger('click');
    expect(wrapper.emitted('save')).toBeTruthy();

    await wrapper.findAll('button').find(b => b.text() === 'Refresh')?.trigger('click');
    expect(wrapper.emitted('refresh')).toBeTruthy();
  });
});
