import { describe, expect, it } from 'vitest';
import { call, on } from '../../bridge';
import { epochOf } from '../../shared/time/zone';
import { installFakeCore } from './install';

// `npm run dev` with VITE_FAKE_CORE=1 (docs/HEAT.md; main.tsx): the whole UI
// runs on the fake core with a seeded sample. What a fail looks like: the
// shell's calls at start not answered, Heat's commands not reaching the fake,
// its events not coming back, or the sample not holding the three spaces of
// 3.4 with tasks, blocks and a course.

describe('the fake core behind the bridge', () => {
  const now = epochOf({ year: 2026, month: 10, day: 7, hour: 10, minute: 0 }, 'America/New_York');
  const fake = installFakeCore({ now, zone: 'America/New_York' });

  it('answers what the shell asks at start', async () => {
    expect(await call('app.hello')).toMatchObject({ signedIn: false });
    expect(await call('account.status')).toEqual({ signedIn: false });
    expect(await call('app.settings.get')).toMatchObject({ appearance: 'system', textSize: 13 });
    expect(await call('app.settings.set', { patch: { textSize: 15 } })).toMatchObject({ textSize: 15 });
    expect(await call('history.get', { room: 'space' })).toEqual({ undo: null, redo: null, cant: null });
    await expect(call('library.list')).rejects.toMatchObject({ code: 'not_in_fake_core' });
  });

  it('holds the three spaces, a course, tasks, blocks and an inbox', async () => {
    const snap = await call<{ records: Record<string, unknown[]>; date: string; zone: string }>('heat.snapshot', {
      date: '2026-10-07',
    });
    expect(snap.records.space.map((s) => (s as { name: string }).name)).toEqual(['Classes', 'WWAV', 'Personal']);
    expect(snap.records.course.length).toBeGreaterThan(0);
    expect(snap.records.task.length).toBeGreaterThan(8);
    expect(snap.records.timeBlock.length).toBeGreaterThan(1);
    expect(snap.records.capture.length).toBeGreaterThan(0);
    expect(snap.zone).toBe('America/New_York');
  });

  it('sends heat and history events when something changes, and refuses as the core does', async () => {
    const events: [string, unknown][] = [];
    const offHeat = on('heat', (p) => events.push(['heat', p]));
    const offHistory = on('history', (p) => events.push(['history', p]));
    const r = await call<{ inbox: string }>('heat.capture.add', { text: 'one more' });
    expect(r.inbox).toBe('4 in inbox · captured ✓');
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(events.map((e) => e[0])).toEqual(['heat', 'history']);
    expect(events[1][1]).toEqual({ room: 'heat', undo: 'Undo capture', redo: null, cant: null });
    await expect(call('heat.done', { taskId: 'nope', done: true })).rejects.toMatchObject({
      message: 'No task has that id.',
    });
    await expect(call('history.redo', { room: 'heat' })).rejects.toMatchObject({ code: 'nothing_to_redo' });
    offHeat();
    offHistory();
  });

  it('runs a clock from where it was told to start, and can be moved on', () => {
    const t = fake.now;
    expect(t).toBeGreaterThanOrEqual(now);
    expect(t).toBeLessThan(now + 5000);
    fake.now = now + 3_600_000;
    expect(fake.now).toBeGreaterThanOrEqual(now + 3_600_000);
    expect((window as unknown as { __wiFake: unknown }).__wiFake).toBeTruthy();
  });
});
