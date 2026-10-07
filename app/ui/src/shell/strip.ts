// The Now strip's track half (docs/SPEC.md 2.2), and the listening player's
// state as the core sends it (docs/COMMANDS.md player.*). The task half is
// Heat's LCD: heat/model/lcd.ts.

import { freshMix, type Mix, type StageAction } from '../shared/stems/mix';
import type { Stem } from '../shared/stems/stems';

export interface PlayerStem {
  stem: Stem;
  level: number;
  mute: boolean;
  solo: boolean;
}

export interface PlayerState {
  clip: string | null;
  title: string | null;
  key: string | null;
  bpm: number | null;
  playing: boolean;
  position: number;
  duration: number;
  /** The four stems in PRANA's order, or none for a song that plays as its master only. */
  stems: PlayerStem[];
  pausedFor: null | 'console' | 'film';
}

export const NO_PLAYER: PlayerState = {
  clip: null,
  title: null,
  key: null,
  bpm: null,
  playing: false,
  position: 0,
  duration: 0,
  stems: [],
  pausedFor: null,
};

export const EMPTY_TRACK = { line1: 'Nothing playing', line2: 'Select anything and press Space.' };

/** 2.3: what the strip says when something else paused the listening player. */
export function pausedLine(pausedFor: PlayerState['pausedFor']): string | null {
  if (pausedFor === 'console') return 'Paused for the Console';
  if (pausedFor === 'film') return 'The planet is paused while this plays.';
  return null;
}

/** "1:42", or "1:02:03" past an hour; time gone, so it rounds down. */
export function elapsed(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const ss = String(s % 60).padStart(2, '0');
  if (s < 3600) return `${Math.floor(s / 60)}:${ss}`;
  return `${Math.floor(s / 3600)}:${String(Math.floor(s / 60) % 60).padStart(2, '0')}:${ss}`;
}

/**
 * "1:42 / 3:58". The narrow strip leaves 43.5 pt beside the lights, room
 * for the elapsed time only; the length is still in the half's label.
 */
export function trackTime(position: number, duration: number, narrow: boolean): string {
  return narrow ? elapsed(position) : `${elapsed(position)} / ${elapsed(duration)}`;
}

export interface ConsoleTransport {
  bar: number;
  beat: number;
  bpm: number;
  armed: boolean;
}

/** In the Console the track half shows the session: "BAR 42.3 · 128.00 BPM · REC ARMED". */
export function consoleLine(t: ConsoleTransport): string {
  const line = `BAR ${t.bar}.${t.beat} · ${t.bpm.toFixed(2)} BPM`;
  return t.armed ? `${line} · REC ARMED` : line;
}

/** The player's stems as the stem model's mix, or null for a song with no stems. */
export function mixOf(state: PlayerState): Mix | null {
  if (state.stems.length === 0) return null;
  const mix = freshMix(1);
  for (const s of state.stems) {
    mix[s.stem] = { ...mix[s.stem], level: s.level, muted: s.mute, soloed: s.solo };
  }
  return mix;
}

export type StemCall = { stem: Stem; mute?: boolean; solo?: boolean; level?: number };

/**
 * The gesture's actions as player.stem calls: one per stem, so a second
 * click's revert-and-solo reaches the engine as one change.
 */
export function stemCalls(actions: StageAction[]): StemCall[] {
  const calls = new Map<Stem, StemCall>();
  const call = (stem: Stem) => calls.get(stem) ?? calls.set(stem, { stem }).get(stem)!;
  for (const a of actions) {
    if (a.type === 'mute') call(a.stem).mute = a.muted;
    else if (a.type === 'solo') call(a.stem).solo = a.soloed;
    else if (a.type === 'level') call(a.stem).level = a.level;
  }
  return [...calls.values()];
}
