import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import FakeIpPanel from '../FakeIpPanel.vue';

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: {
    en: {
      fake_ip: { title: 'Fake IP', range: 'Range', flush: 'Flush' },
      common: { save: 'Save', refresh: 'Refresh' }
    }
  }
});

describe('FakeIpPanel', () => {
  it('renders fake ip config values', () => {
    const config = {
      fake_ip_range: '198.18.0.1/16',
      fake_ip_filter: ['google.com'],
      store_fake_ip: true,
    };
    const wrapper = mount(FakeIpPanel, {
      global: { plugins: [i18n] },
      props: {
        modelValue: config,
      },
    });

    expect(wrapper.find('input[type="text"]').element.value).toBe('198.18.0.1/16');
    expect(wrapper.find('textarea').element.value).toBe('google.com');
  });

  it('emits update when fields changed', async () => {
    const wrapper = mount(FakeIpPanel, {
      global: { plugins: [i18n] },
      props: {
        modelValue: {},
      },
    });

    await wrapper.find('input[type="text"]').setValue('10.0.0.1/24');
    
    expect(wrapper.emitted('update:modelValue')).toBeTruthy();
    expect(wrapper.emitted('update:modelValue')![0][0]).toMatchObject({
      fake_ip_range: '10.0.0.1/24'
    });
  });

  it('emits flush event', async () => {
    const wrapper = mount(FakeIpPanel, {
      global: { plugins: [i18n] },
      props: { modelValue: {} },
    });

    await wrapper.findAll('button').find(b => b.text() === 'Flush')?.trigger('click');
    expect(wrapper.emitted('flush')).toBeTruthy();
  });
});
