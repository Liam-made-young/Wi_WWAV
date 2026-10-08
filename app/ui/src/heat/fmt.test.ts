import { describe, expect, it } from 'vitest';
import { courseLabel, homeLabel } from './fmt';

// A course's name is written once. What a fail looks like: "JPN 101 JPN 101"
// on a card or in a list, a name dropped when it does say more than the code,
// or a task's home naming its space while it has a course.

describe('a course’s label', () => {
  it('is the code alone when the name is empty or only says the code again', () => {
    expect(courseLabel({ code: 'JPN 101', name: 'JPN 101' })).toBe('JPN 101');
    expect(courseLabel({ code: 'JPN 101', name: '' })).toBe('JPN 101');
    expect(courseLabel({ code: 'JPN 101' })).toBe('JPN 101');
    expect(courseLabel({ code: 'JPN 101', name: null })).toBe('JPN 101');
    // Case and spaces don't make a second name.
    expect(courseLabel({ code: 'JPN 101', name: 'jpn101' })).toBe('JPN 101');
    expect(courseLabel({ code: 'JPN 101', name: '  Jpn  101 ' })).toBe('JPN 101');
  });

  it('is the code, a dot and the name when the name says more', () => {
    expect(courseLabel({ code: 'ELE 209', name: 'Intro to Computer Systems Lab' })).toBe(
      'ELE 209 · Intro to Computer Systems Lab',
    );
    expect(courseLabel({ code: ' ELE 209 ', name: ' Intro to Computer Systems Lab ' })).toBe(
      'ELE 209 · Intro to Computer Systems Lab',
    );
    // A course with a name and no code is still named.
    expect(courseLabel({ code: '', name: 'Writing' })).toBe('Writing');
  });
});

describe('a task’s home', () => {
  const derived = (label: string | null) =>
    ({ home: label === null ? null : { kind: 'course', id: 'c', label } }) as Parameters<typeof homeLabel>[0];

  it('is the course, else the project, else the space', () => {
    expect(homeLabel(derived('ELE 209 · Lab'), { project: 'EP', space: 'Classes' })).toBe('ELE 209 · Lab');
    expect(homeLabel(derived(null), { course: 'JPN 101', project: 'EP', space: 'Classes' })).toBe('JPN 101');
    expect(homeLabel(undefined, { project: 'EP', space: 'WWAV' })).toBe('EP');
    expect(homeLabel(undefined, { space: 'Personal' })).toBe('Personal');
    expect(homeLabel(undefined, {})).toBe('');
  });
});
