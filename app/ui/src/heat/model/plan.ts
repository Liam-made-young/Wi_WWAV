// Today: the time column, Plan my day, and the plan list (docs/SPEC.md 3.5).
//
// The column reuses the PKM calendar's scale (portfolio/src/pkm/calendarUtils.js):
// 44 px an hour, a 15-minute snap and 30-minute default blocks, here from 7 AM
// to midnight. Plan my day is a written rule, not Claude: unplanned open tasks
// in heat order, each a block of its estimate rounded up to 15 minutes and
// capped at 90, in the first gap that fits between now and "Day ends at".

import { clock, longDay } from '../../shared/time/format';
import { addDays, type DayKey, dayKey, minuteOfDay, startOfDay } from '../../shared/time/zone';
import * as copy from './copy';
import { estimateContext, type EstimateContext, estimateMin, formatMinutes } from './estimate';
import { byHeat, duePhrase, heatOf } from './heat';
import type { CalendarEvent, FocusSession, Habit, Id, Task, TaskOccurrence, TimeBlock } from './records';
import { openTasks, taskOccurrences } from './recurrence';
import { inSpace } from './spaces';

export const HOUR_PX = 44;
export const SNAP_MIN = 15;
export const DEFAULT_BLOCK_MIN = 30;
const COLUMN_START = 7 * 60;
const COLUMN_END = 24 * 60;
export const COLUMN_HEIGHT = ((COLUMN_END - COLUMN_START) / 60) * HOUR_PX;
const PLAN_CAP_MIN = 90;
export const DAY_ENDS_AT = 23 * 60;
const HOT_SUGGESTIONS = 5;

export interface PlanData {
  tasks: Task[];
  occurrences: TaskOccurrence[];
  blocks: TimeBlock[];
  events: CalendarEvent[];
  sessions: FocusSession[];
  habits: Habit[];
}

/** What a block is for: a task or a habit with a length. */
export type BlockTarget = { taskId: Id } | { habitId: Id };

export interface Draft {
  taskId: Id;
  date: DayKey;
  start: number;
  minutes: number;
  /** Estimate past the 90-minute cap, still to plan. */
  leftMin: number;
  reason: string;
  leftLine: string | null;
}

const clamp = (x: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, x));

export const minutesToY = (min: number) => ((min - COLUMN_START) / 60) * HOUR_PX;
export const yToMinutes = (y: number) => COLUMN_START + (y / HOUR_PX) * 60;
/** The nearest 15-minute mark, as the PKM snaps a drag. */
export const snap = (min: number) => Math.round(min / SNAP_MIN) * SNAP_MIN;
const roundUp = (min: number) => Math.ceil(min / SNAP_MIN) * SNAP_MIN;

/** Scroll the column so now sits a third of the way down. */
export function initialScrollTop(nowMin: number, viewportPx: number): number {
  return clamp(minutesToY(nowMin) - viewportPx / 3, 0, Math.max(0, COLUMN_HEIGHT - viewportPx));
}

/** Where the 1 px red line marks the current minute; null outside the column. */
export function nowLineY(nowMin: number): number | null {
  return nowMin < COLUMN_START || nowMin >= COLUMN_END ? null : minutesToY(nowMin);
}

/** A block as long as an estimate, rounded up to 15 minutes. */
export function blockLength(estimate: number): number {
  return Math.max(SNAP_MIN, roundUp(estimate));
}

type Span = [number, number];

// Today's busy minutes: every block and every timed event, whatever space it is in.
function busySpans(data: Pick<PlanData, 'blocks' | 'events'>, today: DayKey, tz: string): Span[] {
  const dayStart = startOfDay(today, tz);
  const dayEnd = startOfDay(addDays(today, 1), tz);
  const spans: Span[] = data.blocks.filter((b) => b.date === today).map((b) => [b.start, b.start + b.minutes]);
  for (const e of data.events) {
    if (e.allDay || e.end <= dayStart || e.start >= dayEnd) continue;
    spans.push([e.start <= dayStart ? 0 : minuteOfDay(e.start, tz), e.end >= dayEnd ? 1440 : minuteOfDay(e.end, tz)]);
  }
  return spans;
}

// The first start on the 15-minute grid, from `from` on, where `length` fits before `until`.
function firstGap(spans: readonly Span[], from: number, until: number, length: number): number | null {
  for (let s = roundUp(from); s + length <= until; s += SNAP_MIN) {
    if (spans.every(([a, b]) => s + length <= a || s >= b)) return s;
  }
  return null;
}

function targetLength(target: BlockTarget, data: Pick<PlanData, 'tasks' | 'habits'>, ctx: EstimateContext): number | null {
  if ('taskId' in target) {
    const t = data.tasks.find((x) => x.id === target.taskId);
    return t ? blockLength(estimateMin(t, ctx)) : null;
  }
  const h = data.habits.find((x) => x.id === target.habitId);
  return h ? blockLength(h.minutes ?? DEFAULT_BLOCK_MIN) : null;
}

