import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { provingGround } from './flight/ground';

// Space's frame (docs/SPACE.md 1, 10). What a fail looks like: a proving
// ground not marked as one; a body with no name in the sky; nothing saying
// how to move; a Mac without 3D left with a blank view and no sentence; the
// app's order to fly somewhere ignored, or you arriving in a page with no way
// shown to back out of it; home known to the core and not in the sky, or you
// not starting there.
//
// The old frame's test (a sample sky seen from above, with − · + · fit) went
// with that frame on 8 Oct 2026, when Space became first person.

const fake = vi.hoisted(() => ({
  fail: false,
  draws: 0,
  home: null as unknown,
  heard: new Map<string, (payload: unknown) => void>(),
}));

vi.mock('./flight/scene', () => ({
  createSky: () => {
    if (fake.fail) throw new Error('no WebGL');
    return {
      resize() {},
      draw() {
        fake.draws += 1;
      },
      pick: () => null,
      picture() {},
      dispose() {},
    };
  },
}));

vi.mock('../bridge', () => ({
  call: vi.fn(async (cmd: string) => {
    if (cmd !== 'space.home') return {};
    if (!fake.home) throw new Error('no connection, and nothing kept');
    return { home: fake.home, fresh: true };
  }),
  on: (event: string, handler: (payload: unknown) => void) => {
    fake.heard.set(event, handler);
    return () => fake.heard.delete(event);
  },
}));

import { SpaceView } from './SpaceView';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let root: Root;

async function show() {
  await act(async () => root.render(<SpaceView active />));
}

/** Lets the sky run for a while: frames come every 16 ms of the fake clock. */
async function fly(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['requestAnimationFrame', 'cancelAnimationFrame', 'performance', 'setTimeout', 'clearTimeout'] });
  fake.fail = false;
  fake.draws = 0;
  fake.home = null;
  fake.heard.clear();
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.useRealTimers();
});

describe('Space, in first person', () => {
  it('says it is a proving ground, names every body in the sky, and says how to move', async () => {
    await show();
    expect(host.querySelector('.space-sample')?.textContent).toBe('Proving ground');
    const names = [...host.querySelectorAll('.space-label')].map((el) => el.textContent);
    expect(names).toEqual(provingGround().map((b) => b.name));
    expect(host.querySelector('.space-hint')?.textContent).toContain('W A S D move · Shift to hurry');
    expect(host.querySelector('.space-hint')?.textContent).toContain('Pages open in the Mac app.');
    expect(host.querySelector('.space-sky')?.getAttribute('aria-label')).toContain('Return to go in');
  });

  it('draws the sky again and again while it is showing, and starts you facing the biggest body', async () => {
    await show();
    await fly(200);
    expect(fake.draws).toBeGreaterThan(5);
    expect(host.querySelector('.space-where strong')?.textContent).toBe('YouTube');
    expect(host.querySelector('.space-acts button')?.textContent).toBe('Go in');
  });

  it('says so when this Mac cannot draw in 3D', async () => {
    fake.fail = true;
    await show();
    expect(host.querySelector('.space-failed')?.textContent).toBe("This Mac couldn't draw the sky in 3D.");
  });

  it('flies you to a link the app names, into its page, and shows the way back out', async () => {
    await show();
    await fly(100);
    const saturn = provingGround().find((b) => b.id === 'wikipedia-saturn')!;
    await act(async () => fake.heard.get('space.fly')?.({ url: saturn.url, enter: true }));
    await fly(6000);
    expect(host.querySelector('.space-where strong')?.textContent).toBe('Saturn');
    const acts = [...host.querySelectorAll('.space-acts button')].map((b) => b.textContent);
    expect(acts).toEqual(['‹', '›', 'Back out']);
    expect(host.querySelector('.space-hint')).toBeNull();

    // What the page says brings you out again: Esc inside it, or ⌘2.
    await act(async () => fake.heard.get('space.page')?.({ what: 'said', said: { leave: 1 } }));
    await fly(3000);
    expect(host.querySelector('.space-acts button')?.textContent).toBe('Go in');
    expect(host.querySelector('.space-hint')).not.toBeNull();
  });

  it('puts home in the sky when the core knows it, starts you facing its sun, and is no longer only a proving ground', async () => {
    fake.home = {
      slug: 'someone',
      url: 'https://www.mi-wwav.com/summer_26/g/someone',
      name: 'someone',
      systems: [
        {
          url: 'https://www.mi-wwav.com/summer_26/g/someone/s/first',
          name: 'First album',
          x: 2932,
          y: 3018,
          worlds: [{ url: 'https://www.mi-wwav.com/summer_26/play/t1', name: 'A song', kind: 'song', order: 0, radius: 800, phase: 1, palette: null }],
        },
      ],
    };
    await show();
    await fly(200);
    const names = [...host.querySelectorAll('.space-label')].map((el) => el.textContent);
    expect(names.slice(0, 3)).toEqual(['someone', 'First album', 'A song']);
    expect(names).toContain('YouTube');
    expect(host.querySelector('.space-sample')).toBeNull();
    expect(host.querySelector('.space-where strong')?.textContent).toBe('someone');
  });

  it('ignores an order to fly to a link that is not in the sky', async () => {
    await show();
    await act(async () => fake.heard.get('space.fly')?.({ url: 'https://nowhere.example/', enter: true }));
    await fly(3000);
    expect(host.querySelector('.space-acts button')?.textContent).toBe('Go in');
  });
});
