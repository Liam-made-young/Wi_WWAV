import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Document, Workspace } from './client';

const state = vi.hoisted(() => ({ workspace: null as Workspace | null, documents: [] as Document[], calls: [] as string[], changed: () => {} }));
vi.mock('../bridge', () => ({ on: (_name: string, f: () => void) => { state.changed = f; return () => {}; } }));
vi.mock('./write/client', () => ({ write: {
  read: async (id: string) => ({ document: state.documents.find(d => d.id === id), base: 'v1', manuscript: { format: 'wi-write/1', mode: 'prose', sections: [{ id: 'section', parent: null, title: 'Draft', kind: 'section', synopsis: '', text: 'A first line' }], notes: '', research: [] }, stats: { words: 3, sections: [] }, view: {} }),
  render: async () => ({ html: '<p>A first line</p>', stats: { words: 3, lines: [], outline: [] } }),
  view: async () => {},
} }));
vi.mock('./write/Editor', () => ({ Editor: (p: { text: string; onChange(text: string): void }) => <textarea aria-label="Document text" value={p.text} onChange={e => p.onChange(e.target.value)} /> }));
vi.mock('./client', () => ({
  errorText: (e: Error) => e.message,
  api: {
    workspace: async () => ({ workspace: structuredClone(state.workspace), root: '/Library' }),
    library: async () => ({ documents: structuredClone(state.documents), root: '/Library', issues: [] }),
    select: async (tool: Workspace['tool']) => { state.calls.push(`select:${tool}`); state.workspace!.tool = tool; },
    read: async (id: string) => { const d = state.documents.find(d => d.id === id)!; return { document: d, version: { id: d.head, title: d.title, asset: d.asset }, text: 'A first line', base64: null, path: '/Library/draft.md' }; },
    selection: async () => {},
    save: async () => {},
  },
}));
import { ConsoleView } from './ConsoleView';
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
let host: HTMLElement;
let root: Root;
beforeEach(async () => {
  state.calls = [];
  state.workspace = { tool: 'write', tools: { write: { open: ['one'], active: 'one', selection: null }, image: { open: [], active: null, selection: null }, audiovisual: { open: [], active: null, selection: null }, three: { open: [], active: null, selection: null } } };
  state.documents = [{ id: 'one', tool: 'write', title: 'First work', head: 'v1', updatedAt: 0, versions: 1, parent: null, marker: 'Made by hand', canUndo: false, canRedo: false, asset: { name: 'draft.md', file: 'draft.md', mime: 'text/plain', bytes: 12, sha256: '', preview: 'A first line' } }];
  host = document.createElement('div'); document.body.append(host); root = createRoot(host);
  await act(async () => { root.render(<ConsoleView active />); });
});
afterEach(() => { act(() => root.unmount()); host.remove(); });
const button = (label: string) => host.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!;
const key = async (target: EventTarget, init: KeyboardEventInit) => { await act(async () => { target.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init })); }); };

describe('Console phase 0', () => {
  it('has four independent tool panels and the real Library preview', () => {
    expect(host.querySelectorAll('[role="tab"]')).toHaveLength(4);
    expect(host.querySelector('[aria-label="Console Library"]')?.textContent).toContain('First work');
    expect(host.querySelector('[aria-label="Console Library"]')?.textContent).toContain('A first line');
    expect(button('Undo document edit').disabled).toBe(true);
  });
  it('preserves an unsaved draft and the same textarea across switches', async () => {
    const text = host.querySelector<HTMLTextAreaElement>('[aria-label="Document text"]')!;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')!.set!.call(text, 'Unsaved words');
      text.dispatchEvent(new Event('input', { bubbles: true }));
    });
    text.blur(); await key(document.body, { key: '2', code: 'Digit2' });
    expect(state.workspace!.tool).toBe('image');
    await key(document.body, { key: '1', code: 'Digit1' });
    expect(host.querySelector('[aria-label="Document text"]')).toBe(text);
    expect(text.value).toBe('Unsaved words');
    expect(button('Save as variation').disabled).toBe(true);
  });
  it('leaves digits in text fields and keys in another view alone', async () => {
    const text = host.querySelector('[aria-label="Document text"]')!;
    await key(text, { key: '3', code: 'Digit3' });
    expect(state.calls).toEqual([]);
    const external = document.createElement('input'); document.body.append(external);
    await key(external, { key: '4', code: 'Digit4' }); external.remove();
    expect(state.calls).toEqual([]);
  });
  it('opens the shared Claude entry with Command-T and closes on Escape', async () => {
    await key(document.body, { key: 't', code: 'KeyT', metaKey: true });
    expect(host.querySelector('[aria-label="Claude"]')?.textContent).toContain('First work');
    await key(document.body, { key: 'Escape', code: 'Escape' });
    expect(host.querySelector('[aria-label="Claude"]')).toBeNull();
  });
  it('does not switch tools behind a shell overlay', async () => {
    const overlay = document.createElement('div'); overlay.className = 'backdrop'; overlay.dataset.overlay = 'palette'; document.body.append(overlay);
    await key(document.body, { key: '2', code: 'Digit2' }); overlay.remove();
    expect(state.calls).toEqual([]);
  });
});
