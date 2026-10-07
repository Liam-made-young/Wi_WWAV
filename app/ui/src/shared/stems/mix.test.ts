import { describe, expect, test } from 'vitest';
import { STEMS, STEM_COLOURS, nextStem } from './stems';
import { applyToMix, audible, freshMix, stemShape, stemState, voiceLabel, type Mix } from './mix';

function mix(patch: Partial<Record<keyof Mix, Partial<Mix['vocals']>>> = {}): Mix {
  const m = freshMix();
  for (const [stem, p] of Object.entries(patch)) Object.assign(m[stem as keyof Mix], p);
  return m;
}

describe('the four stems', () => {
  test("are always in PRANA's order and colours", () => {
    expect(STEMS).toEqual(['vocals', 'drums', 'other', 'bass']);
    expect(STEM_COLOURS).toEqual({ vocals: '#D23C2A', drums: '#F0B90B', other: '#2E9A55', bass: '#1F4E9E' });
  });

  test('Tab walks them in file order and leaves at the ends', () => {
    expect(nextStem('vocals', false)).toBe('drums');
    expect(nextStem('other', false)).toBe('bass');
    expect(nextStem('bass', false)).toBeNull();
    expect(nextStem('drums', true)).toBe('vocals');
    expect(nextStem('vocals', true)).toBeNull();
  });
});

describe('who is audible', () => {
  test('without a solo, every unmuted stem', () => {
    expect(audible(mix({ drums: { muted: true } }))).toEqual({ vocals: true, drums: false, other: true, bass: true });
  });

  test('with a solo, only the soloed stems, muted or not', () => {
    const m = mix({ bass: { soloed: true, muted: true }, vocals: { soloed: true } });
    expect(audible(m)).toEqual({ vocals: true, drums: false, other: false, bass: true });
  });
});

describe('state is shape, not colour (8.3)', () => {
  test('names each state, a solo before a mute before a silence', () => {
    const m = mix({ vocals: { soloed: true, muted: true }, drums: { muted: true } });
    expect(STEMS.map((s) => stemState(m, s))).toEqual(['soloed', 'muted', 'silent', 'silent']);
    expect(stemState(freshMix(), 'bass')).toBe('audible');
  });

  test('draws each state as 8.3 lists it', () => {
    const m = mix({ vocals: { soloed: true }, drums: { muted: true } });
    expect(stemShape(freshMix(), 'vocals', false, 'night')).toEqual({
      fill: 1,
      stemRing: 0,
      inkRing: 'solid',
      outerRing: null,
      notch: false,
    });
    expect(stemShape(m, 'drums', false, 'desk')).toMatchObject({ fill: 0, stemRing: 2, inkRing: 'solid' });
    expect(stemShape(m, 'vocals', false, 'night')).toMatchObject({ fill: 1, outerRing: '#2946FF' });
    expect(stemShape(m, 'vocals', false, 'desk')).toMatchObject({ fill: 1, outerRing: 'ink' });
    expect(stemShape(m, 'other', false, 'night')).toMatchObject({ fill: 0.45, inkRing: 'dashed', outerRing: null });
    expect(stemShape(m, 'bass', true, 'night')).toMatchObject({ notch: true });
  });
});

describe('what VoiceOver reads', () => {
  test('one sentence per light or moon', () => {
    const m = mix({ vocals: { level: 0.7 }, drums: { muted: true } });
    expect(voiceLabel(m, 'vocals')).toBe('Vocals, 70 percent, audible');
    expect(voiceLabel(m, 'drums')).toBe('Drums, muted');
    const solo = mix({ bass: { soloed: true } });
    expect(voiceLabel(solo, 'bass')).toBe('Bass, soloed');
    expect(voiceLabel(solo, 'other')).toBe('Other, silent under a solo');
  });
});

describe('actions', () => {
  test('set the mix, and the master FX moon sets all four stems', () => {
    const m = applyToMix(freshMix(), [
      { type: 'level', stem: 'vocals', level: 0.3 },
      { type: 'mute', stem: 'drums', muted: true },
      { type: 'solo', stem: 'bass', soloed: true },
      { type: 'pan', stem: 'other', pan: -0.5 },
      { type: 'fx', owner: 'master', effect: 'delay', wet: 0.6 },
      { type: 'fx', owner: 'vocals', effect: 'reverb', wet: 0.2 },
      { type: 'playPause' },
    ]);
    expect(m.vocals.level).toBe(0.3);
    expect(m.drums.muted).toBe(true);
    expect(m.bass.soloed).toBe(true);
    expect(m.other.pan).toBe(-0.5);
    expect(STEMS.map((s) => m[s].fx.delay)).toEqual([0.6, 0.6, 0.6, 0.6]);
    expect(m.vocals.fx.reverb).toBe(0.2);
    expect(freshMix().vocals.fx.reverb).toBe(0);
  });
});
