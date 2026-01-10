import { ref } from 'vue';

export type ToastTone = 'info' | 'error' | 'success' | 'warning';
export type Toast = { id: number; message: string; tone: ToastTone };

export function useToasts() {
  const toasts = ref<Toast[]>([]);
  let toastCounter = 0;

  function pushToast(message: string, tone: ToastTone = 'info') {
    const id = ++toastCounter;
    toasts.value.push({ id, message, tone });
    setTimeout(() => {
      toasts.value = toasts.value.filter((toast) => toast.id !== id);
    }, 4200);
  }

  return {
    toasts,
    pushToast,
  };
}
