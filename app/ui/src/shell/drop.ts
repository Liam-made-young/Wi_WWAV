// Files dropped onto the window, as paths (2.5's import, 2.7's capture).
// The core imports by path, and a browser never tells a page a dropped
// file's path, so in the app the paths come from Tauri's own drop event.
// In a browser (the dev bridge, Playwright) a drop's file:// URLs stand in.
// Either way the drop goes to the nearest element marked data-drop, and the
// listener is handed that element's other data-* values with it: a course's
// card in Grades is data-drop="syllabus" with its data-course-id.

import { getCurrentWebview } from '@tauri-apps/api/webview';

export type DropTarget = 'capture' | 'drawer' | 'import' | 'syllabus';

/** A drop's target, the paths dropped, and what the marked element says of itself (`data.courseId`). */
export type DropHandler = (target: DropTarget, paths: string[], data: DOMStringMap) => void;

/** Paths from a drop's file:// URLs. */
export function pathsFromUris(uriList: string): string[] {
  return uriList
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.startsWith('file://'))
    .map((uri) => decodeURIComponent(new URL(uri).pathname));
}

function targetAt(el: Element | null): { target: DropTarget; data: DOMStringMap } | null {
  const marked = el?.closest<HTMLElement>('[data-drop]');
  return marked ? { target: marked.getAttribute('data-drop') as DropTarget, data: marked.dataset } : null;
}

/** Hears drops until the returned function is called. More than one listener may hear; each takes the targets it knows. */
export function listenForDrops(onDrop: DropHandler): () => void {
  if ('__TAURI_INTERNALS__' in window) {
    const off = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type !== 'drop') return;
      const scale = window.devicePixelRatio || 1;
      const at = targetAt(document.elementFromPoint(payload.position.x / scale, payload.position.y / scale));
      if (at && payload.paths.length) onDrop(at.target, payload.paths, at.data);
    });
    return () => void off.then((stop) => stop());
  }
  const over = (e: DragEvent) => {
    if (targetAt(e.target as Element)) e.preventDefault();
  };
  const drop = (e: DragEvent) => {
    const at = targetAt(e.target as Element);
    if (!at) return;
    e.preventDefault();
    const paths = pathsFromUris(e.dataTransfer?.getData('text/uri-list') ?? '');
    if (paths.length) onDrop(at.target, paths, at.data);
  };
  window.addEventListener('dragover', over);
  window.addEventListener('drop', drop);
  return () => {
    window.removeEventListener('dragover', over);
    window.removeEventListener('drop', drop);
  };
}
