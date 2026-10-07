// The six tabs and their acts (docs/SPEC.md 3.3). Each tab has one primary
// act ("+" or N) and exactly one secondary act, which ⇧Return runs (3.16).
// Mail has no "+", because Mail can't write.

import * as copy from './copy';
import { HABIT_LIMIT } from './habits';

export type TabName = 'Today' | 'Tasks' | 'Calendar' | 'Grades' | 'Habits' | 'Mail';

export const TABS: { name: TabName; key: string }[] = [
  { name: 'Today', key: '1' },
  { name: 'Tasks', key: '2' },
  { name: 'Calendar', key: '3' },
  { name: 'Grades', key: '4' },
  { name: 'Habits', key: '5' },
  { name: 'Mail', key: '6' },
];

export interface TabActs {
  /** What "+" adds, and why it is off when it is; null where "+" is hidden. */
  plus: { adds: string; disabled: string | null } | null;
  /** The one secondary act; null while it has nothing to act on. */
  secondary: string | null;
}

export function tabActs(tab: TabName, state: { inboxCount: number; habitCount: number }): TabActs {
  const secondary = tab === 'Tasks' && state.inboxCount === 0 ? null : copy.tabs.secondary[tab];
  if (tab === 'Mail') return { plus: null, secondary };
  const full = tab === 'Habits' && state.habitCount >= HABIT_LIMIT;
  return { plus: { adds: copy.tabs.plus[tab], disabled: full ? copy.habits.limit : null }, secondary };
}
