import { nextTick, onBeforeUnmount, onMounted } from 'vue';

type PanelRef = { value: HTMLElement | null };

function resolveAnchor(hash: string): string {
  if (!hash) {
    return '';
  }
  try {
    return decodeURIComponent(hash.replace(/^#/, '').trim());
  } catch {
    return '';
  }
}

type PanelNavigatorOptions = {
  onActivate?: (anchor: string) => void;
};

export function usePanelNavigator(
  panelMap: Record<string, PanelRef>,
  options: PanelNavigatorOptions = {},
) {
  function scrollToAnchor(anchor: string) {
    const target = panelMap[anchor]?.value;
    if (!target) {
      return;
    }
    target.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  function handleHash() {
    const anchor = resolveAnchor(window.location.hash);
    if (!anchor) {
      return;
    }
    options.onActivate?.(anchor);
    nextTick(() => {
      scrollToAnchor(anchor);
    });
  }

  onMounted(() => {
    handleHash();
    window.addEventListener('hashchange', handleHash);
  });

  onBeforeUnmount(() => {
    window.removeEventListener('hashchange', handleHash);
  });
}
