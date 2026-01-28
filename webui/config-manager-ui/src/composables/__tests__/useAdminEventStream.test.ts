import { describe, expect, it, vi, beforeEach } from 'vitest';
import { ref, nextTick, defineComponent, h } from 'vue';
import { mount } from '@vue/test-utils';
import { useAdminEventStream } from '../useAdminEventStream';

const mockEventSourceInstances: any[] = [];

// Properly mock EventSource as a class/constructor
const MockEventSource = vi.fn().mockImplementation(function(this: any, url: string) {
  this.url = url;
  this.onmessage = null;
  this.close = vi.fn();
  this.emit = (kind: string) => {
    if (this.onmessage) {
      this.onmessage({ data: JSON.stringify({ kind }) });
    }
  };
  mockEventSourceInstances.push(this);
  return this;
});

vi.stubGlobal('EventSource', MockEventSource);

describe('useAdminEventStream', () => {
  let options: any;

  beforeEach(() => {
    mockEventSourceInstances.length = 0;
    vi.clearAllMocks();
    options = {
      busy: ref(false),
      hasUnsavedChanges: ref(false),
      refresh: vi.fn().mockResolvedValue(undefined),
    };
  });

  const TestComponent = defineComponent({
    setup() {
      const stream = useAdminEventStream(options);
      return { ...stream };
    },
    render() { return h('div'); }
  });

  it('connects to EventSource on mount and refreshes on message', async () => {
    mount(TestComponent);
    
    const es = mockEventSourceInstances[0];
    expect(es).toBeDefined();
    
    es.emit('profiles-changed');
    await nextTick();
    await nextTick();
    
    expect(options.refresh).toHaveBeenCalled();
  });

  it('sets hasPendingUpdates if user has unsaved changes', async () => {
    options.hasUnsavedChanges.value = true;
    const wrapper = mount(TestComponent);
    const es = mockEventSourceInstances[0];

    es.emit('settings-changed');
    await nextTick();
    await nextTick();

    expect(options.refresh).not.toHaveBeenCalled();
    expect(wrapper.vm.hasPendingUpdates).toBe(true);
  });

  it('queues refresh if system is busy', async () => {
    options.busy.value = true;
    const wrapper = mount(TestComponent);
    const es = mockEventSourceInstances[0];

    es.emit('dns-changed');
    await nextTick();
    await nextTick();

    expect(options.refresh).not.toHaveBeenCalled();
    
    // Stop being busy
    options.busy.value = false;
    await nextTick();
    await nextTick();
    
    expect(options.refresh).toHaveBeenCalled();
  });
});
