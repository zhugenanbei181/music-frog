import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import TunPanel from '../TunPanel.vue';

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: {
    en: {
      tun: { title: 'TUN' },
      common: { save: 'Save', refresh: 'Refresh' }
    }
  }
});

describe('TunPanel', () => {
  it('renders tun config values', () => {
    const config = {
      enable: true,
      stack: 'system',
      mtu: 1500,
    };
    const wrapper = mount(TunPanel, {
      global: { plugins: [i18n] },
      props: {
        modelValue: config,
      },
    });

    expect(wrapper.find('select').element.value).toBe('system');
    expect(wrapper.find('input[type="number"]').element.value).toBe('1500');
  });

  it('emits update when fields changed', async () => {
    const wrapper = mount(TunPanel, {
      global: { plugins: [i18n] },
      props: {
        modelValue: {},
      },
    });

    await wrapper.find('input[type="number"]').setValue(1400);
    
    expect(wrapper.emitted('update:modelValue')).toBeTruthy();
    expect(wrapper.emitted('update:modelValue')![0][0]).toMatchObject({
      mtu: 1400
    });
  });

  it('emits save and refresh events', async () => {
    const wrapper = mount(TunPanel, {
      global: { plugins: [i18n] },
      props: { modelValue: {} },
    });

    await wrapper.findAll('button').find(b => b.text() === 'Save')?.trigger('click');
    expect(wrapper.emitted('save')).toBeTruthy();
  });
});
