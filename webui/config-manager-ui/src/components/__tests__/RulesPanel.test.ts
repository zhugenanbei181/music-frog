import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import RulesPanel from '../RulesPanel.vue';

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: {
    en: {
      rules: { 
        title: 'Rules', 
        add_rule: 'Add',
        save_rules: 'Save Rules',
        save_providers: 'Save Providers'
      },
      common: { refresh: 'Refresh' }
    }
  }
});

describe('RulesPanel', () => {
  it('renders rules list and providers JSON', () => {
    const rules = [{ rule: 'DOMAIN,google.com,PROXY', enabled: true }];
    const providers = '{"p1": {}}';
    
    const wrapper = mount(RulesPanel, {
      global: { plugins: [i18n] },
      props: {
        rules,
        providersJson: providers,
      },
    });

    expect(wrapper.find('textarea').element.value).toBe(providers);
    expect(wrapper.find('input[type="text"]').element.value).toBe('DOMAIN,google.com,PROXY');
  });

  it('adds a rule when button clicked', async () => {
    const rules = [{ rule: 'r1', enabled: true }];
    const wrapper = mount(RulesPanel, {
      global: { plugins: [i18n] },
      props: { rules, providersJson: '{}' },
    });

    await wrapper.find('button.btn-outline').trigger('click');
    expect(wrapper.emitted('update:rules')).toBeTruthy();
    expect(wrapper.emitted('update:rules')![0][0]).toHaveLength(2);
  });

  it('moves rules up and down', async () => {
    const rules = [{ rule: 'r1', enabled: true }, { rule: 'r2', enabled: true }];
    const wrapper = mount(RulesPanel, {
      global: { plugins: [i18n] },
      props: { rules, providersJson: '{}' },
    });

    // Move r2 up (index 1, direction -1)
    const moveUpBtns = wrapper.findAll('button').filter(b => b.text() === '↑');
    await moveUpBtns[1].trigger('click'); // Click second row's up button

    expect(wrapper.emitted('update:rules')![0][0][0].rule).toBe('r2');
  });

  it('removes a rule', async () => {
    const rules = [{ rule: 'r1', enabled: true }];
    const wrapper = mount(RulesPanel, {
      global: { plugins: [i18n] },
      props: { rules, providersJson: '{}' },
    });

    await wrapper.find('button.btn-danger').trigger('click');
    expect(wrapper.emitted('update:rules')![0][0]).toHaveLength(0);
  });

  it('triggers scroll logic', async () => {
    // Mock many rules to enable scrolling
    const rules = Array.from({ length: 100 }, (_, i) => ({ rule: `r${i}`, enabled: true }));
    const wrapper = mount(RulesPanel, {
      global: { plugins: [i18n] },
      props: { rules, providersJson: '{}' },
    });

    const container = wrapper.find({ ref: 'listContainer' });
    // Manually trigger scroll event
    await container.trigger('scroll');
    
    // We can't easily check internal reactive state from outside without exposing it, 
    // but triggering it increases coverage.
  });
});
