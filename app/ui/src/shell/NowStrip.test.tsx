import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { freshMix } from '../shared/stems/mix';
import { NowStrip } from './NowStrip';
import { NO_PLAYER, type PlayerState } from './strip';
import type { Player } from './usePlayer';

// The strip's variants that the end-to-end tests can't reach yet, because
// the rooms that set them aren't built: the Console's session line, and a
// player paused for the Console. What a fail looks like: the Console line
// not shown in the Console, or shown elsewhere; "Paused for the Console"
// not said; the narrow strip's time not fitting its 43.5 pt.

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const song: PlayerState = {
  ...NO_PLAYER,
  clip: '01J',
  title: 'World Ending',
  key: 'A minor',
  bpm: 128,
  position: 102,
  duration: 238,
  stems: (['vocals', 'drums', 'other', 'bass'] as const).map((stem) => ({ stem, level: 1, mute: false, solo: false })),
};

const player = (state: PlayerState): Player => ({
  state,
  anchor: { seconds: state.position, at: 0, speed: 0 },
  mix: state.stems.length ? freshMix(1) : null,
  stems: async () => {},
  playPause: async () => {},
  load: async () => {},
});

const task = { taskId: null, line1: 'All clear', line2: 'Nothing open right now.', meter: 0, level: null };

let host: HTMLElement;
let root: Root;
beforeEach(() => {
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

function render(state: PlayerState, room: 'heat' | 'console', narrow = false) {
  act(() =>
    root.render(
      <NowStrip
        narrow={narrow}
        task={task}
        player={player(state)}
        room={room}
        consoleTransport={{ bar: 42, beat: 3, bpm: 128, armed: true }}
        onTask={() => {}}
        onExpand={() => {}}
      />,
    ),
  );
  return host.querySelector('.strip-track')!;
}

describe('the Now strip’s track half', () => {
  it('shows the session in the Console, and the listening player everywhere else', () => {
    expect(render(song, 'console').textContent).toBe('BAR 42.3 · 128.00 BPM · REC ARMED');
    expect(render(song, 'heat').textContent).toContain('World Ending');
    expect(render(song, 'heat').textContent).toContain('1:42 / 3:58');
  });

  it('says when the Console paused the player', () => {
    expect(render({ ...song, pausedFor: 'console' }, 'heat').querySelector('.strip-title')!.textContent).toBe(
      'Paused for the Console',
    );
  });

  it('keeps only the elapsed time in the narrow strip, and four labelled lights at both widths', () => {
    const narrow = render(song, 'heat', true);
    expect(narrow.querySelector('.strip-time')!.textContent).toBe('1:42');
    expect([...narrow.querySelectorAll('[data-stem]')].map((b) => b.getAttribute('aria-label'))).toEqual([
      'Vocals, 100 percent, audible',
      'Drums, 100 percent, audible',
      'Other, 100 percent, audible',
      'Bass, 100 percent, audible',
    ]);
    expect((narrow.querySelector('[data-stem="vocals"]') as HTMLElement).style.left).toBe('0px');
    expect((narrow.querySelector('[data-stem="bass"]') as HTMLElement).style.width).toBe('44px');
  });

  it('shows a song with no stems as its master, with no lights', () => {
    const master = render({ ...song, stems: [] }, 'heat');
    expect(master.querySelectorAll('[data-stem]')).toHaveLength(0);
    expect(master.querySelector('.strip-time')!.textContent).toBe('1:42 / 3:58');
  });
});
