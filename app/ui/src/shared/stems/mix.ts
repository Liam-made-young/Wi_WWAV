// A song's four-stem mix, what each stem looks and sounds like in it, and
// the actions every stem surface sends: the Now strip's lights, the planet
// player's moons, the Console's Planet and a record in your hands.

import { STEMS, STEM_LABELS, type Stem } from './stems';

export type Effect = 'reverb' | 'delay' | 'distortion' | 'tremolo';

export const EFFECTS: readonly Effect[] = ['reverb', 'delay', 'distortion', 'tremolo'];

export interface StemMix {
  level: number; // 0…1, the moon's distance from the planet
  muted: boolean;
  soloed: boolean;
  pan: number; // −1…1
  fx: Record<Effect, number>; // dry/wet, 0…1
}

export type Mix = Record<Stem, StemMix>;

// FX moons belong to one stem, or to the planet, where they reach all four.
export type FxOwner = Stem | 'master';

export type MixAction =
  | { type: 'mute'; stem: Stem; muted: boolean }
  | { type: 'solo'; stem: Stem; soloed: boolean }
  | { type: 'level'; stem: Stem; level: number }
  | { type: 'pan'; stem: Stem; pan: number }
  | { type: 'fx'; owner: FxOwner; effect: Effect; wet: number };

// What a surface does besides changing the mix.
export type StageAction =
  | MixAction
  | { type: 'bloom'; owner: FxOwner | null } // FX moons out round a body, or away
  | { type: 'fxCharacter'; owner: FxOwner; effect: Effect } // the effect's next character
  | { type: 'playPause' }
  | { type: 'focus'; stem: Stem | null }
  // ⌘-click or Return (5.8): the stem becomes the planet and its tracks its
  // moons. Only the Console's Planet has an inside; elsewhere it is ignored.
  | { type: 'enter'; stem: Stem };

export function freshMix(level = 0.8): Mix {
  const stem = (): StemMix => ({
    level,
    muted: false,
    soloed: false,
    pan: 0,
    fx: { reverb: 0, delay: 0, distortion: 0, tremolo: 0 },
  });
  return { vocals: stem(), drums: stem(), other: stem(), bass: stem() };
}

export function applyToMix(mix: Mix, actions: StageAction[]): Mix {
  const next = structuredClone(mix);
  for (const a of actions) {
    switch (a.type) {
      case 'mute':
        next[a.stem].muted = a.muted;
        break;
      case 'solo':
        next[a.stem].soloed = a.soloed;
        break;
      case 'level':
        next[a.stem].level = a.level;
        break;
      case 'pan':
        next[a.stem].pan = a.pan;
        break;
      case 'fx':
        // The planet's FX moons are the same knob on all four stems.
        for (const s of a.owner === 'master' ? STEMS : [a.owner]) next[s].fx[a.effect] = a.wet;
        break;
    }
  }
  return next;
}

// A mute always silences, and under a solo only soloed stems sound. So a
// click on a soloed stem mutes it at once, as anywhere else, and the solo
// still holds the others: on iOS, where a solo is every other stem muted,
// the same click leaves the same silence. Soloing a stem un-mutes it
// (gesture.ts), so "solo instead" is always heard.
export function audible(mix: Mix): Record<Stem, boolean> {
  const anySolo = STEMS.some((s) => mix[s].soloed);
  const out = {} as Record<Stem, boolean>;
  for (const s of STEMS) out[s] = !mix[s].muted && (!anySolo || mix[s].soloed);
  return out;
}

export type StemState = 'audible' | 'muted' | 'soloed' | 'soloedMuted' | 'silent';

// A stem's own settings show before the silence a solo elsewhere imposes,
// and a solo and a mute together show as both.
export function stemState(mix: Mix, stem: Stem): StemState {
  const { soloed, muted } = mix[stem];
  if (soloed) return muted ? 'soloedMuted' : 'soloed';
  if (muted) return 'muted';
  return audible(mix)[stem] ? 'audible' : 'silent';
}

// State is shape, not colour (8.3). Every mark wears a 1 pt ring in its
// register's ink, which carries the contrast; the fill is the stem's own hex.
// The centre says muted or not and the outer ring says soloed or not, so a
// soloed stem that is also muted wears both: the open centre and the ring.
export interface StemShape {
  fill: number; // opacity of the stem-colour fill: 1, 0.45, or 0 for an open centre
  stemRing: number; // width of a stem-colour ring, pt (2 when muted)
  inkRing: 'solid' | 'dashed';
  outerRing: string | null; // a 2 pt ring outside: royal blue in Space, ink elsewhere
  notch: boolean; // selected: a 2 pt notch beneath; PRANA blinks, the app doesn't
}

export type Register = 'desk' | 'night' | 'interior';

export function stemShape(mix: Mix, stem: Stem, selected: boolean, register: Register): StemShape {
  const shape: StemShape = { fill: 1, stemRing: 0, inkRing: 'solid', outerRing: null, notch: selected };
  switch (stemState(mix, stem)) {
    case 'muted':
      return { ...shape, fill: 0, stemRing: 2 };
    case 'soloed':
      return { ...shape, outerRing: soloRing(register) };
    case 'soloedMuted':
      return { ...shape, fill: 0, stemRing: 2, outerRing: soloRing(register) };
    case 'silent':
      return { ...shape, fill: 0.45, inkRing: 'dashed' };
    default:
      return shape;
  }
}

function soloRing(register: Register): string {
  return register === 'night' ? '#2946FF' : 'ink';
}

// "Vocals, 70 percent, audible", "Drums, muted", "Bass, soloed".
export function voiceLabel(mix: Mix, stem: Stem): string {
  const name = STEM_LABELS[stem];
  switch (stemState(mix, stem)) {
    case 'audible':
      return `${name}, ${Math.round(mix[stem].level * 100)} percent, audible`;
    case 'muted':
      return `${name}, muted`;
    case 'soloed':
      return `${name}, soloed`;
    case 'soloedMuted':
      return `${name}, soloed and muted`;
    case 'silent':
      return `${name}, silent under a solo`;
  }
}
