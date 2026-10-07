import { describe, expect, it } from 'vitest';
import { TABS, tabActs } from './tabs';

describe('the six tabs and their acts (3.3)', () => {
  it('are Today, Tasks, Calendar, Grades, Habits and Mail, on 1 to 6', () => {
    expect(TABS.map((t) => [t.name, t.key])).toEqual([
      ['Today', '1'],
      ['Tasks', '2'],
      ['Calendar', '3'],
      ['Grades', '4'],
      ['Habits', '5'],
      ['Mail', '6'],
    ]);
  });

  it('give each tab one "+" act and exactly one secondary act', () => {
    const quiet = { inboxCount: 1, habitCount: 0 };
    expect(TABS.map((t) => [t.name, tabActs(t.name, quiet)])).toEqual([
      ['Today', { plus: { adds: 'A task straight into the plan', disabled: null }, secondary: 'Plan my day' }],
      ['Tasks', { plus: { adds: 'A task', disabled: null }, secondary: 'Triage inbox' }],
      ['Calendar', { plus: { adds: 'A task due 11:59 PM on the selected day', disabled: null }, secondary: 'Today' }],
      ['Grades', { plus: { adds: 'A grade', disabled: null }, secondary: 'Add course' }],
      ['Habits', { plus: { adds: 'A habit', disabled: null }, secondary: 'Show the year' }],
      ['Mail', { plus: null, secondary: 'Sync now' }],
    ]);
  });

  it('offer Triage inbox only while the inbox has items', () => {
    expect(tabActs('Tasks', { inboxCount: 0, habitCount: 0 }).secondary).toBeNull();
  });

  it('turn Habits’ "+" off at 6, saying why', () => {
    expect(tabActs('Habits', { inboxCount: 0, habitCount: 6 }).plus).toEqual({ adds: 'A habit', disabled: 'Habit limit reached' });
  });
});
