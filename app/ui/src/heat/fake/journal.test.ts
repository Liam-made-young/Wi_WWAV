import { describe, expect, it } from 'vitest';
import { heatClient } from '../client';
import { createFake } from './all';

// The fake core's journal as docs/HEAT.md describes the real one: each entry
// names who made the change, and one entry can be undone out of order, but
// not once a later change touched the same records.

function boot() {
  const { fake, transport } = createFake({}, { now: Date.UTC(2026, 9, 7, 14), zone: 'America/New_York' });
  return { fake, heat: heatClient(transport), call: transport.call };
}

const task = (title: string) => ({
  spaceId: 's',
  title,
  type: 'Other',
  due: null,
  difficulty: 3,
  estMin: null,
  adjustMin: 0,
  notes: '',
  done: false,
  doneAt: null,
  source: 'you' as const,
});

describe('the fake core’s journal', () => {
  it('keeps who made each change, with the tool and the reason when it was Claude', async () => {
    const { fake, heat } = boot();
    await heat.put('task', task('Mine'));
    fake.write('add task', ['task'], () => fake.store.task.set('c1', { ...task('Claude’s'), id: 'c1' }), {
      actor: 'claude',
      tool: 'add_task',
      reason: 'The email asks for it.',
    });
    expect(fake.journal.map((e) => [e.actor, e.tool ?? null, e.reason ?? null, e.undone])).toEqual([
      ['you', null, null, false],
      ['claude', 'add_task', 'The email asks for it.', false],
    ]);
    expect(fake.journal[1].rows).toEqual([
      { kind: 'task', key: 'c1', before: undefined, after: { ...task('Claude’s'), id: 'c1' } },
    ]);
  });

  it('undoes the newest change with ⌘Z, and the Edit menu names the one after it', async () => {
    const { heat, call } = boot();
    const a = await heat.put('task', task('First'));
    const b = await heat.put('task', task('Second'));
    expect(await call('history.get', {})).toMatchObject({ undo: 'Undo add task' });
    await call('history.undo', {});
    expect((await heat.snapshot('2026-10-07')).records.task.map((t) => t.id)).toEqual([a.record.id]);
    expect(b.record.id).not.toBe(a.record.id);
    await call('history.undo', {});
    await expect(call('history.undo', {})).rejects.toMatchObject({ code: 'nothing_to_undo' });
    expect(await call('history.get', {})).toEqual({ undo: null, redo: null, cant: null });
  });

  it('undoes one entry out of order, and ⌘Z then skips it without bringing it back', async () => {
    const { fake, heat, call } = boot();
    await heat.put('task', task('Mine'));
    const claude = fake.write(
      'add task',
      ['task'],
      () => fake.store.task.set('c1', { ...task('Claude’s'), id: 'c1' }),
      {
        actor: 'claude',
        tool: 'add_task',
        reason: 'Because.',
      },
    );
    await heat.put('task', task('Later'));

    expect(await call('history.undoEntry', { txnId: claude.txnId })).toEqual({ label: 'add task' });
    let titles = (await heat.snapshot('2026-10-07')).records.task.map((t) => t.title);
    expect(titles).toEqual(['Mine', 'Later']);
    expect(fake.journal.find((e) => e.id === claude.txnId)!.undone).toBe(true);

    // ⌘Z undoes "Later", then "Mine"; Claude's change, already undone, does not return.
    await call('history.undo', {});
    titles = (await heat.snapshot('2026-10-07')).records.task.map((t) => t.title);
    expect(titles).toEqual(['Mine']);
    await call('history.undo', {});
    expect((await heat.snapshot('2026-10-07')).records.task).toEqual([]);
    await expect(call('history.undoEntry', { txnId: claude.txnId })).rejects.toMatchObject({ code: 'already_undone' });
    await expect(call('history.undoEntry', { txnId: 'nope' })).rejects.toMatchObject({ code: 'no_such_entry' });
  });

  it('refuses to undo an entry when a later one touched the same records', async () => {
    const { fake, heat, call } = boot();
    const claude = fake.write(
      'add task',
      ['task'],
      () => fake.store.task.set('c1', { ...task('Claude’s'), id: 'c1' }),
      {
        actor: 'claude',
        tool: 'add_task',
        reason: 'Because.',
      },
    );
    await heat.patch('task', 'c1', { notes: 'I added a note' });
    await expect(call('history.undoEntry', { txnId: claude.txnId })).rejects.toMatchObject({
      code: 'changed_since',
      message: 'This changed again since. Undo the later change first.',
    });
    // Undo the later change, and the earlier one can go.
    await call('history.undo', {});
    expect(await call('history.undoEntry', { txnId: claude.txnId })).toEqual({ label: 'add task' });
    await expect(call('history.undoEntry', { txnId: claude.txnId })).rejects.toMatchObject({ code: 'already_undone' });
  });

  it('names a habit’s tick and untick for the Edit menu', async () => {
    const { heat, call } = boot();
    const { record } = await heat.put('habit', { title: 'Stretch', log: {}, showCounter: false });
    expect((await heat.patch('habit', record.id, { log: { '2026-10-07': true } })).undo).toBe('Undo tick habit');
    expect((await heat.patch('habit', record.id, { log: {} })).undo).toBe('Undo untick habit');
    expect(await call('history.get', {})).toMatchObject({ undo: 'Undo untick habit' });
  });

  it('never lets a record arrive public: only the Public switch does that', async () => {
    const { heat } = boot();
    await expect(heat.put('task', { ...task('Sneaky'), public: true })).rejects.toThrow(
      'The Public switch has its own command.',
    );
    const { record } = await heat.put('task', task('Quiet'));
    expect(record.public).toBe(false);
    await heat.setPublic('task', record.id, true);
    // A whole record put back with its own public flag is fine; a flip through put is not.
    const now = (await heat.snapshot('2026-10-07')).records.task[0];
    await expect(heat.put('task', { ...now, title: 'Renamed' })).resolves.toBeTruthy();
    await heat.setPublic('task', record.id, false);
    const off = (await heat.snapshot('2026-10-07')).records.task[0];
    await expect(heat.put('task', { ...off, public: true })).rejects.toThrow('The Public switch has its own command.');
  });
});
