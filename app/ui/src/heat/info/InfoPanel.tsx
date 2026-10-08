// Get Info for the selected task or block (docs/SPEC.md 3.5, 3.6, 3.15): it
// slides over the right column while something is selected, and Esc slides
// it back. Every field saves as it is left, as its own undo step with the
// core's label ("Undo rename task", "Undo estimate"); Return and ⌘I put the
// keyboard in the first field. A task Claude added shows its source and the
// reason it gave; an estimate Claude made says so in the field's hint, and
// one a type gave names the type and where it is defined. The line under the
// source says where the task belongs: its course, else its project, else its
// space. The Public switch is off until it is turned on, and is one item at
// a time.

import { type ReactNode, useState } from 'react';
import { shortMonthDay, clock } from '../../shared/time/format';
import { dayKey, minuteOfDay } from '../../shared/time/zone';
import { useActions } from '../actions';
import type { Id, Task, TaskEstimate, TimeBlock } from '../client';
import { copy, courseLabel, dueText, effectiveDue, formatMinutes, homeLabel, plural } from '../fmt';
import { useSelection } from '../frame';
import { fieldsOf, instantOf } from '../sheets';
import { useHeat } from '../store';
import { ulidTime } from '../ulid';
import { Heat } from '../ui';
import './info.css';

export function InfoPanel() {
  const { taskId, blockId } = useSelection();
  const { idx } = useHeat();
  if (blockId && idx.block.has(blockId)) return <BlockInfo key={blockId} id={blockId} />;
  if (taskId && idx.task.has(taskId)) return <TaskInfo key={taskId} id={taskId} />;
  return <aside className="heat-info" aria-label="Get Info" />;
}

// --- fields ---------------------------------------------------------------------

/** A text, number, date or time field that saves when you leave it, or press Return. */
function Field({
  label,
  saved,
  commit,
  type = 'text',
  first,
  ...rest
}: {
  label: string;
  saved: string;
  commit(value: string): void;
  type?: string;
  first?: boolean;
} & Omit<React.InputHTMLAttributes<HTMLInputElement>, 'type' | 'value' | 'onChange'>) {
  const [draft, setDraft] = useState<string | null>(null);
  const finish = () => {
    if (draft !== null && draft !== saved) commit(draft);
    setDraft(null);
  };
  return (
    <label className="heat-field">
      <span data-text="secondary">{label}</span>
      <input
        {...rest}
        type={type}
        value={draft ?? saved}
        data-info-first={first ? '' : undefined}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={finish}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            finish();
          }
        }}
      />
    </label>
  );
}

