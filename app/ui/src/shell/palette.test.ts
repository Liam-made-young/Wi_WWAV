import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fuzzy, latestOnly, parseQuery, rank } from './palette';

// 2.7's ⌘K. What a fail looks like: a filter read as a search word; a
// near miss ranked over an exact start; an older, slower answer replacing
// a newer one (the PKM's palette clears its timer but not its request, so
// a slow reply for "gr" can land after the reply for "grammar"); a server
// call before 200 ms, or one per keystroke.

describe('the palette’s query', () => {
  it('reads tag:, key:, bpm:, is:hot, is:remix and @name apart from the words', () => {
    expect(parseQuery('night tag:Live-Drums key:A-minor bpm:120-130 is:hot is:remix @lmy drive')).toEqual({
      words: 'night drive',
      tags: ['live-drums'],
      key: 'A minor',
      bpm: [120, 130],
      hot: true,
      remix: true,
      person: 'lmy',
    });
    expect(parseQuery('bpm:128').bpm).toEqual([128, 128]);
    expect(parseQuery('bpm:fast').words).toBe('bpm:fast');
    expect(parseQuery('').words).toBe('');
  });
});

describe('fuzzy matching', () => {
  it('matches letters in order, and nothing out of order', () => {
    expect(fuzzy('gq4', 'Grammar quiz 4')).not.toBeNull();
    expect(fuzzy('qg', 'Grammar quiz 4')).toBeNull();
    expect(fuzzy('', 'anything')).toBe(0);
  });

  it('ranks a start over a word start over a scatter', () => {
    const titles = ['Program the drums', 'Drum bus', 'Hydrant um'];
    expect(rank('drum', titles, (t) => t)).toEqual(['Drum bus', 'Program the drums', 'Hydrant um']);
  });
});

describe('only the newest answer is shown', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('waits 200 ms, asks once for a burst of keys, and drops a slower older answer', async () => {
    const asked: string[] = [];
    const answers: Record<string, (v: string) => void> = {};
    const shown: string[] = [];
    const search = latestOnly(
      (q: string) => {
        asked.push(q);
        return new Promise<string>((ok) => (answers[q] = ok));
      },
      (q, r) => shown.push(`${q}=${r}`),
      200,
    );
    search.ask('g');
    search.ask('gr');
    await vi.advanceTimersByTimeAsync(199);
    expect(asked).toEqual([]);
    await vi.advanceTimersByTimeAsync(1);
    expect(asked).toEqual(['gr']);

    search.ask('grammar');
    await vi.advanceTimersByTimeAsync(200);
    expect(asked).toEqual(['gr', 'grammar']);
    answers.grammar('new');
    await vi.advanceTimersByTimeAsync(0);
    answers.gr('old');
    await vi.advanceTimersByTimeAsync(0);
    expect(shown).toEqual(['grammar=new']);
  });

  it('drops an answer that arrives after the palette closes', async () => {
    const shown: string[] = [];
    let answer: (v: string) => void = () => {};
    const search = latestOnly(
      () => new Promise<string>((ok) => (answer = ok)),
      (_q, r) => shown.push(r),
      200,
    );
    search.ask('x');
    await vi.advanceTimersByTimeAsync(200);
    search.cancel();
    answer('late');
    await vi.advanceTimersByTimeAsync(0);
    expect(shown).toEqual([]);
  });
});
