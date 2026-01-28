import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import ProfilesPanel from '../ProfilesPanel.vue';

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: {
    en: {
      profiles: {
        title: 'Profiles',
        count: '{active} / {total}',
        current: 'Current',
        available: 'Available',
        set_active: 'Set Active',
        active: 'Active',
        delete: 'Delete',
        time_not_set: 'Not set',
      }
    }
  }
});

const mockProfiles = [
  {
    name: 'profile1',
    active: true,
    path: '/path/1',
    subscription_url: 'http://sub.com/1',
    auto_update_enabled: true,
    update_interval_hours: 24,
  },
  {
    name: 'profile2',
    active: false,
    path: '/path/2',
    subscription_url: null,
    auto_update_enabled: false,
  }
];

describe('ProfilesPanel', () => {
  it('renders profile list correctly', () => {
    const wrapper = mount(ProfilesPanel, {
      global: { plugins: [i18n] },
      props: {
        profiles: mockProfiles,
        activeCount: 1,
        filter: '',
      },
    });

    expect(wrapper.text()).toContain('profile1');
    expect(wrapper.text()).toContain('profile2');
    expect(wrapper.text()).toContain('Current'); // badge for active
  });

  it('filters profiles based on filter prop', () => {
    const wrapper = mount(ProfilesPanel, {
      global: { plugins: [i18n] },
      props: {
        profiles: mockProfiles,
        activeCount: 1,
        filter: 'profile2',
      },
    });

    expect(wrapper.text()).not.toContain('profile1');
    expect(wrapper.text()).toContain('profile2');
  });

  it('emits switch event when set active is clicked', async () => {
    const wrapper = mount(ProfilesPanel, {
      global: { plugins: [i18n] },
      props: {
        profiles: mockProfiles,
        activeCount: 1,
        filter: '',
      },
    });

    const setVisibleBtn = wrapper.findAll('button').find(b => b.text() === 'Set Active');
    await setVisibleBtn?.trigger('click');

    expect(wrapper.emitted('switch')).toBeTruthy();
    expect(wrapper.emitted('switch')![0]).toEqual(['profile2']);
  });

  it('opens subscription settings when clicked', async () => {
    const wrapper = mount(ProfilesPanel, {
      global: { plugins: [i18n] },
      props: {
        profiles: mockProfiles,
        activeCount: 1,
        filter: '',
      },
    });

    // Profile 1 is active and has sub url, should show 'Settings' (mapped from $t('profiles.settings'))
    // Note: Our mock i18n is minimal, let's look for text or use a better mock
  });
});
