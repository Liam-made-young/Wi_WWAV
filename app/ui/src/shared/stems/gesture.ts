// The one stem gesture, for every surface that shows stems: the Now
// strip's lights, the planet player's moons, the Console's Planet and a
// record in your hands (docs/SPEC.md 2.2, 4.6, 5.8, 7.6).
//
//   click (under 250 ms, under 8 pt)  mute, at once
//   a second click within 250 ms      revert that mute and solo instead
//   hold 400 ms                       bloom the four FX moons (or put them away)
//   drag along the arm (8 pt slop)    level = clamp(level0 + (d · armDir) / span, 0, 1)
//   drag across a bloomed arm         pan
//   ⌘-click, or Return                enter the stem (5.8; only the Console's Planet has an inside)
//   keys on a focused stem            arrows ±5%, M mute, S solo, Tab next in file order
//
// A click acts on release, never waiting to see whether a second follows:
// that is what makes the mute immediate. A second click counts per stem,
// as iOS keeps one last tap per moon. Soloing a stem un-mutes it, so "solo
// instead" is always heard; muting a soloed stem leaves the solo, so the
// click silences it as it would anywhere (mix.ts).
//
// The machine keeps no timers. Every input carries its time, and a hold is
// judged on the clock of whatever input comes next, so a surface sends a
// tick at 400 ms and the rules are exactly testable. One pointer at a time:
// the first one down is captured, and every other pointer's inputs are
// ignored until it lifts. `command` is ⌘ on macOS and Ctrl elsewhere; with
// it held a key is the app's shortcut, never a stem's. The lights have no
// arms and no room to bloom, so on them (stage null) only clicks and keys
// do anything.

import { ARMS, FX_DIRECTIONS, PAN_AXES, fxReach, fxWet, type Point, type StageGeometry } from './geometry';
import type { Effect, FxOwner, Mix, StageAction } from './mix';
import { nextStem, type Stem } from './stems';

export const TAP_MS = 250;
export const HOLD_MS = 400;
export const SLOP_PT = 8;
const KEY_STEP = 0.05;
// A click on a dry FX moon wakes it this wet, so the click is audible.
const WAKE_WET = 0.45;
const DRY = 0.02;

export type Target =
  | { kind: 'stem'; stem: Stem }
  | { kind: 'planet' }
  | { kind: 'fx'; owner: FxOwner; effect: Effect }
  | { kind: 'sky' };

export interface GestureState {
  phase: 'idle' | 'pressed' | 'dragging' | 'held';
  pointer: number | null; // the captured pointer, while one is down
  command: boolean; // whether ⌘ was held when it went down
  target: Target | null;
  down: Point;
  downTime: number;
  level0: number;
  pan0: number;
  // Which way this drag moves its stem; null until it leaves the slop,
  // when a bloomed stem may still go either way.
  axis: 'level' | 'pan' | null;
  // Each stem's last click, for the second click that solos instead.
  lastClicks: Partial<Record<Stem, { time: number; mutedBefore: boolean }>>;
  bloom: FxOwner | null;
}

// `pointer` is the PointerEvent's pointerId. A cancel without one (Esc,
// lost focus) ends whatever press there is.
export type GestureInput =
  | { type: 'down'; pointer: number; command: boolean; target: Target; at: Point; time: number }
  | { type: 'move'; pointer: number; at: Point; time: number }
  | { type: 'up'; pointer: number; time: number }
  | { type: 'tick'; time: number }
  | { type: 'cancel'; pointer?: number }
  | { type: 'key'; key: string; shift: boolean; command: boolean; stem: Stem };

export interface Surface {
  mix: Mix;
  stage: StageGeometry | null; // null for the Now strip's lights
}

interface Step {
  state: GestureState;
  actions: StageAction[];
}

export function idleGesture(): GestureState {
  return {
    phase: 'idle',
    pointer: null,
    command: false,
    target: null,
    down: { x: 0, y: 0 },
    downTime: 0,
    level0: 0,
    pan0: 0,
    axis: null,
    lastClicks: {},
    bloom: null,
  };
}

