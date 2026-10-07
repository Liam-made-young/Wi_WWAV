import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GALAXIES } from './model/fixtures/catalogue';

// Space's frame (docs/SPEC.md 4.3; PLAN, the shell for all three views).
// What a fail looks like: the sky not filling the view; the breadcrumb, the
// info card, Newest or − · + · fit missing; a sample sky not marked as one;
// a click that doesn't select or Esc that doesn't clear; zoom not stepping
// by 1.55; a machine without 3D left with a blank view and no sentence.

const fake = vi.hoisted(() => ({
  zooms: [] as number[],
  fits: 0,
  fail: false,
}));

vi.mock('./sky', () => ({
  ZOOM_STEP: 1.55,
  createSky: () => {
    if (fake.fail) throw new Error('no WebGL');
    return {
      render() {},
      resize() {},
      zoom(f: number) {
        fake.zooms.push(f);
      },
      fit() {
        fake.fits += 1;
      },
      project: () => [{ id: GALAXIES[0].id, name: GALAXIES[0].displayName, x: 100, y: 100, visible: true }],
      pick: () => GALAXIES[0],
      dispose() {},
    };
  },
}));

import { SpaceView } from './SpaceView';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement;
let root: Root;

async function show() {
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root.render(<SpaceView active />));
}

beforeEach(() => {
  fake.zooms = [];
  fake.fits = 0;
  fake.fail = false;
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe('the Space frame', () => {
  it('has the breadcrumb, a marked sample sky, the card, Newest and the zoom buttons', async () => {
    await show();
    expect(host.querySelector('.space-breadcrumb')?.textContent).toContain('Everyone');
    expect(host.querySelector('.space-sample')?.textContent).toBe('Sample sky');
    expect(host.querySelector('.card-empty')?.textContent).toBe('Select a galaxy to see whose it is.');
    expect(host.querySelector<HTMLButtonElement>('.space-newest')?.disabled).toBe(true);
    expect(host.querySelectorAll('.space-zoom button')).toHaveLength(3);
    expect(host.querySelector('.space-label')?.textContent).toBe(GALAXIES[0].displayName);
  });

  it('selects on click, clears on Esc, and zooms by 1.55', async () => {
    await show();
    await act(async () => {
      host.querySelector<HTMLElement>('.space-sky')!.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: 100, clientY: 100 }));
    });
    expect(host.querySelector('.card-title')?.textContent).toBe(GALAXIES[0].displayName);
    await act(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(host.querySelector('.card-title')).toBeNull();

    const [out, into, fit] = host.querySelectorAll<HTMLButtonElement>('.space-zoom button');
    await act(async () => {
      into.click();
      out.click();
      fit.click();
    });
    expect(fake.zooms).toEqual([1.55, 1 / 1.55]);
    expect(fake.fits).toBe(1);
  });

  it('says so in a sentence when this machine can not draw 3D', async () => {
    fake.fail = true;
    await show();
    expect(host.querySelector('.space-failed')?.textContent).toContain("couldn't draw the sky");
  });
});
