import { describe, expect, it } from 'vitest';
import { heatClient } from '../client';
import { createFake } from './all';

// The fake core answers the shapes docs/HEAT.md gives, so the views can be
// built and tested before the Rust commands exist.
describe('the fake core', () => {
  it('puts, patches, deletes and undoes, with the labels HEAT.md gives', async () => {
    const { transport } = createFake({}, { now: Date.UTC(2026, 9, 7, 14), zone: 'America/New_York' });
    const heat = heatClient(transport);
    const kinds: string[][] = [];
    heat.onChange((k) => kinds.push(k));

    const { record, undo } = await heat.put('space', { name: 'Classes', hue: 210, groupKind: 'course', groupLabel: 'Course', types: ['Homework'], persona: '' });
    expect(undo).toBe('Undo add space');
    expect(record.id).toBeTruthy();
    expect((await heat.patch('space', record.id, { name: 'School' })).undo).toBe('Undo edit space');
    expect((await heat.snapshot('2026-10-07')).records.space[0].name).toBe('School');
    expect(kinds).toEqual([['space'], ['space']]);

    await expect(heat.patch('space', record.id, { public: true } as never)).rejects.toThrow('The Public switch has its own command.');
    expect((await heat.delete('space', record.id)).undo).toBe('Undo delete space');
    expect((await heat.snapshot('2026-10-07')).records.space).toEqual([]);
  });

  it('keeps every Public switch off until it is set, and refuses a seventh habit', async () => {
    const { transport } = createFake();
    const heat = heatClient(transport);
    for (let i = 0; i < 6; i++) {
      const { record } = await heat.put('habit', { title: `h${i}`, log: {}, showCounter: false });
      expect(record.public).toBe(false);
    }
    await expect(heat.put('habit', { title: 'h7', log: {}, showCounter: false })).rejects.toThrow('Habit limit reached');
  });
});