export function stemGesture(state: GestureState, input: GestureInput, surface: Surface): Step {
  switch (input.type) {
    case 'key':
      return { state, actions: keyActions(input, surface.mix) };
    case 'cancel':
      if (input.pointer !== undefined && input.pointer !== state.pointer) return { state, actions: [] };
      return { state: released(state), actions: [] };
    case 'down':
      return press(state, input, surface);
  }
  if (input.type !== 'tick' && state.phase !== 'idle' && input.pointer !== state.pointer) {
    return { state, actions: [] };
  }
  const held = holdIfDue(state, input.time, surface);
  if (input.type === 'move') {
    const moved = move(held.state, input.at, surface);
    return { state: moved.state, actions: [...held.actions, ...moved.actions] };
  }
  if (input.type === 'up') {
    const clicked =
      held.state.phase === 'pressed' && input.time - held.state.downTime < TAP_MS
        ? click(held.state, input.time, surface.mix)
        : { state: held.state, actions: [] };
    return { state: released(clicked.state), actions: [...held.actions, ...clicked.actions] };
  }
  return held;
}

function released(state: GestureState): GestureState {
  return { ...state, phase: 'idle', pointer: null, command: false, target: null };
}

function press(state: GestureState, input: Extract<GestureInput, { type: 'down' }>, surface: Surface): Step {
  // One pointer at a time: a second finger is ignored until the first lifts.
  if (state.phase !== 'idle') return { state, actions: [] };
  const { target } = input;
  if (target.kind === 'sky') {
    // A click on empty sky puts any FX moons away.
    return state.bloom === null
      ? { state, actions: [] }
      : { state: { ...state, bloom: null }, actions: [{ type: 'bloom', owner: null }] };
  }
  const stem = target.kind === 'stem' ? target.stem : null;
  return {
    state: {
      ...state,
      phase: 'pressed',
      pointer: input.pointer,
      command: input.command,
      target,
      down: input.at,
      downTime: input.time,
      level0: stem ? surface.mix[stem].level : 0,
      pan0: stem ? surface.mix[stem].pan : 0,
      // Pan is behind the hold: only a stem whose own FX moons are out
      // may go either way.
      axis: stem && surface.stage && state.bloom === stem ? null : 'level',
    },
    actions: [],
  };
}

function holdIfDue(state: GestureState, time: number, surface: Surface): Step {
  if (state.phase !== 'pressed' || time - state.downTime < HOLD_MS) return { state, actions: [] };
  const heldState = { ...state, phase: 'held' as const };
  const owner: FxOwner | null =
    state.target?.kind === 'stem' ? state.target.stem : state.target?.kind === 'planet' ? 'master' : null;
  if (!surface.stage || owner === null) return { state: heldState, actions: [] };
  const bloom = state.bloom === owner ? null : owner;
  return { state: { ...heldState, bloom }, actions: [{ type: 'bloom', owner: bloom }] };
}

function move(state: GestureState, at: Point, surface: Surface): Step {
  if (state.phase !== 'pressed' && state.phase !== 'dragging') return { state, actions: [] };
  const d = { x: at.x - state.down.x, y: at.y - state.down.y };
  let next = state;
  if (state.phase === 'pressed') {
    if (d.x * d.x + d.y * d.y < SLOP_PT * SLOP_PT) return { state, actions: [] };
    next = { ...state, phase: 'dragging' };
    if (state.target?.kind === 'stem' && state.axis === null) {
      // Decided once, so a diagonal wobble can't slide the level while
      // you place the stem between the ears, or the other way round.
      const along = Math.abs(dot(d, ARMS[state.target.stem]));
      const across = Math.abs(dot(d, PAN_AXES[state.target.stem]));
      next = { ...next, axis: across > along ? 'pan' : 'level' };
    }
  }
  const { stage, mix } = surface;
  const target = next.target;
  if (!stage || !target) return { state: next, actions: [] };
  if (target.kind === 'stem') {
    if (next.axis === 'pan') {
      const pan = Math.min(1, Math.max(-1, next.pan0 + dot(d, PAN_AXES[target.stem]) / stage.span));
      return { state: next, actions: [{ type: 'pan', stem: target.stem, pan }] };
    }
    const level = Math.min(1, Math.max(0, next.level0 + dot(d, ARMS[target.stem]) / stage.span));
    return { state: next, actions: [{ type: 'level', stem: target.stem, level }] };
  }
  if (target.kind === 'fx') {
    // An FX moon's wetness is its distance out along its own diagonal.
    const reach = fxReach(stage, target.owner, mix);
    const along = dot({ x: at.x - reach.parent.x, y: at.y - reach.parent.y }, FX_DIRECTIONS[target.effect]);
    const wet = Math.min(1, Math.max(0, (along - reach.rMin) / (reach.rMax - reach.rMin)));
    return { state: next, actions: [{ type: 'fx', owner: target.owner, effect: target.effect, wet }] };
  }
  return { state: next, actions: [] };
}