function newBlock(target: BlockTarget, date: DayKey, start: number, minutes: number, newId: () => Id): TimeBlock {
  return { id: newId(), ...target, date, start, minutes, origin: 'you' };
}

/** P: the selected task or habit into the next free gap after now. */
export function planNext(target: BlockTarget, data: PlanData, now: number, tz: string, newId: () => Id): TimeBlock | null {
  const length = targetLength(target, data, estimateContext(data.tasks, data.sessions));
  if (length === null) return null;
  const today = dayKey(now, tz);
  const start = firstGap(busySpans(data, today, tz), minuteOfDay(now, tz), COLUMN_END, length);
  return start === null ? null : newBlock(target, today, start, length, newId);
}

/** A task or habit dropped on the column at `y` becomes a block as long as its estimate. */
export function dragOntoColumn(
  target: BlockTarget,
  y: number,
  date: DayKey,
  data: PlanData,
  newId: () => Id,
): TimeBlock | null {
  const length = targetLength(target, data, estimateContext(data.tasks, data.sessions));
  if (length === null) return null;
  return newBlock(target, date, clamp(snap(yToMinutes(y)), COLUMN_START, COLUMN_END - length), length, newId);
}

/** Dragging a block's bottom edge. It changes the block, never the task's estimate. */
export function resizeBlock(block: TimeBlock, bottomY: number): TimeBlock {
  return { ...block, minutes: clamp(snap(yToMinutes(bottomY) - block.start), SNAP_MIN, COLUMN_END - block.start) };
}

/** Dragging a block's body to a new top. */
export function moveBlock(block: TimeBlock, topY: number): TimeBlock {
  return { ...block, start: clamp(snap(yToMinutes(topY)), COLUMN_START, COLUMN_END - block.minutes) };
}

/** "Block ends 3:30 PM", for the Now widget. */
export function blockEndsLine(block: TimeBlock): string {
  return copy.today.blockEnds(clock(block.start + block.minutes));
}

/** A draft's reason: "Due tomorrow 11:59 PM, Hot." */
export function planReason(task: Task, now: number, tz: string): string {
  if (task.due === null) return copy.draft.noDue;
  const phrase = duePhrase(task.due, now, tz);
  const level = heatOf(task, now).level;
  return level === 'Overdue' ? copy.draft.overdue(phrase) : copy.draft.reason(phrase, level);
}

/** Plan my day: dashed drafts, nothing saved until they are accepted. */
export function planMyDay(
  data: PlanData,
  now: number,
  tz: string,
  options: { dayEndsAt?: number; spaceId?: Id } = {},
): Draft[] {
  const today = dayKey(now, tz);
  const ctx = estimateContext(data.tasks, data.sessions);
  const plannedToday = new Set(data.blocks.filter((b) => b.date === today).map((b) => b.taskId));
  // A parent's estimate is its open children's; plan the children instead.
  const parents = new Set(data.tasks.filter((t) => !t.done && t.parentTaskId).map((t) => t.parentTaskId));
  const candidates = openTasks(data.tasks, data.occurrences, now, tz).filter(
    (t) => inSpace(options.spaceId)(t) && !plannedToday.has(t.id) && !parents.has(t.id),
  );
  const spans = busySpans(data, today, tz);
  const nowMin = minuteOfDay(now, tz);
  const drafts: Draft[] = [];
  for (const t of byHeat(candidates, now)) {
    const full = blockLength(estimateMin(t, ctx));
    const minutes = Math.min(PLAN_CAP_MIN, full);
    const start = firstGap(spans, nowMin, options.dayEndsAt ?? DAY_ENDS_AT, minutes);
    if (start === null) continue;
    spans.push([start, start + minutes]);
    const leftMin = full - minutes;
    drafts.push({
      taskId: t.id,
      date: today,
      start,
      minutes,
      leftMin,
      reason: planReason(t, now, tz),
      leftLine: leftMin > 0 ? copy.draft.leftToPlan(formatMinutes(leftMin)) : null,
    });
  }
  return drafts;
}

/** A click on a draft accepts that one. */
export function acceptDraft(d: Draft, newId: () => Id): TimeBlock {
  return { id: newId(), taskId: d.taskId, date: d.date, start: d.start, minutes: d.minutes, origin: 'plan' };
}

/** Return accepts all the drafts and Esc clears them; any other key, or no drafts, does nothing here. */
export function draftKey(
  drafts: readonly Draft[],
  key: string,
  newId: () => Id,
): { drafts: Draft[]; blocks: TimeBlock[] } | null {
  if (drafts.length === 0) return null;
  if (key === 'Enter') return { drafts: [], blocks: drafts.map((d) => acceptDraft(d, newId)) };
  if (key === 'Escape') return { drafts: [], blocks: [] };
  return null;
}

