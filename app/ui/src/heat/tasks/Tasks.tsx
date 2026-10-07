// Tasks (docs/SPEC.md 3.6): Heat's List view with the gaps filled. The
// sidebar holds the lists and the groups below them; the main view is the
// striped list, with a milestone timeline above it when a space of
// milestones is chosen. "+" adds a task; "Triage inbox" (⇧Return, only while
// the inbox has items) shows the captures waiting to become tasks, notes,
// projects or uploads.
//
// Keys: ↑ ↓ move the selection, Return edits, ⌘I opens Get Info, ⌘↩ marks
// done and ⌫ deletes, with Undo; ⌘F filters the list.

import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useActions } from '../actions';
import { copy, plural } from '../fmt';
import { useFrame, useSelection, useSheets, useSidebarSlot, useSpaceFilter, useTabActs, useTabKeys } from '../frame';
import type { Id } from '../client';
import { useHeat } from '../store';
import { Inbox } from './Inbox';
import { matchesFilter, nest } from './lists';
import { TaskTable } from './TaskTable';
import { Timeline } from './Timeline';
import { TasksSidebar } from './TasksSidebar';
import { ALL_OPEN, LISTS, type TasksView } from './view';
import './tasks.css';

export function Tasks() {
  const { snap, idx } = useHeat();
  const { spaceId } = useSpaceFilter();
  const { taskId, selectTask } = useSelection();
  const { newTask } = useSheets();
  const { focusInfo } = useFrame();
  const slot = useSidebarSlot();
  const actions = useActions();

  const [view, setView] = useState<TasksView>(ALL_OPEN);
  const [collapsed, setCollapsed] = useState<ReadonlySet<Id>>(new Set());
  const [filter, setFilter] = useState('');
  const [cursor, setCursor] = useState<Id | null>(null);
  const filterField = useRef<HTMLInputElement>(null);
  const inSpace = (id: Id) => !spaceId || idx.task.get(id)?.spaceId === spaceId;

  const inbox = snap?.derived.lists.inbox ?? [];
  const inInbox = view.kind === 'list' && view.list === 'inbox';

  // The ids to show, in the order the snapshot gave them: a list, or the open tasks of a group, then the filter.
  const ids = useMemo(() => {
    if (!snap) return [];
    if (inInbox) return inbox;
    const base = view.kind === 'list' ? snap.derived.lists[view.list] : snap.derived.lists.allOpen;
    return base.filter(inSpace).filter((id) => {
      const t = idx.task.get(id);
      if (!t) return false;
      if (view.kind === 'group') {
        const field = { course: t.courseId, milestone: t.milestoneId, project: t.projectId, area: t.group }[view.by];
        if (field !== view.id) return false;
      }
      if (!filter.trim()) return true;
      const g = t.courseId
        ? idx.course.get(t.courseId)?.code
        : (t.group ?? idx.milestone.get(t.milestoneId ?? '')?.title);
      return matchesFilter(filter, t.title, t.type, g);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snap, idx, view, filter, spaceId]);

  const rows = useMemo(
    () => (inInbox ? [] : nest(ids, (id) => idx.task.get(id)?.parentTaskId, collapsed)),
    [ids, idx, collapsed, inInbox],
  );
  const order = inInbox ? ids : rows.map((r) => r.id);

  // Something opened Heat on a task that this list doesn't hold (the strip, ⌘K): show the list that does.
  const here = useRef(order);
  here.current = order;
  useEffect(() => {
    if (!taskId || !snap || here.current.includes(taskId)) return;
    const t = idx.task.get(taskId);
    if (!t) return;
    setFilter('');
    setView(t.done ? { kind: 'list', list: 'done' } : ALL_OPEN);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [taskId, snap !== null]);

  // A selected row stays in view as ↑ ↓ walk the list.
  useEffect(() => {
    if (taskId) document.getElementById(`heat-row-t-${taskId}`)?.scrollIntoView?.({ block: 'nearest' });
  }, [taskId, rows]);

  const showInbox = () => {
    setView({ kind: 'list', list: 'inbox' });
    setCursor(inbox[0] ?? null);
  };
  useTabActs({
    plus: { run: () => newTask({ spaceId: spaceId ?? undefined }) },
    secondary: { run: showInbox, hidden: inbox.length === 0 },
    count: snap ? (inInbox ? `${inbox.length} in inbox` : plural(order.length, 'task')) : null,
  });
  useTabKeys({
    move(by) {
      if (order.length === 0) return;
      const current = inInbox ? cursor : taskId;
      const at = current ? order.indexOf(current) : -1;
      const next = order[at < 0 ? (by === 1 ? 0 : order.length - 1) : Math.min(order.length - 1, Math.max(0, at + by))];
      if (inInbox) setCursor(next);
      else selectTask(next);
    },
    edit() {
      if (!inInbox) focusInfo();
      else if (cursor) void actions.triage(cursor, 'task');
    },
    escape() {
      if (document.activeElement === filterField.current) {
        filterField.current?.blur();
        return true;
      }
      return false;
    },
    filter: () => filterField.current?.focus(),
  });

  if (!snap) return <div className="heat-tasks" />;

  const space = snap.records.space.find((s) => s.id === spaceId) ?? null;
  const title = view.kind === 'group' ? view.label : (LISTS.find((l) => l.list === view.list)?.label ?? '');
  const anyTasks = snap.records.task.some((t) => !spaceId || t.spaceId === spaceId);
  const timeline = space?.groupKind === 'milestone' ? space.id : null;
  const groupLabel = space?.groupLabel ?? 'Group';

  return (
    <div className="heat-tasks">
      {slot &&
        createPortal(
          <TasksSidebar
            view={view}
            onView={(v) => {
              setView(v);
              setFilter('');
              if (v.kind === 'list' && v.list === 'inbox') setCursor(inbox[0] ?? null);
            }}
          />,
          slot,
        )}
      <header className="heat-tasks-head">
        <div>
          <h1 className="heat-heading">{title}</h1>
          <p className="heat-subtitle" data-text="secondary">
            {inInbox ? `${inbox.length} in inbox` : plural(order.length, 'task')}
          </p>
        </div>
        <div className="heat-tasks-tools">
          {inbox.length > 0 && !inInbox && (
            <button type="button" className="gel" onClick={showInbox} title="Triage inbox (⇧Return)">
              Triage inbox
            </button>
          )}
          {!inInbox && (
            <input
              ref={filterField}
              type="search"
              className="heat-filter"
              aria-label="Filter the list"
              placeholder="Filter"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            />
          )}
        </div>
      </header>
      {timeline && (
        <Timeline
          spaceId={timeline}
          picked={view.kind === 'group' && view.by === 'milestone' ? view.id : null}
          onPick={(id) => setView({ kind: 'group', by: 'milestone', id, label: idx.milestone.get(id)?.title ?? '' })}
        />
      )}
      <div className="heat-tasks-body">
        {inInbox ? (
          <Inbox ids={inbox} cursor={cursor} onCursor={setCursor} />
        ) : order.length > 0 ? (
          <TaskTable
            rows={rows}
            collapsed={collapsed}
            selected={taskId}
            groupLabel={groupLabel}
            onToggle={(id) =>
              setCollapsed((was) => {
                const next = new Set(was);
                if (!next.delete(id)) next.add(id);
                return next;
              })
            }
            onSelect={selectTask}
            onEdit={(id) => {
              selectTask(id);
              requestAnimationFrame(() => focusInfo());
            }}
          />
        ) : (
          <p className="heat-empty">{anyTasks ? copy.status.empty : copy.tasks.empty(space?.name ?? null)}</p>
        )}
      </div>
    </div>
  );
}
