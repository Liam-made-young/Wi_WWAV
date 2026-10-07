// The case's 22 pt status bar (docs/SPEC.md 2.1, 2.7): on the left the
// screen's own count and its verbs, as the Console prints them ("⇧Return
// Cut at playhead · ⌘Z Undo move clip"); on the right the sync and save
// state, in the core's own sentences.

import type { Status } from './hooks';
import { keys } from './platform';

export interface ScreenStatus {
  /** "12 clips". */
  count: string | null;
  /** The screen's one secondary act, on ⇧Return. */
  act: string | null;
}

interface Props {
  screen: ScreenStatus;
  undo: string | null;
  status: Status;
}

export function StatusBar({ screen, undo, status }: Props) {
  const left = [screen.count, screen.act && `${keys('⇧')}Return ${screen.act}`, undo && `${keys('⌘Z')} ${undo}`].filter(
    Boolean,
  );
  const right = [status.upload, status.sync, status.save].filter(Boolean);
  return (
    <footer className="case-status" data-text="secondary">
      <span className="status-left">{left.join(' · ')}</span>
      <span className="status-right" role="status">
        {right.join(' · ')}
      </span>
    </footer>
  );
}