function click(state: GestureState, time: number, mix: Mix): Step {
  const target = state.target!;
  // A ⌘-click is never a plain click: it enters a stem, and does nothing
  // anywhere else.
  if (state.command) return { state, actions: target.kind === 'stem' ? [{ type: 'enter', stem: target.stem }] : [] };
  if (target.kind === 'fx') {
    const wet = fxWet(mix, target.owner, target.effect);
    const action: StageAction =
      wet <= DRY
        ? { type: 'fx', owner: target.owner, effect: target.effect, wet: WAKE_WET }
        : { type: 'fxCharacter', owner: target.owner, effect: target.effect };
    return { state, actions: [action] };
  }
  const own: FxOwner | null = target.kind === 'stem' ? target.stem : target.kind === 'planet' ? 'master' : null;
  if (state.bloom !== null && state.bloom !== own) {
    // With another body's FX moons out, a click only puts them away, so one
    // click never does two things.
    return { state: { ...state, bloom: null }, actions: [{ type: 'bloom', owner: null }] };
  }
  if (target.kind === 'planet') return { state, actions: [{ type: 'playPause' }] };
  if (target.kind !== 'stem') return { state, actions: [] };

  const stem = target.stem;
  const last = state.lastClicks[stem];
  if (last && time - last.time < TAP_MS) {
    const soloed = !mix[stem].soloed;
    return {
      state: { ...state, lastClicks: { ...state.lastClicks, [stem]: undefined } },
      actions: [
        { type: 'mute', stem, muted: soloed ? false : last.mutedBefore },
        { type: 'solo', stem, soloed },
      ],
    };
  }
  const muted = mix[stem].muted;
  return {
    state: { ...state, lastClicks: { ...state.lastClicks, [stem]: { time, mutedBefore: muted } } },
    actions: [{ type: 'mute', stem, muted: !muted }],
  };
}

function keyActions(input: Extract<GestureInput, { type: 'key' }>, mix: Mix): StageAction[] {
  const { stem } = input;
  if (input.command) return [];
  const nudge = (by: number): StageAction[] => [
    { type: 'level', stem, level: Math.min(1, Math.max(0, mix[stem].level + by)) },
  ];
  switch (input.key) {
    case 'ArrowUp':
    case 'ArrowRight':
      return nudge(KEY_STEP);
    case 'ArrowDown':
    case 'ArrowLeft':
      return nudge(-KEY_STEP);
    case 'm':
    case 'M':
      return [{ type: 'mute', stem, muted: !mix[stem].muted }];
    case 's':
    case 'S':
      return mix[stem].soloed || !mix[stem].muted
        ? [{ type: 'solo', stem, soloed: !mix[stem].soloed }]
        : [
            { type: 'mute', stem, muted: false },
            { type: 'solo', stem, soloed: true },
          ];
    case 'Tab':
      return [{ type: 'focus', stem: nextStem(stem, input.shift) }];
    case 'Enter':
      return [{ type: 'enter', stem }];
    default:
      return [];
  }
}

function dot(a: Point, b: Point): number {
  return a.x * b.x + a.y * b.y;
}
