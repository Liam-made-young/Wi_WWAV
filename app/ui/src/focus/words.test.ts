// The readout's words, and how they are fitted to a row of cells.

import { describe, expect, it } from 'vitest';
import { epochOf } from '../shared/time/zone';
import { bandOf } from './band';
import { glyph, lit } from './dots';
import { hintShows } from './layout';
import { CONFIG, type Now } from './model';
import { layoutCells, readoutText } from './words';

const NY = 'America/New_York';
const AT = epochOf({ year: 2026, month: 10, day: 7, hour: 9, minute: 0 }, NY);
const at = (day: number, hour: number) => epochOf({ year: 2026, month: 10, day, hour, minute: 0 }, NY);
const clear: Now = { kind: 'clear', taskId: null, why: null, fix: null };
const onTask: Now = { kind: 'task', taskId: 't', why: 'heat', fix: null };

describe('the readout', () => {
  it('says ALL CLEAR and the next commitment when every loop is closed', () => {
    const text = readoutText({ now: clear, task: null, next: { id: 'e', title: 'JPN 101', start: at(7, 10) }, playing: null, at: AT, tz: NY });
    expect(text.left).toBe('ALL CLEAR · NEXT JPN 101 10:00 AM');
    expect(text.label).toBe('All clear. Next JPN 101 10:00 AM.');
  });

  it('says the Now task and when it is due, then the next commitment and what is playing', () => {
    const text = readoutText({
      now: onTask,
      task: { title: '第2課 Grammar quiz', due: 'Saturday 6:00 PM' },
      next: { id: 'e', title: 'Studio time', start: at(8, 14) },
      playing: 'World Ending',
      at: AT,
      tz: NY,
    });
    expect(text.left).toBe('第2課 GRAMMAR QUIZ · SAT 6:00 PM');
    expect(text.right).toEqual(['NEXT STUDIO TIME TOMORROW 2:00 PM', '♪ WORLD ENDING']);
    expect(text.label).toBe('Now: 第2課 Grammar quiz, Saturday 6:00 PM. Next Studio time Tomorrow 2:00 PM. Playing World Ending.');
  });

  it('keeps the main line and drops from the right when the row is short', () => {
    const text = { left: 'GRAMMAR QUIZ 4 · TOMORROW 11:59 PM', right: ['NEXT JPN 101 10:00 AM', '♪ SONG'], label: '' };
    expect(layoutCells(text, 80)).toBe(`${text.left}${' '.repeat(16)}NEXT JPN 101 10:00 AM   ♪ SONG`);
    expect(layoutCells(text, 60).trimEnd()).toBe(`${text.left}     NEXT JPN 101 10:00 AM`);
    expect(layoutCells(text, 40).trimEnd()).toBe(text.left);
    expect(layoutCells(text, 20)).toBe('GRAMMAR QUIZ 4 · TO…');
    expect(layoutCells(text, 60)).toHaveLength(60);
  });

  it('draws characters as the 5 by 7 dots a character LCD holds', () => {
    const rows = (ch: string) =>
      Array.from({ length: 7 }, (_, row) => Array.from({ length: 5 }, (_, col) => (lit(glyph(ch)!, col, row) ? '#' : '.')).join(''));
    expect(rows('A')).toEqual(['.###.', '#...#', '#...#', '#...#', '#####', '#...#', '#...#']);
    expect(rows('1')).toEqual(['..#..', '.##..', '..#..', '..#..', '..#..', '..#..', '.###.']);
    expect(glyph('a')).toEqual(glyph('A'));
    expect(glyph('課')).toBeNull();
    expect(glyph('♪')).not.toBeNull();
  });
});

describe('a space’s band', () => {
  it('sits on the anchor its hue is nearest, and between two when it falls between', () => {
    expect(bandOf(211)).toBe('var(--prism-band-blue)');
    expect(bandOf(145)).toBe('var(--prism-band-green)');
    expect(bandOf(356)).toBe('var(--prism-band-red)');
    expect(bandOf(76)).toBe('color-mix(in oklab, var(--prism-band-orange) 50%, var(--prism-band-green))');
    expect(bandOf(undefined)).toBe('var(--prism-ink3)');
  });
});

describe('the hint', () => {
  it('shows for the first days or launches, whichever ends first, then not again', () => {
    localStorage.clear();
    expect(hintShows(0)).toBe(true);
    localStorage.setItem('wi.focusHint', JSON.stringify({ first: 0, launches: 3 }));
    expect(hintShows(6 * 86_400_000)).toBe(true);
    expect(hintShows(CONFIG.hintDays * 86_400_000)).toBe(false);
    localStorage.setItem('wi.focusHint', JSON.stringify({ first: 0, launches: CONFIG.hintLaunches + 1 }));
    expect(hintShows(1)).toBe(false);
  });
});

describe('the commitments’ own line', () => {
  it('stands where NEXT does, when the snapshot carries one', () => {
    const text = readoutText({
      now: clear,
      task: null,
      next: { id: 'e', title: 'Other', start: at(7, 10) },
      nextLine: 'NEXT JPN 101 10:00 · LEAVE 9:35',
      playing: null,
      at: AT,
      tz: NY,
    });
    expect(text.left).toBe('ALL CLEAR · NEXT JPN 101 10:00 · LEAVE 9:35');
  });
});
