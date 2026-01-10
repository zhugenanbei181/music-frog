import { ref } from 'vue';

export function useBusyState() {
  const busy = ref(false);
  const busyMessage = ref('');
  const busyDetail = ref('');

  function startBusy(message: string, detail: string) {
    busy.value = true;
    busyMessage.value = message;
    busyDetail.value = detail;
  }

  function updateBusyDetail(detail: string) {
    if (busy.value) {
      busyDetail.value = detail;
    }
  }

  function endBusy() {
    busy.value = false;
    busyMessage.value = '';
    busyDetail.value = '';
  }

  return {
    busy,
    busyMessage,
    busyDetail,
    startBusy,
    updateBusyDetail,
    endBusy,
  };
}
