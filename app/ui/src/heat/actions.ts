// What a person does to a task or a block, in one place for Today, Tasks,
// Calendar, the widgets and the keyboard. Each action is one core command
// (docs/HEAT.md); a refusal is said in the core's own sentence. The core
// names each change for ⌘Z ("Undo mark done"); an action adds a line of its
// own only where the person needs to hear what happened ("Done. Took 1h
// 15m across 3 focus sessions.").

import { useMemo } from 'react';
import { type DayKey, minuteOfDay } from '../shared/time/zone';
import type { Id, Snapshot, Task, TimeBlock } from './client';
import { clock, copy, formatMinutes } from './fmt';
import { useFrame } from './frame';
import { blockLength, dropStart, movedStart, nextGap, resizedMinutes } from './today/gap';
import { useHeat } from './store';

export type NewTask = Partial<Omit<Task, 'id'>> & { title: string; spaceId: Id };

export function useActions() {
  const heat = useHeat();
  const frame = useFrame();
  const { client, snap, idx, tz, date, act, say, now } = heat;
  const { selection, focusLength, askTook, selectTask, selectBlock } = frame;

  return useMemo(() => {
    const derived = (id: Id) => snap?.derived.tasks[id];
    const timer = snap?.heatState.timer;
    const running = timer ? (timer.running ?? timer.endsAt !== null) : false;

    /** The task a block would be as long as: its estimate, rounded up to 15 minutes. */
    const lengthFor = (id: Id) => blockLength(derived(id)?.estimate.min ?? 30);

    const actions = {
      /** A tick, or checking off: with no logged time, asks "Time it took" first. */
      async toggleDone(taskId: Id, day?: DayKey) {
        const t = idx.task.get(taskId);
        if (!t) return;
        if (t.rrule) {
          const target = day ?? derived(taskId)?.next ?? date;
          const ticked = idx.ticked.get(taskId)?.has(target) ?? false;
          const r = await act(client.done(taskId, !ticked, target));
          if (r?.took) say(r.took);
          return;
        }
        if (t.done) {
          await act(client.done(taskId, false));
          return;
        }
        if ((derived(taskId)?.actualMin ?? 0) <= 0 && !idx.sessions.get(taskId)) {
          askTook(taskId);
          return;
        }
        const r = await act(client.done(taskId, true));
        if (r) say(r.took ?? 'Done.');
      },

      /** The "Time it took" sheet's answer; no minutes means done without a time. */
      async finish(taskId: Id, minutes: number | null) {
        if (minutes !== null && minutes > 0) await act(client.tookTime(taskId, minutes));
        const r = await act(client.done(taskId, true));
        if (r) say(r.took ?? 'Done.');
      },

      async remove(taskId: Id) {
        const t = idx.task.get(taskId);
        if (!t) return;
        const r = await act(client.delete('task', taskId));
        if (!r) return;
        if (selection.taskId === taskId) selectTask(null);
        say(`Deleted ‘${t.title}’.`);
      },

      async makeCurrent(taskId: Id | null) {
        await act(client.setCurrent(taskId));
      },

      /** P: the task into the next free gap after now. `from` is a fresher snapshot, for a task made a moment ago. */
      async place(taskId: Id, from: Snapshot | null = snap) {
        const t = from?.records.task.find((x) => x.id === taskId);
        if (!t || !from) return;
        const length = blockLength(from.derived.tasks[taskId]?.estimate.min ?? 30);
        const start = nextGap(from.records.timeBlock, from.events, date, tz, minuteOfDay(now(), tz), length);
        if (start === null) {
          say('No free gap is left today.');
          return;
        }
        const r = await act(client.putBlock({ taskId, date, start, minutes: length }));
        if (r) {
          selectBlock(r.block.id);
          say(`Planned ‘${t.title}’ at ${clock(start)}, ${formatMinutes(length)}.`);
        }
      },

      /** A task made a moment ago goes into the plan: read its estimate from the core first. */
      async placeNew(taskId: Id) {
        await actions.place(taskId, await act(client.snapshot(date)));
      },

      /** A task dropped on a day's column at `y` px from its 7 AM top. */
      async dropOnColumn(taskId: Id, y: number, day: DayKey) {
        if (!idx.task.has(taskId)) return;
        const minutes = lengthFor(taskId);
        const r = await act(client.putBlock({ taskId, date: day, start: dropStart(y, minutes), minutes }));
        if (r) selectBlock(r.block.id);
      },

      /** A block's bottom edge dragged to `bottomY`; resizing changes the block, never the estimate. */
      async resizeBlock(block: TimeBlock, bottomY: number) {
        const minutes = resizedMinutes(block.start, bottomY);
        if (minutes !== block.minutes) await act(client.putBlock({ ...block, minutes }));
      },

      async setBlock(block: TimeBlock, change: { start?: number; minutes?: number; date?: DayKey }) {
        await act(client.putBlock({ ...block, ...change }));
      },

      /** A block's body dragged so its top lands at `topY`. */
      async moveBlock(block: TimeBlock, topY: number, day?: DayKey) {
        const start = movedStart(block.minutes, topY);
        if (start !== block.start || (day && day !== block.date))
          await act(client.putBlock({ ...block, start, date: day ?? block.date }));
      },

      async removeBlock(blockId: Id) {
        const r = await act(client.delete('timeBlock', blockId));
        if (r && selection.blockId === blockId) selectBlock(null);
      },

      async schedule(taskId: Id, day: DayKey) {
        const t = idx.task.get(taskId);
        if (t && t.scheduledDate !== day) await act(client.patch('task', taskId, { scheduledDate: day }));
      },

      async makeTask(fields: NewTask) {
        const space = idx.space.get(fields.spaceId);
        const record = {
          type: space?.types[0] ?? 'Other',
          due: null,
          difficulty: 3,
          estMin: null,
          adjustMin: 0,
          notes: '',
          done: false,
          doneAt: null,
          source: 'you' as const,
          ...fields,
        };
        const r = await act(client.put('task', record));
        return r?.record ?? null;
      },

      // The Pomodoro timer. Nothing starts without a press (3.5).
      /** F: start on the selection or the current task; else pause, or resume. */
      async focusKey() {
        if (!timer || !snap) return;
        if (timer.phase === 'idle') {
          const target = selection.taskId ?? snap.heatState.currentTaskId ?? null;
          if (target) await act(client.focus('start', { taskId: target, length: focusLength }));
          return;
        }
        await act(client.focus(running ? 'pause' : 'resume'));
      },
      /** ⇧F: stop and log what was done. */
      async stopFocus() {
        const r = await act(client.focus('stop'));
        if (r?.logged) say(r.heatState.timer.note ?? copy.focus.stopped(formatMinutes(r.logged.focusMin), ''));
      },
      /** I: pulled away. */
      async pulledAway() {
        if (timer?.phase === 'focus' && running) await act(client.focus('interrupt'));
      },

      async planMyDay() {
        const r = await act(client.planMake(date));
        if (r && r.drafts.length === 0) say('Nothing is left to plan before the day ends.');
      },
      async acceptDrafts(taskIds?: Id[]) {
        await act(client.planAccept(date, taskIds));
      },
      async clearDrafts() {
        await act(client.planClear());
      },
    };
    return actions;
  }, [client, snap, idx, tz, date, act, say, now, selection, focusLength, askTook, selectTask, selectBlock]);
}

export type Actions = ReturnType<typeof useActions>;
