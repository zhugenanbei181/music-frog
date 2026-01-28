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
        edit: 'Edit',
        external_edit: 'External Edit',
        update_now: 'Update Now',
        refresh: 'Refresh',
        clear: 'Clear',
        time_not_set: 'Not set',
        settings: 'Settings',
        save_settings: 'Save Settings',
        enable_auto_update: 'Auto Update',
        hours_48: '48 Hours',
        sub_url: '{url}',
        collapse: 'Collapse',
        hours_12: '12h',
        hours_24: '24h',
        days_7: '7d',
        subscription: 'Sub',
        next_update: 'Next: {time}'
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
    expect(wrapper.text()).toContain('Current');
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
    expect(setVisibleBtn).toBeDefined();
    await setVisibleBtn?.trigger('click');

    expect(wrapper.emitted('switch')).toBeTruthy();
    expect(wrapper.emitted('switch')![0]).toEqual(['profile2']);
  });

  it('emits delete, load, open-external, update-now events', async () => {
    const wrapper = mount(ProfilesPanel, {
      global: { plugins: [i18n] },
      props: {
        profiles: mockProfiles,
        activeCount: 1,
        filter: '',
      },
    });

    // delete profile2
    const deleteBtn = wrapper.findAll('button').find(b => b.text() === 'Delete');
    expect(deleteBtn).toBeDefined();
    await deleteBtn?.trigger('click');
    expect(wrapper.emitted('delete')![0]).toEqual(['profile2']);

    // edit profile1
    const editBtn = wrapper.findAll('button').find(b => b.text() === 'Edit');
    expect(editBtn).toBeDefined();
    await editBtn?.trigger('click');
    expect(wrapper.emitted('load')![0]).toEqual(['profile1']);

    // external edit
    const extBtn = wrapper.findAll('button').find(b => b.text() === 'External Edit');
    expect(extBtn).toBeDefined();
    await extBtn?.trigger('click');
    expect(wrapper.emitted('open-external')![0]).toEqual(['profile1']);

    // update now
    const updateBtn = wrapper.findAll('button').find(b => b.text() === 'Update Now');
    expect(updateBtn).toBeDefined();
    await updateBtn?.trigger('click');
    expect(wrapper.emitted('update-now')![0]).toEqual(['profile1']);
  });

  it('emits global refresh and clear events', async () => {
    const wrapper = mount(ProfilesPanel, {
      global: { plugins: [i18n] },
      props: { profiles: [], activeCount: 0, filter: '' },
    });

    const refreshBtn = wrapper.findAll('button').find(b => b.text() === 'Refresh');
    expect(refreshBtn).toBeDefined();
    await refreshBtn?.trigger('click');
    expect(wrapper.emitted('refresh')).toBeTruthy();

    const clearBtn = wrapper.findAll('button').find(b => b.text() === 'Clear');
    expect(clearBtn).toBeDefined();
    await clearBtn?.trigger('click');
    expect(wrapper.emitted('clear')).toBeTruthy();
  });

  it('interacts with subscription settings', async () => {
    const wrapper = mount(ProfilesPanel, {
      global: { plugins: [i18n] },
      props: {
        profiles: mockProfiles,
        activeCount: 1,
        filter: '',
      },
    });

    // Toggle settings open
    const settingsBtn = wrapper.findAll('button').find(b => b.text() === 'Settings');
    expect(settingsBtn).toBeDefined();
    await settingsBtn?.trigger('click');
    
    // Auto update is true initially from mock
    expect(wrapper.find('select').exists()).toBe(true);
    
    // Select interval
    const select = wrapper.find('select');
    await select.setValue('48');

    // Save
    const saveBtn = wrapper.findAll('button').find(b => b.text() === 'Save Settings');
    expect(saveBtn).toBeDefined();
    await saveBtn?.trigger('click');

    expect(wrapper.emitted('update-subscription')).toBeTruthy();
    expect(wrapper.emitted('update-subscription')![0][0]).toMatchObject({
        name: 'profile1',
        auto_update_enabled: true,
        update_interval_hours: 48
    });
  });
});