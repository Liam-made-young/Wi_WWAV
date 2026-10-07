// The four stems, in PRANA's order and colours, "the same everywhere"
// (docs/SPEC.md 2.2, 8.3). Order is always vocals, drums, other, bass: the
// file's order, the lights' order and Tab's order.

import tokens from '../../../../../design/tokens.json' with { type: 'json' };

export type Stem = 'vocals' | 'drums' | 'other' | 'bass';

export const STEMS: readonly Stem[] = ['vocals', 'drums', 'other', 'bass'];

export const STEM_LABELS: Record<Stem, string> = { vocals: 'Vocals', drums: 'Drums', other: 'Other', bass: 'Bass' };

// The colours live in the token file, like every other (docs/SPEC.md 8.11).
export const STEM_COLOURS: Record<Stem, string> = {
  vocals: tokens.stem.vocals,
  drums: tokens.stem.drums,
  other: tokens.stem.other,
  bass: tokens.stem.bass,
};

// Tab (or ⇧Tab) from one stem: the next in file order, or null past the
// end so focus moves on out of the group.
export function nextStem(stem: Stem, back: boolean): Stem | null {
  const i = STEMS.indexOf(stem) + (back ? -1 : 1);
  return i >= 0 && i < STEMS.length ? STEMS[i] : null;
}
