import { describe, expect, it } from 'vitest';
import { DAY_MS } from './heat';
import type { Capture, Course, Milestone, Project, Space } from './records';
import {
  type SidebarData,
  defaultSpaces,
  groupCounts,
  groupHeading,
  groupName,
  inSpace,
  libraryLists,
  spaceCounts,
  tasksEmptyLine,
} from './spaces';
import { NY, ids, ny, task } from './testkit';

const now = ny('2026-10-06 08:40');

function data(over: Partial<SidebarData> = {}): SidebarData {
  return { tasks: [], occurrences: [], captures: [], projects: [], milestones: [], courses: [], ...over };
}

describe('the three default spaces (3.1, 3.4)', () => {
  const [classes, wwav, personal] = defaultSpaces(ids('space'));

  it('are Classes, WWAV and Personal, with today’s groups and types plus Other', () => {
    expect([classes, wwav, personal].map((s) => [s.name, s.groupKind, s.groupLabel, s.types])).toEqual([
      ['Classes', 'course', 'Course', ['Homework', 'Quiz', 'Listening', 'Reading', 'Lab', 'Project', 'Exam prep', 'Other']],
      ['WWAV', 'milestone', 'Milestone', ['Hardware', 'Software', 'Design', 'Music', 'Business', 'Content', 'Other']],
      ['Personal', 'free', 'Area', ['Errand', 'Admin', 'Money', 'Health', 'Home', 'Social', 'Other']],
    ]);
    expect([classes.id, wwav.id, personal.id]).toEqual(['space-1', 'space-2', 'space-3']);
  });

  it('give WWAV the persona rewritten to cover the app', () => {
    expect(wwav.persona).toBe(
      'a solo founder building WWAV: the PRANA handheld (Teensy 4.1, C++ firmware, PCBs), the Wi_WWAV desktop app, and an album.',
    );
    expect(classes.persona).not.toBe('');
    expect(personal.persona).not.toBe('');
  });

  it('have three different hues', () => {
    expect(new Set([classes.hue, wwav.hue, personal.hue]).size).toBe(3);
  });
});

describe('filters', () => {
  it('keep a space’s tasks, or every task under All', () => {
    const a = task({ spaceId: 'classes' });
    const b = task({ spaceId: 'wwav' });
    expect([a, b].filter(inSpace('wwav'))).toEqual([b]);
    expect([a, b].filter(inSpace(null))).toEqual([a, b]);
  });

  it('count each space’s open tasks, and all of them', () => {
    const spaces: Space[] = defaultSpaces(ids('s'));
    const tasks = [
      task({ spaceId: 's-1' }),
      task({ spaceId: 's-1' }),
      task({ spaceId: 's-1', done: true, doneAt: now }),
      task({ spaceId: 's-2' }),
      // A series that has ended isn't open.
      task({ spaceId: 's-3', rrule: 'FREQ=DAILY;COUNT=1', due: ny('2026-10-01 09:00') }),
    ];
    expect(spaceCounts(spaces, data({ tasks }), now, NY)).toEqual({
      all: 3,
      spaces: [
        { space: spaces[0], open: 2 },
        { space: spaces[1], open: 1 },
        { space: spaces[2], open: 0 },
      ],
    });
  });
});

