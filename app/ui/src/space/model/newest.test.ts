import { describe, expect, test } from 'vitest';
import { PAGE, cardOf, filterNewest, pageOf, sinceYouLastLooked, type NewestItem, type SinceEvent } from './newest';

function item(n: number, extra: Partial<NewestItem> = {}): NewestItem {
  return {
    id: `w${String(n).padStart(3, '0')}`,
    createdAt: new Date(Date.UTC(2026, 9, 1, 0, n)).toISOString(),
    title: `Work ${n}`,
    artist: 'LMY',
    galaxyId: 1,
    medium: 'song',
    key: 'A minor',
    bpm: 120,
    ...extra,
  };
}

describe('Newest', () => {
  test('is newest first, whatever order the items arrive in', () => {
    const items = [item(3), item(1), item(7), item(2)];
    expect(filterNewest(items, {}, new Set()).map((i) => i.id)).toEqual(['w007', 'w003', 'w002', 'w001']);
  });

  test('breaks a tie in time by id, newest id first', () => {
    const at = item(5).createdAt;
    const items = [item(1, { createdAt: at }), item(2, { createdAt: at })];
    expect(filterNewest(items, {}, new Set()).map((i) => i.id)).toEqual(['w002', 'w001']);
  });

  test('filters by medium, galaxies I have added, key and BPM', () => {
    const items = [
      item(1),
      item(2, { medium: 'film', key: null, bpm: null }),
      item(3, { galaxyId: 2 }),
      item(4, { key: 'E major' }),
      item(5, { bpm: 90 }),
      item(6, { medium: 'writing', key: null, bpm: null, words: 900 }),
    ];
    const ids = (f: Parameters<typeof filterNewest>[1]) => filterNewest(items, f, new Set([2])).map((i) => i.id);
    expect(ids({ media: ['film', 'writing'] })).toEqual(['w006', 'w002']);
    expect(ids({ addedOnly: true })).toEqual(['w003']);
    expect(ids({ key: 'E major' })).toEqual(['w004']);
    expect(ids({ bpm: { min: 85, max: 100 } })).toEqual(['w005']);
  });

  test('shows 30, then "Show older", then ends "That\'s everything."', () => {
    const items = filterNewest(
      Array.from({ length: 65 }, (_, n) => item(n)),
      {},
      new Set(),
    );
    expect(PAGE).toBe(30);
    expect(pageOf(items, 1)).toMatchObject({ end: 'Show older' });
    expect(pageOf(items, 1).cards).toHaveLength(30);
    expect(pageOf(items, 2).cards).toHaveLength(60);
    expect(pageOf(items, 3)).toMatchObject({ end: "That's everything." });
    expect(pageOf(items, 3).cards).toHaveLength(65);
    expect(pageOf([], 1)).toEqual({ cards: [], end: "That's everything." });
  });

  test('a card says what a thing is and waits: one fact line, one verb, no count', () => {
    const card = cardOf(item(1, { title: 'glass hours', artist: 'Ana', bpm: 128 }));
    expect(card).toEqual({ id: 'w001', title: 'glass hours', artist: 'Ana', facts: 'Song · A minor · 128 BPM', verb: 'Play' });
    expect(cardOf(item(2, { medium: 'fashion', key: null, bpm: null, photos: 14 }))).toMatchObject({
      facts: 'Fashion · 14 photos',
      verb: 'Look',
    });
  });
});

describe('Since you last looked', () => {
  const lastLooked = new Date(2026, 9, 4, 16, 30);
  const now = new Date(2026, 9, 7, 22, 30);
  const at = (day: number, hour: number) => new Date(2026, 9, day, hour, 0).toISOString();
  const events: SinceEvent[] = [
    { kind: 'fork', at: at(6, 12), who: 'Ana', work: 'World Ending', fork: 'glass hours' },
    { kind: 'work', at: at(3, 9), who: 'Ana', title: 'Too old' },
    {
      kind: 'link',
      at: at(1, 9),
      who: 'LMY',
      linkId: 4,
      link: { from: { type: 'planet', id: 1 }, to: { type: 'planet', id: 2 }, kind: 'influence' },
    },
    { kind: 'letter', at: at(7, 8), who: 'LMY', greeting: 'Dear Wi-WWAV,', opening: 'Hardware is hard.' },
    { kind: 'work', at: at(6, 20), who: 'Ana', title: 'Low Tide' },
  ];

  test('lists what happened since, newest first, and ends with the date', () => {
    const list = sinceYouLastLooked(events, lastLooked, now)!;
    expect(list.items.map((i) => i.line)).toEqual([
      'A letter from LMY: "Dear Wi-WWAV, Hardware is hard."',
      'New from Ana: Low Tide.',
      'Ana forked World Ending: glass hours.',
      'LMY says their planet is an influence of your planet.',
    ]);
    expect(list.end).toBe("That's everything since Oct 4.");
  });

  test('keeps a link waiting on you, however old, with Agree and Refuse', () => {
    const list = sinceYouLastLooked(events, lastLooked, now)!;
    const waiting = list.items.find((i) => i.linkId === 4)!;
    expect(waiting.actions).toEqual(['Agree', 'Refuse']);
  });

  test('ends with a time when you last looked today, and a year when it was long ago', () => {
    const today = sinceYouLastLooked(events, new Date(2026, 9, 7, 16, 30), now)!;
    expect(today.end).toBe("That's everything since 4:30 PM.");
    const long = sinceYouLastLooked(events, new Date(2025, 9, 4, 9, 5), now)!;
    expect(long.end).toBe("That's everything since Oct 4, 2025.");
  });

  test('opens only when there is anything new', () => {
    expect(sinceYouLastLooked(events.filter((e) => e.kind === 'work' && e.title === 'Too old'), lastLooked, now)).toBeNull();
  });
});
