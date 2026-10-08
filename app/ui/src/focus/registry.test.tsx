// The view registry: order, keys, and what every summon path reads.

import { afterEach, describe, expect, it } from 'vitest';
import { registerBuiltins } from './builtin';
import { allViews, registerView, viewById, viewByShortcut, viewForInterrupt } from './registry';

const Nothing = () => null;
const undo: (() => void)[] = [];
const add = (def: Parameters<typeof registerView>[0]) => undo.push(registerView(def));

afterEach(() => {
  while (undo.length) undo.pop()!();
});

describe('the view registry', () => {
  it('holds the six first tabs on keys 1 to 6, in order', () => {
    registerBuiltins();
    expect(allViews().slice(0, 6).map((v) => [v.shortcut, v.id, v.title])).toEqual([
      [1, 'today', 'Today'],
      [2, 'tasks', 'Tasks'],
      [3, 'calendar', 'Calendar'],
      [4, 'grades', 'Grades'],
      [5, 'habits', 'Habits'],
      [6, 'mail', 'Mail'],
    ]);
    expect(viewById('tasks')).toMatchObject({ plus: 'New task', secondary: 'Triage inbox', sidebar: true });
  });

  it('gives a new view the next free key, and takes it out again', () => {
    registerBuiltins();
    add({ id: 'database', title: 'Database', component: Nothing });
    add({ id: 'wiki', title: 'Wiki', component: Nothing });
    add({ id: 'notes', title: 'Notes', component: Nothing, shortcut: 9, interrupts: ['notes'] });
    expect(viewByShortcut(7)?.id).toBe('database');
    expect(viewByShortcut(8)?.id).toBe('wiki');
    expect(viewByShortcut(9)?.id).toBe('notes');
    expect(viewForInterrupt('notes')?.id).toBe('notes');
    expect(viewForInterrupt('gradeWaiting')?.id).toBe('grades');
    undo.pop()!();
    expect(viewById('notes')).toBeUndefined();
    expect(viewByShortcut(9)).toBeUndefined();
  });

  it('never gives one key to two views, and leaves a tenth view without one', () => {
    registerBuiltins();
    add({ id: 'a', title: 'A', component: Nothing, shortcut: 1 });
    add({ id: 'b', title: 'B', component: Nothing });
    add({ id: 'c', title: 'C', component: Nothing });
    add({ id: 'd', title: 'D', component: Nothing });
    const keys = allViews().map((v) => v.shortcut);
    expect(viewByShortcut(1)?.id).toBe('today');
    expect(new Set(keys.filter((k) => k !== null)).size).toBe(9);
    expect(viewById('d')?.shortcut).toBeNull();
  });
});
