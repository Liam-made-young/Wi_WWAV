import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// The Console's frame (docs/SPEC.md 5.2; PLAN, the shell for all three
// views). What a fail looks like: a region missing; the browser not listing
// the real library; ⌥⌘B, ⌘I or ⌥1–⌥3 not toggling their region (the open
// panel again closing it); a control the Console stage fills that looks
// usable; the engine's state not said.

vi.mock('../bridge', () => ({
  call: vi.fn(async (cmd: string) => {
    if (cmd === 'library.list') return { clips: [{ id: '01J', kind: 'wwav', title: 'Low Tide', key: 'A minor', bpm: 86 }] };
    if (cmd === 'engine.status') return { state: 'stopped', sampleRate: null, block: null };
    throw new Error(`unexpected ${cmd}`);
  }),
  on: vi.fn(() => () => {}),
}));

import { ConsoleView } from './ConsoleView';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement;
let root: Root;

beforeEach(async () => {
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root.render(<ConsoleView active />));
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

const region = (label: string) => host.querySelector(`[aria-label="${label}"]`);
const press = async (init: KeyboardEventInit) => {
  await act(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, ...init }));
  });
};

describe('the Console frame', () => {
  it('has its regions and the OUTPUT row', () => {
    for (const label of ['Transport', 'Browser', 'Arrangement', 'Inspector', 'Bottom panel']) {
      expect(region(label), label).not.toBeNull();
    }
    expect(host.querySelector('.output-row')?.textContent).toContain('OUTPUT');
  });

  it('lists the real library in the browser', () => {
    expect(region('Browser')?.textContent).toContain('Low Tide');
    expect(region('Browser')?.textContent).toContain('A minor · 86 BPM');
  });

  it('says the engine is off', () => {
    expect(host.querySelector('.transport-engine')?.textContent).toBe('ENGINE OFF');
  });

  it('toggles the browser with ⌥⌘B, the inspector with ⌘I, and the bottom panel with ⌥1–⌥3', async () => {
    await press({ code: 'KeyB', metaKey: true, altKey: true });
    expect(region('Browser')).toBeNull();
    await press({ code: 'KeyB', metaKey: true, altKey: true });
    expect(region('Browser')).not.toBeNull();

    await press({ code: 'KeyI', metaKey: true });
    expect(region('Inspector')).toBeNull();

    await press({ code: 'Digit1', altKey: true });
    expect(region('Bottom panel')?.textContent).toContain('piano roll');
    await press({ code: 'Digit1', altKey: true });
    expect(region('Bottom panel')).toBeNull();
  });

  it("shows what the Console stage fills as disabled, with the sentence that says so", () => {
    const transport = [...host.querySelectorAll<HTMLButtonElement>('.transport-buttons button')];
    expect(transport).toHaveLength(4);
    for (const b of transport) {
      expect(b.disabled).toBe(true);
      expect(b.title).toBe('Arrives with the Console stage.');
    }
  });
});
