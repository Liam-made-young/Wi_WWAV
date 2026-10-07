import { describe, expect, it } from 'vitest';
import { type HeatExport, importArtifact, parseHeatExport } from './importArtifact';
import { defaultSpaces } from './spaces';
import { ids, ny } from './testkit';

function dump(): HeatExport {
  return {
    format: 'heat-export',
    version: 1,
    exportedAt: '2026-10-06T12:40:00.000Z',
    timeZone: 'America/New_York',
    workspaces: [
      {
        key: 'classes',
        name: 'Classes',
        groupLabel: 'Course',
        types: ['Homework', 'Quiz', 'Other'],
        persona: 'a student',
      },
      { key: 'wwav', name: 'WWAV', groupLabel: 'Milestone', types: ['Hardware', 'Other'], persona: 'a founder' },
      { key: 'personal', name: 'Personal', groupLabel: 'Area', types: ['Errand', 'Other'], persona: 'a person' },
    ],
    tasks: [
      {
        id: '5h3k2j1abcdefg_20261008T035900Z',
        workspace: 'classes',
        title: 'Grammar quiz 4',
        group: 'JPN 201',
        type: 'Quiz',
        due: '2026-10-08T03:59:00.000Z',
        difficulty: 2,
        estMin: 45,
        actualMin: null,
        notes: '',
        done: false,
        doneAt: null,
        source: 'calendar',
      },
      {
        id: 'em-3f2a9c71',
        workspace: 'classes',
        title: 'Read chapter 3',
        group: 'MTH 142',
        type: 'Reading',
        due: '2026-10-10T03:59:00.000Z',
        difficulty: 3,
        estMin: null,
        actualMin: 75,
        notes: 'From mail: …',
        done: true,
        doneAt: '2026-10-05T20:00:00.000Z',
        source: 'gmail',
      },
      {
        id: 't-1001',
        workspace: 'wwav',
        title: 'Route the PCB',
        group: 'Enclosure v2',
        type: 'Hardware',
        due: null,
        difficulty: 4,
        estMin: 120,
        actualMin: null,
        notes: '',
        done: false,
        doneAt: null,
        source: 'manual',
      },
      {
        id: 't-1002',
        workspace: 'personal',
        title: 'Renew registration',
        group: 'Car',
        type: 'Errand',
        due: null,
        difficulty: 1,
        estMin: null,
        actualMin: null,
        notes: '',
        done: false,
        doneAt: null,
        source: 'manual',
      },
    ],
    milestones: [{ id: 'ms-7', workspace: 'wwav', title: 'Enclosure v2', date: '2026-10-20', done: false, order: 1 }],
    habits: [{ id: 'hb-1', title: 'Practise kanji', log: { '2026-10-05': true, '2026-10-06': true } }],
    term: 'Fall 2026',
    courses: [
      {
        code: 'JPN 201',
        name: 'Intermediate Japanese',
        categories: [
          { name: 'Quizzes', weight: 40, keywords: ['quiz', 'kanji'] },
          { name: 'Exit tickets', weight: 60, keywords: ['exit ticket'] },
        ],
        sticky: 'Office hours Tue 2 PM',
      },
    ],
    grades: [
      {
        id: 'gr-31',
        course: 'JPN 201',
        title: 'Kanji quiz 6',
        category: 'Quizzes',
        score: 18,
        outOf: 20,
        dropped: false,
        pending: false,
      },
      {
        id: 'gp-91be2c',
        course: 'JPN 201',
        title: 'Exit Ticket 12',
        category: null,
        score: null,
        outOf: 10,
        dropped: false,
        pending: true,
        link: 'https://brightspace.uri.edu/d2l/home',
      },
    ],
    processedMailIds: ['18f2a', '18f2b', '18f2c'],
    lastSyncAt: '2026-10-06T12:41:00.000Z',
  };
}

describe('parsing the export', () => {
  it('takes a Heat export and refuses anything else in one line', () => {
    expect(parseHeatExport(JSON.parse(JSON.stringify(dump())))).toEqual(dump());
    expect(parseHeatExport({ tasks: [] })).toEqual({ error: 'This file isn’t a Heat export.' });
    expect(parseHeatExport(null)).toEqual({ error: 'This file isn’t a Heat export.' });
    const stray = dump();
    stray.tasks[0].workspace = 'nowhere';
    expect(parseHeatExport(stray)).toEqual({ error: 'This file isn’t a Heat export.' });
    expect(parseHeatExport({ ...dump(), version: 2 })).toEqual({
      error: 'This export is from a newer Heat. Update Wi_WWAV, then try again.',
    });
  });
});

