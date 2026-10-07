// The one listening player as the strip sees it (2.3): its state from the
// core's `player` events, and the cursor's anchor from the engine's clock.

import { useEffect, useRef, useState } from 'react';
import { call } from '../bridge';
import { applyToMix, type Mix, type StageAction } from '../shared/stems/mix';
import { STEMS } from '../shared/stems/stems';
import { type Anchor, anchorFromClock, anchorFromState, type Clock } from './cursor';
import { useCoreEvent } from './hooks';
import { mixOf, NO_PLAYER, type PlayerState, stemCalls } from './strip';

export interface Player {
  state: PlayerState;
  anchor: Anchor;
  mix: Mix | null;
  /** Sends a stem gesture's actions: shown at once, then as the engine answers. */
  stems(actions: StageAction[]): Promise<void>;
  /** Space: play or pause what is loaded. */
  playPause(): Promise<void>;
  load(clip: string): Promise<void>;
}

export function usePlayer(): Player {
  const [state, setState] = useState<PlayerState>(NO_PLAYER);
  const [anchor, setAnchor] = useState<Anchor>({ seconds: 0, at: 0, speed: 0 });
  const sampleRate = useRef(0);
  const current = useRef(state);
  current.current = state;

  const take = (s: PlayerState) => {
    current.current = s;
    setState(s);
    setAnchor(anchorFromState(s, performance.now()));
  };

  useEffect(() => {
    call<{ sampleRate?: number }>('engine.status').then(
      (s) => (sampleRate.current = s.sampleRate ?? 0),
      () => {},
    );
  }, []);
  useCoreEvent<{ sampleRate?: number }>('engine', (s) => {
    if (s.sampleRate) sampleRate.current = s.sampleRate;
  });
  useCoreEvent<PlayerState>('player', take);
  // The clock is the engine's, so it only speaks for the player while the
  // player has the engine: never while the Console or a film has paused it.
  // A clock read while playing can arrive just after the pause it predates;
  // once paused, only a stopped clock may move the cursor (to where it stopped).
  useCoreEvent<Clock>('clock', (c) => {
    const s = current.current;
    if (s.clip === null || s.pausedFor !== null || (c.state !== 'stopped' && !s.playing)) return;
    setAnchor(anchorFromClock(c, sampleRate.current, performance.now()));
  });

  const answer = (r: { state: PlayerState }) => take(r.state);

  return {
    state,
    anchor,
    mix: mixOf(state),
    async stems(actions) {
      const mix = mixOf(current.current);
      if (!mix) return;
      const shown = applyToMix(mix, actions);
      setState((s) => ({
        ...s,
        stems: STEMS.map((stem) => ({
          stem,
          level: shown[stem].level,
          mute: shown[stem].muted,
          solo: shown[stem].soloed,
        })),
      }));
      for (const c of stemCalls(actions)) answer(await call('player.stem', c));
    },
    async playPause() {
      answer(await call(current.current.playing ? 'player.pause' : 'player.play'));
    },
    async load(clip) {
      answer(await call('player.load', { clip }));
      answer(await call('player.play'));
    },
  };
}
