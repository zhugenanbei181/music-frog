import { onBeforeUnmount, onMounted, ref, watch, type Ref } from 'vue';
import { adminEventsUrl } from '../api';
import type { AdminEvent } from '../types';

const REFRESH_EVENT_KINDS = new Set([
  'rebuild-finished',
  'rebuild-failed',
  'profiles-changed',
  'core-changed',
  'settings-changed',
  'dns-changed',
  'fake-ip-changed',
  'rules-changed',
  'rule-providers-changed',
  'tun-changed',
  'webdav-synced',
]);

type AdminEventOptions = {
  busy: Ref<boolean>;
  hasUnsavedChanges: Ref<boolean>;
  refresh: () => Promise<void>;
};

export function useAdminEventStream(options: AdminEventOptions) {
  const hasPendingUpdates = ref(false);
  let source: EventSource | null = null;
  let refreshInFlight = false;
  let pendingRefresh = false;

  async function triggerRefresh(force: boolean) {
    if (!force && options.hasUnsavedChanges.value) {
      hasPendingUpdates.value = true;
      return;
    }
    if (!force && options.busy.value) {
      pendingRefresh = true;
      return;
    }
    if (refreshInFlight) {
      pendingRefresh = true;
      return;
    }
    refreshInFlight = true;
    try {
      await options.refresh();
      hasPendingUpdates.value = false;
    } catch {
      hasPendingUpdates.value = true;
    } finally {
      refreshInFlight = false;
      if (pendingRefresh) {
        pendingRefresh = false;
        await triggerRefresh(false);
      }
    }
  }

  function handleMessage(event: MessageEvent) {
    let payload: AdminEvent | null = null;
    try {
      payload = JSON.parse(event.data) as AdminEvent;
    } catch {
      return;
    }
    if (!payload || typeof payload.kind !== 'string') {
      return;
    }
    if (!REFRESH_EVENT_KINDS.has(payload.kind)) {
      return;
    }
    void triggerRefresh(false);
  }

  function refreshPendingUpdates() {
    return triggerRefresh(true);
  }

  function clearPendingUpdates() {
    hasPendingUpdates.value = false;
  }

  onMounted(() => {
    source = new EventSource(adminEventsUrl);
    source.onmessage = handleMessage;
  });

  watch([options.busy, options.hasUnsavedChanges], ([busy, hasChanges]) => {
    if (!busy && !hasChanges && pendingRefresh && !refreshInFlight) {
      pendingRefresh = false;
      void triggerRefresh(false);
    }
  });

  onBeforeUnmount(() => {
    source?.close();
    source = null;
  });

  return {
    hasPendingUpdates,
    refreshPendingUpdates,
    clearPendingUpdates,
  };
}
