import { describe, expect, it } from 'vitest';
import { applyToMix, freshMix, voiceLabel } from '../shared/stems/mix';
import { consoleLine, EMPTY_TRACK, elapsed, mixOf, NO_PLAYER, pausedLine, stemCalls, trackTime } from './strip';

// 2.2's track half. What a fail looks like: a time that reads other than
// "1:42 / 3:58"; a narrow strip that can't fit its time; the Console line
// other than "BAR 42.3 · 128.00 BPM · REC ARMED"; empty-half words other
// than 2.2's; a player state read into the wrong stem's light; a click
// (or a second click) that sends the engine anything but one call per stem.

describe('the track half’s words', () => {
  it('reads elapsed time as m:ss, and h:mm:ss past an hour', () => {
    expect(elapsed(102.9)).toBe('1:42');
    expect(elapsed(0)).toBe('0:00');
    expect(elapsed(3723)).toBe('1:02:03');
    expect(elapsed(-3)).toBe('0:00');
  });

  it('reads "1:42 / 3:58", or only the elapsed time in the narrow strip', () => {
    expect(trackTime(102, 238, false)).toBe('1:42 / 3:58');
    expect(trackTime(102, 238, true)).toBe('1:42');
  });

  it('reads the Console’s session line', () => {
    expect(consoleLine({ bar: 42, beat: 3, bpm: 128, armed: true })).toBe('BAR 42.3 · 128.00 BPM · REC ARMED');
    expect(consoleLine({ bar: 1, beat: 1, bpm: 92, armed: false })).toBe('BAR 1.1 · 92.00 BPM');
  });

  it('keeps 2.2’s empty-half words and 2.3’s pauses', () => {
    expect(EMPTY_TRACK).toEqual({ line1: 'Nothing playing', line2: 'Select anything and press Space.' });
    expect(pausedLine('console')).toBe('Paused for the Console');
    expect(pausedLine('film')).toBe('The planet is paused while this plays.');
    expect(pausedLine(null)).toBeNull();
  });
});

describe('the player’s stems as a mix', () => {
  it('reads each stem into its own light, and a master-only song as no stems', () => {
    const mix = mixOf({
      ...NO_PLAYER,
      stems: [
        { stem: 'vocals', level: 0.7, mute: false, solo: false },
        { stem: 'drums', level: 1, mute: true, solo: false },
        { stem: 'other', level: 1, mute: false, solo: true },
        { stem: 'bass', level: 1, mute: false, solo: false },
      ],
    });
    expect(mix!.vocals.level).toBe(0.7);
    expect(mix!.drums.muted).toBe(true);
    expect(mix!.other.soloed).toBe(true);
    expect(voiceLabel(mix!, 'vocals')).toBe('Vocals, silent under a solo');
    expect(mixOf(NO_PLAYER)).toBeNull();
  });

  it('sends one call per stem for a click, and one for a second click’s revert-and-solo', () => {
    expect(stemCalls([{ type: 'mute', stem: 'vocals', muted: true }])).toEqual([{ stem: 'vocals', mute: true }]);
    expect(
      stemCalls([
        { type: 'mute', stem: 'vocals', muted: false },
        { type: 'solo', stem: 'vocals', soloed: true },
      ]),
    ).toEqual([{ stem: 'vocals', mute: false, solo: true }]);
    expect(stemCalls([{ type: 'level', stem: 'bass', level: 0.55 }])).toEqual([{ stem: 'bass', level: 0.55 }]);
    expect(stemCalls([{ type: 'focus', stem: 'bass' }])).toEqual([]);
  });

  it('lets the mix model show a click at once, before the engine answers', () => {
    const mix = applyToMix(freshMix(1), [{ type: 'mute', stem: 'drums', muted: true }]);
    expect(voiceLabel(mix, 'drums')).toBe('Drums, muted');
  });
});
