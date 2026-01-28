import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import FormSwitch from '../FormSwitch.vue';

describe('FormSwitch', () => {
  it('renders label and description', () => {
    const wrapper = mount(FormSwitch, {
      props: {
        modelValue: false,
        label: 'Test Label',
        description: 'Test Description',
      },
    });

    expect(wrapper.text()).toContain('Test Label');
    expect(wrapper.text()).toContain('Test Description');
  });

  it('emits update event on click', async () => {
    const wrapper = mount(FormSwitch, {
      props: {
        modelValue: false,
      },
    });

    const input = wrapper.find('input[type="checkbox"]');
    await input.setChecked(true);

    expect(wrapper.emitted('update:modelValue')).toBeTruthy();
    expect(wrapper.emitted('update:modelValue')![0]).toEqual([true]);
  });

  it('applies correct classes based on modelValue', () => {
    const wrapperOn = mount(FormSwitch, {
      props: { modelValue: true },
    });
    // Check classes on the div container of the switch
    expect(wrapperOn.find('.relative').classes()).toContain('bg-accent-500');

    const wrapperOff = mount(FormSwitch, {
      props: { modelValue: false },
    });
    expect(wrapperOff.find('.relative').classes()).toContain('bg-ink-500/20');
  });
});
