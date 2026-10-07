import { describe, expect, it } from 'vitest';
import * as copy from './copy';

describe('patterns fill in to the spec’s own sentences', () => {
  it('sync, capture, status and undo', () => {
    expect(copy.sync.synced('3:41 PM', { newTasks: 2, dateChanges: 1, newGrades: 1 })).toBe(
      'Synced 3:41 PM: 2 new tasks, 1 date change, 1 new grade posted',
    );
    expect(copy.sync.synced('8:41 AM', { newTasks: 2, dateChanges: 1, newGrades: 0 })).toBe(
      'Synced 8:41 AM: 2 new tasks, 1 date change',
    );
    expect(copy.sync.synced('8:41 AM', { newTasks: 0, dateChanges: 0, newGrades: 0 })).toBe(
      'Synced 8:41 AM: nothing new',
    );
    expect(copy.capture.footer(4, true)).toBe('4 in inbox · captured ✓');
    expect(copy.capture.footer(3, false)).toBe('3 in inbox');
    expect(copy.status.saved('3:41 PM')).toBe('Saved on this Mac · Synced 3:41 PM');
    expect(copy.status.saved(null)).toBe('Saved on this Mac');
    expect(copy.undo.label(copy.undo.markDone)).toBe('Undo mark done');
    expect(copy.undo.label(copy.undo.moveBlock)).toBe('Undo move block');
  });

  it('Claude, projects and connections', () => {
    expect(copy.claude.estimate('45m')).toBe(
      "Claude's estimate: 45m. It read the title, the notes and your past averages.",
    );
    expect(copy.projects.makeTasks(4)).toBe('Make 4 tasks from To finish?');
    expect(copy.connections.nowMaking('the second verse of More Love')).toBe(
      'Now making: the second verse of More Love',
    );
  });
});

describe('the voice (8.10)', () => {
  // Every plain string in the module, however deep.
  const strings: string[] = [];
  const walk = (x: unknown) => {
    if (typeof x === 'string') strings.push(x);
    else if (Array.isArray(x)) x.forEach(walk);
    else if (x && typeof x === 'object') Object.values(x).forEach(walk);
  };
  walk(copy);

  it('has no exclamation marks, no "AI" and none of the words 8.10 rules out', () => {
    expect(strings.length).toBeGreaterThan(100);
    const never = ['Oops', 'Awesome', 'AI-powered', 'Trending', "Don't break your streak", 'people are looking'];
    for (const s of strings) {
      expect(s, s).not.toMatch(/!/);
      expect(s, s).not.toMatch(/\bAI\b/);
      for (const word of never) expect(s, s).not.toContain(word);
    }
  });

  it('writes buttons in sentence case, as Learn does', () => {
    const buttons = [
      copy.tabs.secondary.Today,
      copy.tabs.secondary.Grades,
      copy.tabs.secondary.Habits,
      copy.tabs.secondary.Mail,
      copy.mail.makeTask,
      copy.mail.openInGmail,
      copy.mail.loadImages,
      copy.tasks.markDone,
      copy.widgets.startFocus,
    ];
    for (const b of buttons) {
      const rest = b
        .split(' ')
        .slice(1)
        .filter((w) => w !== 'Gmail' && w !== 'Claude');
      expect(
        rest.every((w) => w === w.toLowerCase()),
        b,
      ).toBe(true);
    }
  });
});