describe('moving in (3.15)', () => {
  const result = importArtifact(dump(), [], ids('new'));

  it('keeps every id: event ids, em- and gp- hashes, and the processed Gmail ids', () => {
    expect(result.tasks.map((t) => t.id)).toEqual([
      '5h3k2j1abcdefg_20261008T035900Z',
      'em-3f2a9c71',
      't-1001',
      't-1002',
    ]);
    expect(result.grades.map((g) => g.id)).toEqual(['gr-31', 'gp-91be2c']);
    expect(result.milestones.map((m) => m.id)).toEqual(['ms-7']);
    expect(result.habits.map((h) => h.id)).toEqual(['hb-1']);
    expect(result.sync).toEqual({
      processedMailIds: ['18f2a', '18f2b', '18f2c'],
      lastSyncAt: Date.parse('2026-10-06T12:41:00.000Z'),
    });
  });

  it('maps each workspace to a space with its group kind', () => {
    expect(result.spaces.map((s) => [s.name, s.groupKind, s.groupLabel, s.types, s.persona])).toEqual([
      ['Classes', 'course', 'Course', ['Homework', 'Quiz', 'Other'], 'a student'],
      ['WWAV', 'milestone', 'Milestone', ['Hardware', 'Other'], 'a founder'],
      ['Personal', 'free', 'Area', ['Errand', 'Other'], 'a person'],
    ]);
    const [classes, wwav, personal] = result.spaces;
    expect(result.tasks.map((t) => t.spaceId)).toEqual([classes.id, classes.id, wwav.id, personal.id]);
    expect(result.milestones[0].spaceId).toBe(wwav.id);
  });

  it('turns groups into course, milestone and area links', () => {
    const [quiz, reading, pcb, errand] = result.tasks;
    const jpn = result.courses.find((c) => c.code === 'JPN 201')!;
    const mth = result.courses.find((c) => c.code === 'MTH 142')!;
    expect(quiz.courseId).toBe(jpn.id);
    // A course only a task names is made once, so it is defined once.
    expect(reading.courseId).toBe(mth.id);
    expect(mth).toMatchObject({ name: 'MTH 142', categories: [] });
    expect(pcb.milestoneId).toBe('ms-7');
    expect(errand.group).toBe('Car');
  });

  it('carries the task fields over, with typed time as the hand adjustment', () => {
    const [quiz, reading] = result.tasks;
    expect(quiz).toEqual({
      id: '5h3k2j1abcdefg_20261008T035900Z',
      spaceId: result.spaces[0].id,
      title: 'Grammar quiz 4',
      type: 'Quiz',
      courseId: result.courses[0].id,
      due: ny('2026-10-07 23:59'),
      difficulty: 2,
      estMin: 45,
      adjustMin: 0,
      notes: '',
      done: false,
      doneAt: null,
      source: 'calendar',
    });
    expect(reading).toMatchObject({
      adjustMin: 75,
      done: true,
      doneAt: Date.parse('2026-10-05T20:00:00.000Z'),
      source: 'mail',
    });
  });

  it('brings habits with their whole log and the counter off', () => {
    expect(result.habits[0]).toEqual({
      id: 'hb-1',
      title: 'Practise kanji',
      log: { '2026-10-05': true, '2026-10-06': true },
      showCounter: false,
    });
  });

  it('brings the term, the courses with their categories and stickies, and the grades', () => {
    expect(result.terms).toEqual([{ id: result.courses[0].termId, name: 'Fall 2026' }]);
    const jpn = result.courses[0];
    expect(jpn.categories.map((c) => [c.name, c.weight, c.keywords])).toEqual([
      ['Quizzes', 40, ['quiz', 'kanji']],
      ['Exit tickets', 60, ['exit ticket']],
    ]);
    expect(jpn.notes).toBe('Office hours Tue 2 PM');
    expect(result.grades[0]).toMatchObject({
      courseId: jpn.id,
      categoryId: jpn.categories[0].id,
      score: 18,
      outOf: 20,
      source: 'you',
    });
    expect(result.grades[1]).toMatchObject({ categoryId: null, score: null, pending: true, source: 'mail' });
  });

  it('moves into spaces that already exist by name, keeping their ids', () => {
    const existing = defaultSpaces(ids('space'));
    const again = importArtifact(dump(), existing, ids('new'));
    expect(again.spaces).toEqual([]);
    expect(again.tasks.map((t) => t.spaceId)).toEqual(['space-1', 'space-1', 'space-2', 'space-3']);
  });
});
