// The player expanded (2.3): outside Space, a sheet dropped from the title
// bar. The player is a state, not a screen, so this is the strip's track
// seen close: its planet, its title, its time, play and pause, and its four
// stems at full size. Esc collapses it. The planet player with moons is
// Space's (4.6); this sheet holds what the strip can already do.

import type { CSSProperties } from 'react';
import { STEMS } from '../shared/stems/stems';
import { glowTone, keyColor, parseKey } from '../styles/keyColor';
import { cursorAt } from './cursor';
import { StemLight, useStemPointer } from './StemLights';
import { elapsed, pausedLine } from './strip';
import type { Player } from './usePlayer';

export function PlayerSheet({ player, shown }: { player: Player; shown: boolean }) {
  const { state, mix } = player;
  const handlers = useStemPointer(player);
  const key = parseKey(state.key);
  const planet = { '--key': keyColor(key), '--glow': glowTone(key) } as CSSProperties;
  // The sheet's time is read when it renders; the strip carries the moving cursor.
  const at = cursorAt(player.anchor, performance.now(), state.duration);
  return (
    <div className="sheet player-sheet register-desk" role="dialog" aria-label="Player" hidden={!shown}>
      <span className="planet planet-large" style={planet} aria-hidden="true" />
      <div className="player-words">
        <p className="player-title">{state.title}</p>
        <p className="player-time" data-text="secondary">
          {pausedLine(state.pausedFor) ?? `${elapsed(at)} / ${elapsed(state.duration)}`}
          {state.key ? ` · ${state.key}` : ''}
          {state.bpm ? ` · ${state.bpm} BPM` : ''}
        </p>
      </div>
      <button type="button" className="gel" onClick={() => void player.playPause()}>
        {state.playing ? 'Pause' : 'Play'}
      </button>
      <div className="player-stems" role="group" aria-label="Stems">
        {mix ? (
          STEMS.map((stem) => <StemLight key={stem} stem={stem} mix={mix} size={16} handlers={handlers(stem)} />)
        ) : (
          <p data-text="secondary">This song plays as its master only.</p>
        )}
      </div>
    </div>
  );
}
