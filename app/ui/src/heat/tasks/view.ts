// Which list Tasks shows: one of the sidebar's seven, or a group below them.

import type { Id } from '../client';

export type ListName = 'inbox' | 'allOpen' | 'hot' | 'dueThisWeek' | 'scheduled' | 'someday' | 'done';

export type TasksView =
  | { kind: 'list'; list: ListName }
  | { kind: 'group'; by: 'course' | 'milestone' | 'project' | 'area'; id: Id; label: string };

/** The sidebar's lists, in its order (3.6), with the words each is called. */
export const LISTS: { list: ListName; label: string }[] = [
  { list: 'inbox', label: 'Inbox' },
  { list: 'allOpen', label: 'All open' },
  { list: 'hot', label: 'Hot' },
  { list: 'dueThisWeek', label: 'Due this week' },
  { list: 'scheduled', label: 'Scheduled' },
  { list: 'someday', label: 'Someday' },
  { list: 'done', label: 'Done' },
];

export const sameView = (a: TasksView, b: TasksView) =>
  a.kind === b.kind &&
  (a.kind === 'list' ? a.list === (b as typeof a).list : a.by === (b as typeof a).by && a.id === (b as typeof a).id);

export const ALL_OPEN: TasksView = { kind: 'list', list: 'allOpen' };