export interface PlannedRow {
  block: TimeBlock;
  title: string;
  time: string;
  length: string;
  /** The block now is in: it wears the selection ring. */
  current: boolean;
  /** A block whose time is over: it dims. */
  finished: boolean;
}

export interface RecurringRow {
  kind: 'task' | 'habit';
  id: Id;
  title: string;
  done: boolean;
}

export type PlanSection =
  | { kind: 'planned'; title: string; items: PlannedRow[] }
  | { kind: 'dueToday'; title: string; items: Task[] }
  | { kind: 'recurring'; title: string; items: RecurringRow[] }
  | { kind: 'hot'; title: string; items: Task[] };

// Today's blocks in the space filter: a task's only if its task is in the
// space, a habit's always, since habits belong to the person (3.4).
function blocksToday(data: Pick<PlanData, 'tasks' | 'blocks'>, today: DayKey, spaceId?: Id): TimeBlock[] {
  const tasks = new Set(data.tasks.filter(inSpace(spaceId)).map((t) => t.id));
  return data.blocks.filter((b) => b.date === today && (b.habitId !== undefined || tasks.has(b.taskId!)));
}

function isDueOn(t: Task, day: DayKey, tz: string): boolean {
  return t.due !== null && dayKey(t.due, tz) === day;
}

/** The plan list's four sections, in order; an empty one is left out. */
export function planSections(data: PlanData, now: number, tz: string, spaceId?: Id): PlanSection[] {
  const today = dayKey(now, tz);
  const nowMin = minuteOfDay(now, tz);
  const blocks = blocksToday(data, today, spaceId).sort((a, b) => a.start - b.start);
  const planned = new Set(blocks.map((b) => b.taskId));
  const open = openTasks(data.tasks.filter(inSpace(spaceId)), data.occurrences, now, tz);

  const recurringTasks = data.tasks.filter(
    (t) => inSpace(spaceId)(t) && t.rrule && !t.done && taskOccurrences(t, tz, today, today).length > 0,
  );
  const recurringIds = new Set(recurringTasks.map((t) => t.id));
  const tickedToday = new Set(data.occurrences.filter((o) => o.date === today).map((o) => o.taskId));
  const recurring: RecurringRow[] = [
    ...recurringTasks.map((t) => ({ kind: 'task' as const, id: t.id, title: t.title, done: tickedToday.has(t.id) })),
    ...data.habits
      .filter((h) => h.minutes !== undefined)
      .map((h) => ({ kind: 'habit' as const, id: h.id, title: h.title, done: h.log[today] === true })),
  ];

  const unplanned = byHeat(open.filter((t) => !planned.has(t.id) && !recurringIds.has(t.id)), now);
  const dueToday = unplanned.filter((t) => isDueOn(t, today, tz));
  const hot = unplanned
    .filter((t) => !isDueOn(t, today, tz) && ['Hot', 'Overdue'].includes(heatOf(t, now).level))
    .slice(0, HOT_SUGGESTIONS);

  const title = (b: TimeBlock) =>
    b.habitId !== undefined
      ? data.habits.find((h) => h.id === b.habitId)?.title
      : data.tasks.find((t) => t.id === b.taskId)?.title;
  const rows: PlannedRow[] = blocks.flatMap((b) => {
    const t = title(b);
    if (t === undefined) return [];
    const end = b.start + b.minutes;
    return [
      {
        block: b,
        title: t,
        time: clock(b.start),
        length: formatMinutes(b.minutes),
        current: b.start <= nowMin && nowMin < end,
        finished: end <= nowMin,
      },
    ];
  });

  const sections: PlanSection[] = [
    { kind: 'planned', title: copy.today.planned, items: rows },
    { kind: 'dueToday', title: copy.today.dueToday, items: dueToday },
    { kind: 'recurring', title: copy.today.recurring, items: recurring },
    { kind: 'hot', title: copy.today.hot, items: hot },
  ];
  return sections.filter((s) => s.items.length > 0);
}

/** "Today, Tuesday, October 6" */
export function planHeader(now: number, tz: string): string {
  return copy.today.header(longDay(dayKey(now, tz)));
}

/** "4 blocks · 3h 10m planned · 2 due today" */
export function planSubtitle(data: PlanData, now: number, tz: string, spaceId?: Id): string {
  const today = dayKey(now, tz);
  const blocks = blocksToday(data, today, spaceId);
  const minutes = blocks.reduce((sum, b) => sum + b.minutes, 0);
  const due = openTasks(data.tasks.filter(inSpace(spaceId)), data.occurrences, now, tz).filter((t) =>
    isDueOn(t, today, tz),
  );
  return copy.today.subtitle(blocks.length, formatMinutes(minutes), due.length);
}
