import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { mountWith, type Rig, settle } from '../heat/testkit';
import { type TaskHalf, useTaskHalf } from './useTaskHalf';

// docs/SPEC.md 2.2. What a fail looks like: a strip that shows anything but
// the current task, or else the hottest open one; whose second line lacks
// the due phrase, the course, or "focus 18:40 left" while a round runs; an
// empty strip that doesn't read "All clear / Nothing open right now."; ⌘K's
// tasks out of the snapshot's heat order.

let rig: Rig;
afterEach(() => rig?.unmount());

let last: ReturnType<typeof useTaskHalf>;
function Probe() {
  last = useTaskHalf();
  return null;
}
const half = (): TaskHalf => last.half;

describe('the Now strip’s task half', () => {
  it('shows the current task, with its due phrase, its group and the week’s load', async () => {
    rig = await mountWith(<Probe />);
    expect(half()).toMatchObject({ taskId: 't-verse', line1: 'Warm: Mix the second verse', level: 'Warm' });
    expect(half().line2).toMatch(/^Saturday 6:00 PM, EP v1 mixed\. This week: .* across \d+ tasks?$/);
  });

  it('shows the hottest open task when none is current', async () => {
    rig = await mountWith(<Probe />);
    await rig.client.setCurrent(null);
    await settle();
    expect(half().taskId).toBe('t-kanji7');
    expect(half()).toMatchObject({ line1: 'Hot: Kanji worksheet 7', level: 'Hot' });
    expect(half().line2).toMatch(/^Today 4:00 PM, JPN 201\. This week:/);
  });

  it('adds "focus 18:40 left" to the second line while a round runs, and drops it when the round stops', async () => {
    rig = await mountWith(<Probe />);
    await rig.client.setCurrent('t-kanji7');
    await rig.client.focus('start', { taskId: 't-kanji7' });
    rig.fake.now += 6 * 60_000 + 20_000;
    rig.fake.emit([]);
    await settle();
    expect(half().line2).toBe('Today 4:00 PM, JPN 201 · focus 18:40 left');
    await act(async () => void (await rig.client.focus('stop')));
    await settle();
    expect(half().line2).not.toContain('focus');
  });

  it('reads "All clear / Nothing open right now." with nothing to do', async () => {
    rig = await mountWith(<Probe />, { empty: true });
    expect(half()).toMatchObject({ taskId: null, line1: 'All clear', line2: 'Nothing open right now.', level: null });
    expect(last.tasks).toEqual([]);
  });

  it('gives ⌘K every open task, hottest first, with its level', async () => {
    rig = await mountWith(<Probe />);
    expect(last.tasks.slice(0, 5).map((t) => [t.title, t.level])).toEqual([
      ['Kanji worksheet 7', 'Hot'],
      ['Listening practice', 'Hot'],
      ['Grammar quiz 4', 'Hot'],
      ['Problem set 5', 'Hot'],
      ['Mix the second verse', 'Warm'],
    ]);
    expect(last.tasks).toHaveLength(11);
  });
});