function Select({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: { value: string; label: string }[];
  onChange(value: string): void;
}) {
  return (
    <label className="heat-field">
      <span data-text="secondary">{label}</span>
      <select value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function Frame({ title, children, onClose }: { title: string; children: ReactNode; onClose(): void }) {
  return (
    <aside className="heat-info" aria-label={`Get Info: ${title}`}>
      <button
        type="button"
        className="heat-info-close"
        data-dense
        aria-label="Close Get Info"
        title="Close (Esc)"
        onClick={onClose}
      >
        ×
      </button>
      {children}
    </aside>
  );
}

const REPEATS: { value: string; label: string; rule: string | null }[] = [
  { value: 'none', label: copy.repeat.none, rule: null },
  { value: 'daily', label: copy.repeat.daily, rule: 'FREQ=DAILY' },
  { value: 'weekdays', label: copy.repeat.weekdays, rule: 'FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR' },
  { value: 'weekly', label: copy.repeat.weekly, rule: 'FREQ=WEEKLY' },
  { value: 'monthly', label: copy.repeat.monthly, rule: 'FREQ=MONTHLY' },
  { value: 'custom', label: copy.repeat.custom, rule: null },
];

const repeatOf = (rrule: string | undefined) =>
  !rrule ? 'none' : (REPEATS.find((r) => r.rule === rrule)?.value ?? 'custom');

/** Clears an optional field: the core reads null as "none". */
const NONE = null as never;

// --- a task --------------------------------------------------------------------

function TaskInfo({ id }: { id: Id }) {
  const heat = useHeat();
  const { snap, idx, tz, date, now, client, act, say } = heat;
  const actions = useActions();
  const { selectTask } = useSelection();
  const task = idx.task.get(id)!;
  const d = snap?.derived.tasks[id];
  const space = idx.space.get(task.spaceId);
  const [customRule, setCustomRule] = useState<string | null>(null);

  const patch = (set: Partial<Task>) => void act(client.patch('task', id, set));
  const due = fieldsOf(task.due, tz);
  const shown = effectiveDue(task, d, tz);
  const sessions = idx.sessions.get(id) ?? 0;
  const minutes = d?.estimate.min ?? 0;

  const estimate = async (set: { difficulty?: number; estMin?: number }) => {
    const r = await act(client.estimate(id, set));
    if (r?.clamped) say('Minutes are kept between 5 and 600.');
  };

  const commitDue = (day: string, time: string) => patch({ due: instantOf(day, time, tz) });

  const makePublic = async (on: boolean) => {
    const r = await act(client.setPublic('task', id, on));
    if (r?.sentence) say(r.sentence);
  };

  const source = sourceLine(task, tz);
  const children = (snap?.records.task ?? []).filter((t) => t.parentTaskId === id).map((t) => t.id);
  const parents = (snap?.records.task ?? []).filter(
    (t) => t.spaceId === task.spaceId && t.id !== id && !t.done && !t.parentTaskId && !children.includes(t.id),
  );
  const courses = snap?.records.course ?? [];
  const milestones = (snap?.records.milestone ?? []).filter((m) => m.spaceId === task.spaceId);
  const projects = (snap?.records.project ?? []).filter((p) => p.spaceId === task.spaceId);
  const nameOf = (c: (typeof courses)[number]) => snap?.derived.courses[c.id]?.label ?? courseLabel(c);

  // Where the task belongs: its course, else its project, else its space.
  const course = task.courseId ? idx.course.get(task.courseId) : undefined;
  const home = homeLabel(d, {
    course: course ? nameOf(course) : undefined,
    project: task.projectId ? idx.project.get(task.projectId)?.title : undefined,
    space: space?.name,
  });

  // The types a picker offers: the home's, the space's, the defaults, and the one the task has.
  const offered = snap?.derived.types;
  const typeNames = [
    ...new Set([
      ...(task.courseId ? (offered?.courses[task.courseId] ?? []) : []),
      ...(task.projectId ? (offered?.projects[task.projectId] ?? []) : []),
      ...(offered?.spaces[task.spaceId] ?? []),
      ...(space?.types ?? []),
      ...(offered?.global ?? []),
      task.type,
    ]),
  ].filter(Boolean);
  // A type the title or Claude picked reads as Automatic, with the type it came to.
  const automatic = task.typeBy !== undefined && task.typeBy !== 'you';
  const where =
    d?.estimate.typeFrom === 'course' || d?.estimate.typeFrom === 'project'
      ? home
      : d?.estimate.typeFrom === 'space'
        ? (space?.name ?? null)
        : d?.estimate.typeFrom === 'global'
          ? copy.types.yourDefaults
          : null;
  const unscored = !task.done && !d?.estimate.typeFrom && (snap?.derived.unscored ?? 0) > 0;
  const setByHand = task.estBy === 'you' || task.difficultyBy === 'you';

  return (
    <Frame title={task.title} onClose={() => selectTask(null)}>
      <Field
        label="Task"
        saved={task.title}
        first
        commit={(title) => (title.trim() ? patch({ title: title.trim() }) : say(copy.tasks.nameFirst))}
      />
      <div className="heat-info-heat">
        {d && <Heat level={d.heat.level} v={d.heat.v} />}
        {shown !== null && !task.done && <span data-text="secondary">{dueText(shown, now(), tz)}</span>}
      </div>
      <p className="heat-source-line" data-text="secondary">
        {source.line}
      </p>
      {source.reason && <p className="heat-reason">{source.reason}</p>}
      {home && (
        <p className="heat-source-line heat-home-line" data-text="secondary">
          {copy.homes.in(home)}
        </p>
      )}

      <div className="heat-fields">
        <Select
          label="Space"
          value={task.spaceId}
          options={(snap?.records.space ?? []).map((s) => ({ value: s.id, label: s.name }))}
          onChange={(spaceId) => patch({ spaceId })}
        />
        {space?.groupKind === 'course' && (
          <Select
            label={space.groupLabel}
            value={task.courseId ?? ''}
            options={[{ value: '', label: 'None' }, ...courses.map((c) => ({ value: c.id, label: nameOf(c) }))]}
            onChange={(v) => patch({ courseId: v || NONE })}
          />
        )}
        {space?.groupKind === 'milestone' && (
          <Select
            label={space.groupLabel}
            value={task.milestoneId ?? ''}
            options={[{ value: '', label: 'None' }, ...milestones.map((m) => ({ value: m.id, label: m.title }))]}
            onChange={(v) => patch({ milestoneId: v || NONE })}
          />
        )}
        {space?.groupKind === 'free' && (
          <Field label={space.groupLabel} saved={task.group ?? ''} commit={(v) => patch({ group: v.trim() || NONE })} />
        )}
        {projects.length > 0 && (
          <Select
            label="Project"
            value={task.projectId ?? ''}
            options={[{ value: '', label: 'None' }, ...projects.map((p) => ({ value: p.id, label: p.title }))]}
            onChange={(v) => patch({ projectId: v || NONE })}
          />
        )}
        <Select
          label="Type"
          value={automatic ? '' : task.type}
          options={[
            { value: '', label: automatic && task.type ? copy.types.automaticAs(task.type) : copy.types.automatic },
            ...typeNames.map((t) => ({ value: t, label: t })),
          ]}
          onChange={(type) => void act(client.setType(id, type || null))}
        />
        <Field label="Due" type="date" saved={due.date} commit={(day) => commitDue(day, due.time)} />
        <Field
          label="Time"
          type="time"
          saved={due.time}
          disabled={!due.date}
          commit={(time) => commitDue(due.date, time)}
        />
        <Field
          label="Scheduled"
          type="date"
          saved={task.scheduledDate ?? ''}
          commit={(v) => patch({ scheduledDate: v || NONE })}
        />
        <Select
          label="Repeat"
          value={customRule !== null ? 'custom' : repeatOf(task.rrule)}
          options={REPEATS.map((r) => ({ value: r.value, label: r.label }))}
          onChange={(v) => {
            if (v === 'custom') return setCustomRule(task.rrule ?? '');
            setCustomRule(null);
            const rule = REPEATS.find((r) => r.value === v)?.rule ?? null;
            // A rule needs a day to start from: a task with neither date is scheduled for today.
            patch(
              rule && task.due === null && !task.scheduledDate
                ? { rrule: rule, scheduledDate: date }
                : { rrule: rule ?? NONE },
            );
          }}
        />
        {(customRule !== null || repeatOf(task.rrule) === 'custom') && (
          <Field
            label="Rule"
            saved={customRule ?? task.rrule ?? ''}
            commit={(rule) => {
              setCustomRule(null);
              patch({ rrule: rule.trim() || NONE });
            }}
            placeholder="FREQ=WEEKLY;BYDAY=MO,WE"
          />
        )}
        <Select
          label="Difficulty"
          value={String(task.difficulty)}
          options={[1, 2, 3, 4, 5].map((n) => ({ value: String(n), label: String(n) }))}
          onChange={(v) => void estimate({ difficulty: Number(v) })}
        />
        <Field
          label="Estimate"
          type="number"
          min={5}
          max={600}
          saved={String(minutes)}
          commit={(v) => Number(v) > 0 && void estimate({ estMin: Number(v) })}
        />
        <p className="heat-hint" data-text="secondary">
          {estimateHint(task, d?.estimate, where, unscored)}
        </p>
        {d?.estimate.by === 'claude' && d.estimate.reason && <p className="heat-reason">{d.estimate.reason}</p>}
        {setByHand && (
          <p className="heat-hint">
            <button
              type="button"
              className="gel plain"
              onClick={() => void act(client.reapplyDefaults({ taskId: id, force: true }))}
            >
              {copy.types.useDefault}
            </button>
          </p>
        )}
        <Field
          label={copy.tasks.took}
          type="number"
          min={0}
          saved={String(d?.actualMin ?? 0)}
          commit={(v) => void act(client.tookTime(id, Math.max(0, Math.round(Number(v)))))}
        />
        <p className="heat-hint" data-text="secondary">
          {d && d.actualMin > 0
            ? `${formatMinutes(d.actualMin)}${sessions > 0 ? ` across ${plural(sessions, 'focus session')}` : ''}. Minutes you set here adjust it.`
            : 'No time is logged yet. Focus sessions add to it.'}
        </p>
        <label className="heat-field heat-field-wide">
          <span data-text="secondary">Notes</span>
          <TextArea saved={task.notes} commit={(notes) => patch({ notes })} />
        </label>
        {parents.length > 0 || task.parentTaskId ? (
          <Select
            label="Part of"
            value={task.parentTaskId ?? ''}
            options={[{ value: '', label: 'Nothing' }, ...parents.map((t) => ({ value: t.id, label: t.title }))]}
            onChange={(v) => patch({ parentTaskId: v || NONE })}
          />
        ) : null}
        {task.link && (
          <p className="heat-hint" data-text="secondary">
            Linked to a {LINKS[task.link.kind]}.
          </p>
        )}
      </div>

      <label className="check heat-switch" data-dense>
        <input
          type="checkbox"
          role="switch"
          checked={task.public === true}
          onChange={(e) => void makePublic(e.target.checked)}
        />
        Public
      </label>
      <p className="heat-hint" data-text="secondary">
        {task.public
          ? 'Anyone who opens your sun can see this task’s title, due date and whether it is done.'
          : 'Private. Turn this on to show the task’s title, due date and whether it is done to anyone who opens your sun.'}
      </p>

      <div className="heat-info-actions">
        <button type="button" className="gel" onClick={() => void actions.toggleDone(id)} title="Mark done (⌘↩)">
          {task.done || (task.rrule && idx.ticked.get(id)?.has(date)) ? 'Mark not done' : copy.tasks.markDone}
        </button>
        <button type="button" className="gel" onClick={() => void actions.remove(id)} title="Delete (⌫)">
          Delete
        </button>
      </div>
    </Frame>
  );
}

const LINKS = { session: 'Console session', system: 'solar system', work: 'work' } as const;

function TextArea({ saved, commit }: { saved: string; commit(value: string): void }) {
  const [draft, setDraft] = useState<string | null>(null);
  return (
    <textarea
      rows={3}
      value={draft ?? saved}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        if (draft !== null && draft !== saved) commit(draft);
        setDraft(null);
      }}
    />
  );
}

