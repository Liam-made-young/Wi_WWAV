// Files dropped onto the window, as paths (2.5's import, 2.7's capture).
// The core imports by path, and a browser never tells a page a dropped
// file's path, so in the app the paths come from Tauri's own drop event.
// In a browser (the dev bridge, Playwright) a drop's file:// URLs stand in.
// Either way the drop goes to the nearest element marked data-drop.

import { getCurrentWebview } from '@tauri-apps/api/webview';

export type DropTarget = 'capture' | 'drawer' | 'import';

/** Paths from a drop's file:// URLs. */
export function pathsFromUris(uriList: string): string[] {
  return uriList
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.startsWith('file://'))
    .map((uri) => decodeURIComponent(new URL(uri).pathname));
}

function targetAt(el: Element | null): DropTarget | null {
  const marked = el?.closest('[data-drop]');
  return marked ? (marked.getAttribute('data-drop') as DropTarget) : null;
}

/** Hears drops until the returned function is called. */
export function listenForDrops(onDrop: (target: DropTarget, paths: string[]) => void): () => void {
  if ('__TAURI_INTERNALS__' in window) {
    const off = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type !== 'drop') return;
      const scale = window.devicePixelRatio || 1;
      const target = targetAt(document.elementFromPoint(payload.position.x / scale, payload.position.y / scale));
      if (target && payload.paths.length) onDrop(target, payload.paths);
    });
    return () => void off.then((stop) => stop());
  }
  const over = (e: DragEvent) => {
    if (targetAt(e.target as Element)) e.preventDefault();
  };
  const drop = (e: DragEvent) => {
    const target = targetAt(e.target as Element);
    if (!target) return;
    e.preventDefault();
    const paths = pathsFromUris(e.dataTransfer?.getData('text/uri-list') ?? '');
    if (paths.length) onDrop(target, paths);
  };
  window.addEventListener('dragover', over);
  window.addEventListener('drop', drop);
  return () => {
    window.removeEventListener('dragover', over);
    window.removeEventListener('drop', drop);
  };
}
