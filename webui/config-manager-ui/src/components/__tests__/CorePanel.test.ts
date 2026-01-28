import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import CorePanel from '../CorePanel.vue';

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: {
    en: {
      core: {
        title: 'Core Manager',
        refresh: 'Refresh',
        current: 'Current: {version}',
        default: 'Bundled Core',
        active: 'Active',
        switchable: 'Installed',
        status_current: 'Using',
        status_use: 'Use',
        empty: 'No versions found'
      }
    }
  }
});

describe('CorePanel', () => {
  it('renders current version correctly', () => {
    const wrapper = mount(CorePanel, {
      global: { plugins: [i18n] },
      props: {
        coreVersions: ['v1.18.0', 'v1.19.0'],
        coreCurrent: 'v1.19.0',
      },
    });

    expect(wrapper.text()).toContain('Current: v1.19.0');
    expect(wrapper.findAll('li').length).toBe(2);
  });

  it('shows empty message when no versions', () => {
    const wrapper = mount(CorePanel, {
      global: { plugins: [i18n] },
      props: {
        coreVersions: [],
        coreCurrent: null,
      },
    });

    expect(wrapper.text()).toContain('Bundled Core');
    expect(wrapper.text()).toContain('No versions found');
  });

  it('emits activate event when button clicked', async () => {
    const wrapper = mount(CorePanel, {
      global: { plugins: [i18n] },
      props: {
        coreVersions: ['v1.18.0'],
        coreCurrent: 'v1.19.0',
      },
    });

    const useBtn = wrapper.find('button.btn-primary');
    await useBtn.trigger('click');

    expect(wrapper.emitted('activate')).toBeTruthy();
    expect(wrapper.emitted('activate')![0]).toEqual(['v1.18.0']);
  });
});