describe('the Tasks sidebar (3.6)', () => {
  it('lists Inbox, All open, Hot, Due this week, Scheduled, Someday and Done', () => {
    const someday: Project = { id: 'p1', spaceId: 'wwav', title: 'Vinyl run', status: 'someday' };
    const active: Project = { id: 'p2', spaceId: 'wwav', title: 'Album', status: 'active' };
    const hot = task({ title: 'hot', difficulty: 3, due: now + DAY_MS });
    const week = task({ title: 'week', difficulty: 1, due: now + 6 * DAY_MS });
    const late = task({ title: 'late', due: now - DAY_MS });
    const scheduled = task({ title: 'scheduled', scheduledDate: '2026-10-08' });
    const parked = task({ title: 'parked', projectId: someday.id });
    const live = task({ title: 'live', projectId: active.id });
    const done = task({ title: 'done', done: true, doneAt: now });
    const captures: Capture[] = [{ id: 'c1', text: 'fix the snare at 1:32' }, { id: 'c2', text: 'old', triagedAt: now }];
    const lists = libraryLists(
      data({ tasks: [hot, week, late, scheduled, parked, live, done], captures, projects: [someday, active] }),
      now,
      NY,
    );
    const titles = (k: keyof typeof lists) => (lists[k] as { title: string }[]).map((t) => t.title);
    expect(lists.inbox).toEqual([captures[0]]);
    expect(titles('allOpen')).toEqual(['late', 'hot', 'week', 'scheduled', 'parked', 'live']);
    expect(titles('hot')).toEqual(['late', 'hot']);
    expect(titles('dueThisWeek')).toEqual(['late', 'hot', 'week']);
    expect(titles('scheduled')).toEqual(['scheduled']);
    expect(titles('someday')).toEqual(['parked']);
    expect(titles('done')).toEqual(['done']);
  });

  it('applies the space filter to tasks but not to the inbox', () => {
    const captures: Capture[] = [{ id: 'c1', text: 'a thought' }];
    const lists = libraryLists(data({ tasks: [task({ spaceId: 'classes' })], captures }), now, NY, 'wwav');
    expect(lists.allOpen).toEqual([]);
    expect(lists.inbox).toEqual(captures);
  });
});

describe('groups', () => {
  const [classes, wwav, personal] = defaultSpaces(ids('s'));
  const jpn: Course = { id: 'jpn201', termId: 't', code: 'JPN 201', name: 'Japanese', categories: [], notes: '' };
  const mth: Course = { id: 'mth142', termId: 't', code: 'MTH 142', name: 'Calculus', categories: [], notes: '' };
  const beads: Milestone[] = [
    { id: 'm2', spaceId: wwav.id, title: 'Firmware 1.0', date: '2026-11-01', done: false, order: 2 },
    { id: 'm1', spaceId: wwav.id, title: 'Enclosure v2', date: '2026-10-20', done: false, order: 1 },
  ];
  const tasks = [
    task({ spaceId: classes.id, courseId: mth.id }),
    task({ spaceId: classes.id, courseId: jpn.id }),
    task({ spaceId: classes.id, courseId: jpn.id }),
    task({ spaceId: classes.id, courseId: jpn.id, done: true, doneAt: now }),
    task({ spaceId: wwav.id, milestoneId: 'm2' }),
    task({ spaceId: personal.id, group: 'Car' }),
    task({ spaceId: personal.id, group: 'Apartment' }),
    task({ spaceId: personal.id, group: 'Car', done: true, doneAt: now }),
  ];
  const d = data({ tasks, milestones: beads, courses: [mth, jpn] });

  it('lists Courses, Milestones or Areas under each space with open counts', () => {
    expect(groupHeading(classes)).toBe('Courses');
    expect(groupHeading(wwav)).toBe('Milestones');
    expect(groupHeading(personal)).toBe('Areas');
    expect(groupCounts(classes, d, now, NY)).toEqual([
      { id: 'jpn201', name: 'JPN 201', open: 2 },
      { id: 'mth142', name: 'MTH 142', open: 1 },
    ]);
    expect(groupCounts(wwav, d, now, NY)).toEqual([
      { id: 'm1', name: 'Enclosure v2', open: 0 },
      { id: 'm2', name: 'Firmware 1.0', open: 1 },
    ]);
    expect(groupCounts(personal, d, now, NY)).toEqual([
      { id: 'Apartment', name: 'Apartment', open: 1 },
      { id: 'Car', name: 'Car', open: 1 },
    ]);
  });

  it('names a task’s group as the LCD shows it: the course code, the milestone, or the area', () => {
    expect(groupName(tasks[1], d)).toBe('JPN 201');
    expect(groupName(tasks[4], d)).toBe('Firmware 1.0');
    expect(groupName(tasks[5], d)).toBe('Car');
    expect(groupName(task(), d)).toBeNull();
  });
});

describe('the empty Tasks list', () => {
  it('keeps Heat’s line', () => {
    const [, wwav] = defaultSpaces(ids());
    expect(tasksEmptyLine(wwav)).toBe('Add your first WWAV task and Heat will rank it.');
    expect(tasksEmptyLine(null)).toBe('Add your first task and Heat will rank it.');
  });
});
