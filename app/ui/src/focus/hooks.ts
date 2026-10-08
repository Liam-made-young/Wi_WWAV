// What the shell needs of the Focus layout as state: the layout and the
// entropy on the page's root, where the tokens read them, and the readout's
// words (docs/FOCUS.md).

import { useEffect, useMemo } from 'react';
import { dueText, effectiveDue } from '../heat/fmt';
import { useHeat, useNow } from '../heat/store';
import { focusOf } from './FocusScreen';
import { countLaunch, type Layout, useLayout } from './layout';
import { type ReadoutText, readoutText } from './words';

/**
 * Puts the layout and entropy on the root: `data-layout` turns the prism
 * tokens on for every view and sheet, and `--entropy` (0 to 1) is the one
 * variable colour intensity reads. Both windows wear them.
 */
export function useFocusRoot(): Layout {
  const layout = useLayout();
  const { snap } = useHeat();
  const score = focusOf(snap)?.entropy.score ?? 0;
  useEffect(() => {
    document.documentElement.dataset.layout = layout;
  }, [layout]);
  useEffect(() => {
    document.documentElement.style.setProperty('--entropy', String(score));
  }, [score]);
  useEffect(() => countLaunch(), []);
  return layout;
}

/** The readout's words, worked out again when Learn changes and when the minute turns. */
export function useReadout(playing: string | null): ReadoutText {
  const { snap, idx, tz } = useHeat();
  const at = useNow(30_000);
  const minute = Math.floor(at / 60_000);
  return useMemo(() => {
    const focus = focusOf(snap);
    const task = focus?.now.kind === 'task' ? idx.task.get(focus.now.taskId) : undefined;
    const due = task && snap ? effectiveDue(task, snap.derived.tasks[task.id], tz) : null;
    return readoutText({
      now: focus?.now ?? null,
      task: task ? { title: task.title, due: due === null ? null : dueText(due, at, tz) } : null,
      next: focus?.next ?? null,
      // Commitments say what is next in their own words, once they are in the snapshot.
      nextLine: (snap as { commitments?: { next?: { line?: string | null } } } | null)?.commitments?.next?.line ?? null,
      playing,
      at,
      tz,
    });
    // `at` moves every tick; the words only move with the minute.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snap, idx, tz, playing, minute]);
}