/** "Added by you", "Brightspace calendar", or "Claude, Oct 6 8:41 AM" with the reason Claude gave. */
function sourceLine(task: Task, tz: string): { line: string; reason?: string } {
  if (task.source === 'claude') {
    const at = ulidTime(task.id);
    const when = at === null ? '' : `, ${shortMonthDay(dayKey(at, tz))} ${clock(minuteOfDay(at, tz))}`;
    return { line: `Claude${when}`, reason: task.claudeReason };
  }
  return { line: copy.source[task.source as keyof typeof copy.source] ?? copy.source.you };
}

/**
 * The estimate field's hint: who made it, and from what. `where` names the
 * level a type's numbers came from ("JPN 101", "Classes", "your defaults");
 * `unscored` says no type matched and Claude has yet to give better.
 */
function estimateHint(task: Task, estimate: TaskEstimate | undefined, where: string | null, unscored: boolean): string {
  if (!estimate) return '';
  if (estimate.by === 'claude') {
    return `Claude's estimate: ${formatMinutes(estimate.min)}, difficulty ${task.difficulty}. It read the title, the notes and your past averages.`;
  }
  if (estimate.by === 'you') return 'Set by you.';
  if (estimate.by === 'type') return copy.types.fromType(task.type, where);
  if (unscored) return copy.types.noMatch;
  return 'Learn’s estimate: your average for this type, or difficulty × 20 minutes.';
}

