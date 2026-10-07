// Tasks' sidebar sections (docs/SPEC.md 3.6), portalled under the spaces
// filter: Inbox, All open, Hot, Due this week, Scheduled, Someday, Done;
// then Projects with their milestones, or Courses, or Areas; then "Your
// average time". Counts come from the snapshot's lists.

import { type ReactNode } from 'react';
import { useActions } from '../actions';
import { copy, formatMinutes } from '../fmt';
import { carriesTask, draggedTask } from '../ui';
import { useHeat } from '../store';
import { useSpaceFilter } from '../frame';
import { LISTS, sameView, type TasksView } from './view';

interface Props {
  view: TasksView;
  onView(view: TasksView): void;
}

function Row({
  label,
  count,
  on,
  onPick,
  onDropTask,
  depth = 0,
}: {
  label: string;
  count?: number;
  on: boolean;
  onPick(): void;
  /** A task dropped on the row: a group row links it. */
  onDropTask?(taskId: string): void;
  depth?: number;
}) {
  return (
    <button
      type="button"
      className="heat-side-row"
      data-dense
      aria-current={on ? 'true' : undefined}
      style={depth ? { paddingLeft: 12 + depth * 14 } : undefined}
      onClick={onPick}
      onDragOver={(e) => {
        if (onDropTask && carriesTask(e)) e.preventDefault();
      }}
      onDrop={(e) => {
        const id = draggedTask(e);
        if (onDropTask && id) {
          e.preventDefault();
          onDropTask(id);
        }
      }}
    >
      <span className="heat-side-name">{label}</span>
      {count !== undefined && (
        <span className="heat-side-count" data-text="secondary" aria-label={`${count} open`}>
          {count}
        </span>
      )}
    </button>
  );
}

export function TasksSidebar({ view, onView }: Props) {
  const { snap, idx } = useHeat();
  const { spaceId } = useSpaceFilter();
  const actions = useActions();
  if (!snap) return null;
  const lists = snap.derived.lists;
  const inSpace = (taskId: string) => !spaceId || idx.task.get(taskId)?.spaceId === spaceId;
  const open = lists.allOpen.filter(inSpace);
  const tasks = snap.records.task.filter((t) => !spaceId || t.spaceId === spaceId);
  const countBy = (pick: (t: (typeof tasks)[number]) => unknown, id: unknown) =>
    open.filter((tid) => pick(idx.task.get(tid)!) === id).length;

  const spaces = snap.records.space.filter((s) => !spaceId || s.id === spaceId);
  const kinds = new Set(spaces.map((s) => s.groupKind));

  const courses = snap.records.course
    .filter((c) => tasks.some((t) => t.courseId === c.id))
    .sort((a, b) => a.code.localeCompare(b.code));
  const milestoneSpaces = new Set(spaces.filter((s) => s.groupKind === 'milestone').map((s) => s.id));
  const projects = snap.records.project.filter((p) => milestoneSpaces.has(p.spaceId) && p.status !== 'archived');
  const areas = [...new Set(tasks.flatMap((t) => (t.group ? [t.group] : [])))].sort((a, b) => a.localeCompare(b));
  const averages = snap.derived.averages.filter((a) => !spaceId || a.space === spaceId);

  const group = (by: 'course' | 'milestone' | 'project' | 'area', id: string, label: string, depth = 0): ReactNode => {
    const next: TasksView = { kind: 'group', by, id, label };
    const field =
      by === 'course' ? 'courseId' : by === 'milestone' ? 'milestoneId' : by === 'project' ? 'projectId' : 'group';
    return (
      <Row
        key={`${by}-${id}`}
        label={label}
        count={countBy((t) => (t as unknown as Record<string, unknown>)[field], id)}
        on={sameView(view, next)}
        onPick={() => onView(next)}
        onDropTask={by === 'area' ? undefined : (taskId) => void actions.linkTo(taskId, by, id, label)}
        depth={depth}
      />
    );
  };

  return (
    <>
      <h2 className="heat-side-heading" data-text="secondary">
        {copy.tasks.library}
      </h2>
      {LISTS.map(({ list, label }) => (
        <Row
          key={list}
          label={label}
          count={list === 'inbox' ? lists.inbox.length : lists[list].filter(inSpace).length}
          on={view.kind === 'list' && view.list === list}
          onPick={() => onView({ kind: 'list', list })}
        />
      ))}

      {kinds.has('course') && courses.length > 0 && (
        <>
          <h2 className="heat-side-heading" data-text="secondary">
            Courses
          </h2>
          {courses.map((c) => group('course', c.id, c.code))}
        </>
      )}
      {kinds.has('milestone') && projects.length > 0 && (
        <>
          <h2 className="heat-side-heading" data-text="secondary">
            Projects
          </h2>
          {projects.map((p) => [
            group('project', p.id, p.title),
            ...snap.records.milestone
              .filter((m) => m.projectId === p.id)
              .sort((a, b) => a.order - b.order)
              .map((m) => group('milestone', m.id, m.title, 1)),
          ])}
        </>
      )}
      {kinds.has('free') && areas.length > 0 && (
        <>
          <h2 className="heat-side-heading" data-text="secondary">
            Areas
          </h2>
          {areas.map((a) => group('area', a, a))}
        </>
      )}

      {averages.length > 0 && (
        <>
          <h2 className="heat-side-heading" data-text="secondary">
            {copy.tasks.averageTime}
          </h2>
          <ul className="heat-averages">
            {averages.map((a) => (
              <li key={`${a.space}-${a.type}`} className="heat-average">
                {copy.averageTime(a.type, formatMinutes(a.minutes), a.count)}
              </li>
            ))}
          </ul>
        </>
      )}
    </>
  );
}
