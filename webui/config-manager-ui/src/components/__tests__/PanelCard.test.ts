import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import PanelCard from '../PanelCard.vue';

describe('PanelCard', () => {
  it('renders slot content', () => {
    const wrapper = mount(PanelCard, {
      slots: {
        default: '<div class="test-content">Hello World</div>',
      },
    });

    expect(wrapper.find('.test-content').exists()).toBe(true);
    expect(wrapper.text()).toContain('Hello World');
  });

  it('renders as specified component', () => {
    const wrapper = mount(PanelCard, {
      props: {
        as: 'section',
      },
    });

    expect(wrapper.element.tagName.toLowerCase()).toBe('section');
  });

  it('has panel class', () => {
    const wrapper = mount(PanelCard);
    expect(wrapper.find('.panel').exists()).toBe(true);
  });
});
