// Learn's own tabs, registered (focus/registry.ts). Whatever is in TAB_IDS
// with a row in TAB_TABLE and a component in HEAT_TABS becomes a view, on
// keys 1, 2, 3… in that order, so a tab added to those tables the old way is
// in every summon path too.

import type { ReactNode } from 'react';
import { TAB_IDS, TAB_TABLE } from '../heat/frame';
import { HEAT_TABS } from '../heat/tabs';
import { registerView } from './registry';

const line = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, strokeLinecap: 'round', strokeLinejoin: 'round' } as const;

/** One glyph per tool, by id: a tab that names none here gets the plain square. */
const ICONS: Record<string, ReactNode> = {
  today: (
    <>
      <circle cx="10" cy="10" r="6.5" {...line} />
      <path d="M10 6v4l2.5 2" {...line} />
    </>
  ),
  tasks: <path d="M4 5.5h1M8 5.5h8M4 10h1M8 10h8M4 14.5h1M8 14.5h8" {...line} />,
  calendar: (
    <>
      <rect x="3.5" y="4.5" width="13" height="12" rx="1" {...line} />
      <path d="M3.5 8.5h13M7 3v3M13 3v3" {...line} />
    </>
  ),
  grades: <path d="M4 16.5 10 3.5l6 13M6.2 12.5h7.6" {...line} />,
  habits: (
    <>
      <circle cx="6" cy="10" r="2.75" fill="currentColor" />
      <circle cx="14" cy="10" r="2.75" {...line} />
    </>
  ),
  mail: (
    <>
      <rect x="2.5" y="4.5" width="15" height="11" rx="1" {...line} />
      <path d="M3 5.5l7 5.5 7-5.5" {...line} />
    </>
  ),
  database: <path d="M3.5 4.5h13v11h-13zM3.5 8.5h13M3.5 12h13M8 4.5v11" {...line} />,
  wiki: <path d="M3 5l3 10 4-8 4 8 3-10" {...line} />,
  notes: <path d="M5 3.5h7l3 3v10H5zM7.5 9h5M7.5 12.5h5" {...line} />,
};

const PLAIN = <rect x="4.5" y="4.5" width="11" height="11" rx="1" {...line} />;

/** The tabs that use the left sidebar: the spaces filter, and a tab's own sections under it. */
const SIDEBAR = new Set(['today', 'tasks', 'calendar', 'mail', 'database', 'wiki', 'notes']);

/** The interrupt sources each tab is the place for (focus/model.ts). */
const INTERRUPTS: Record<string, string[]> = {
  today: ['atRisk', 'focusEnded'],
  tasks: ['dueChanged'],
  calendar: ['leaveFor', 'commitments'],
  grades: ['gradeWaiting'],
  mail: ['mailUrgent'],
  notes: ['notes'],
};

let done = false;

/** Registers every tab in the tables, once. */
export function registerBuiltins() {
  if (done) return;
  done = true;
  TAB_IDS.forEach((id, i) => {
    const row = TAB_TABLE[id];
    const component = HEAT_TABS[id];
    if (!row || !component) return;
    registerView({
      id,
      title: row.name,
      shortcut: i + 1,
      icon: ICONS[id] ?? PLAIN,
      component,
      sidebar: SIDEBAR.has(id),
      plus: row.plus,
      secondary: row.secondary,
      interrupts: INTERRUPTS[id] ?? [],
    });
  });
}
