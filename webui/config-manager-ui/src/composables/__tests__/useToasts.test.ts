import { describe, expect, it, vi } from 'vitest';
import { useToasts } from '../useToasts';

describe('useToasts', () => {
  it('adds a toast to the list', () => {
    const { toasts, pushToast } = useToasts();
    
    pushToast('hello', 'success');
    
    expect(toasts.value.length).toBe(1);
    expect(toasts.value[0]).toMatchObject({
      message: 'hello',
      tone: 'success',
    });
  });

  it('removes toast after timeout', () => {
    vi.useFakeTimers();
    const { toasts, pushToast } = useToasts();
    
    pushToast('bye');
    expect(toasts.value.length).toBe(1);
    
    vi.advanceTimersByTime(4200);
    expect(toasts.value.length).toBe(0);
    
    vi.useRealTimers();
  });

  it('maintains multiple toasts and removes them independently', () => {
    vi.useFakeTimers();
    const { toasts, pushToast } = useToasts();
    
    pushToast('first');
    vi.advanceTimersByTime(1000);
    pushToast('second');
    
    expect(toasts.value.length).toBe(2);
    
    vi.advanceTimersByTime(3200); // Total 4200 for 'first'
    expect(toasts.value.length).toBe(1);
    expect(toasts.value[0].message).toBe('second');
    
    vi.advanceTimersByTime(1000); // Total 4200 for 'second'
    expect(toasts.value.length).toBe(0);
    
    vi.useRealTimers();
  });
});