// --- a block -------------------------------------------------------------------

function BlockInfo({ id }: { id: Id }) {
  const { idx } = useHeat();
  const actions = useActions();
  const { selectBlock, selectTask } = useSelection();
  const block = idx.block.get(id)!;
  const task = block.taskId ? idx.task.get(block.taskId) : undefined;
  const habit = block.habitId ? idx.habit.get(block.habitId) : undefined;
  const title = task?.title ?? habit?.title ?? 'Block';
  const hh = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
  const set = (change: Partial<TimeBlock>) => void actions.setBlock(block, change);

  return (
    <Frame title={title} onClose={() => selectBlock(null)}>
      <p className="heat-info-name">{title}</p>
      <p className="heat-source-line" data-text="secondary">
        {block.origin === 'plan' ? 'Made by Plan my day' : 'Added by you'}
      </p>
      <div className="heat-fields">
        <Field label="Day" type="date" first saved={block.date} commit={(date) => date && set({ date })} />
        <Field
          label="Starts"
          type="time"
          step={900}
          saved={hh(block.start)}
          commit={(v) => {
            const [h, m] = v.split(':').map(Number);
            if (Number.isFinite(h)) set({ start: h * 60 + m });
          }}
        />
        <Field
          label="Minutes"
          type="number"
          min={15}
          step={15}
          saved={String(block.minutes)}
          commit={(v) => Number(v) > 0 && set({ minutes: Number(v) })}
        />
        <p className="heat-hint" data-text="secondary">
          {clock(block.start)} to {clock(block.start + block.minutes)}. Resizing a block never changes the task’s
          estimate.
        </p>
        {task && (
          <button type="button" className="gel" onClick={() => selectTask(task.id)}>
            Open the task
          </button>
        )}
      </div>
      <label className="check heat-switch" data-dense>
        <input type="checkbox" role="switch" checked={task?.public === true} disabled />
        Public
      </label>
      <p className="heat-hint" data-text="secondary">
        A block follows its task’s Public switch.
      </p>
      <div className="heat-info-actions">
        <button type="button" className="gel" onClick={() => void actions.removeBlock(id)} title="Remove block (⌫)">
          Remove block
        </button>
      </div>
    </Frame>
  );
}
