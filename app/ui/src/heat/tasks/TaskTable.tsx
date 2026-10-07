// The task list (docs/SPEC.md 3.6): 26 px striped rows with a checkbox,
// Heat, Task, group, Type, Due, Diff, Time and When. Subtasks indent under a
// disclosure triangle, and ↻ marks a recurring task. The rows come in the
// order the snapshot gave; a row can be dragged onto Today's column, or a
// Calendar day (a tab opens when a drag rests on it).

import { shortMonthDay } from '../../shared/time/format';
import { dayKey } from '../../shared/time/zone';
import { useActions } from '../actions';
import type { Id } from '../client';
import { dayText, dueText, effectiveDue, formatMinutes } from '../fmt';
import { useHeat } from '../store';
import { dragTask, Heat } from '../ui';
import { Tick } from '../today/PlanList';
import { groupOf, type RowSpec } from './lists';

interface Props {
  rows: RowSpec[];
  collapsed: ReadonlySet<Id>;
  selected: Id | null;
  groupLabel: string;
  onToggle(id: Id): void;
  onSelect(id: Id): void;
  onEdit(id: Id): void;
}

const COLUMNS = ['', 'Heat', 'Task', 'Group', 'Type', 'Due', 'Diff', 'Time', 'When'] as const;

export function TaskTable({ rows, collapsed, selected, groupLabel, onToggle, onSelect, onEdit }: Props) {
  const { snap, idx, tz, date, now } = useHeat();
  const actions = useActions();
  if (!snap) return null;

  return (
    <div
      role="grid"
      aria-label="Tasks"
      className="heat-table"
      aria-activedescendant={selected ? `heat-row-t-${selected}` : undefined}
    >
      <div role="row" className="heat-trow heat-thead">
        {COLUMNS.map((c, i) => (
          <div
            key={i}
            role="columnheader"
            className="heat-th"
            data-text="secondary"
            aria-label={c === 'Group' ? groupLabel : c || 'Done'}
          >
            {c === 'Group' ? groupLabel : c}
          </div>
        ))}
      </div>
      {rows.map(({ id, depth, children }) => {
        const t = idx.task.get(id);
        const d = snap.derived.tasks[id];
        if (!t || !d) return null;
        const due = effectiveDue(t, d, tz);
        const dueLabel = t.done
          ? t.doneAt
            ? `Done ${shortMonthDay(dayKey(t.doneAt, tz))}`
            : 'Done'
          : due !== null
            ? dueText(due, now(), tz)
            : '';
        const time = t.done && d.actualMin > 0 ? formatMinutes(d.actualMin) : formatMinutes(d.estimate.min);
        const on = t.rrule ? (idx.ticked.get(id)?.has(date) ?? false) : t.done;
        return (
          <div
            key={id}
            id={`heat-row-t-${id}`}
            role="row"
            className="heat-trow"
            data-dense
            data-done={t.done}
            data-depth={depth}
            aria-selected={selected === id}
            aria-level={depth + 1}
            {...dragTask(id)}
            onClick={() => onSelect(id)}
            onDoubleClick={() => onEdit(id)}
          >
            <div role="gridcell" className="heat-td heat-td-tick">
              <Tick on={on} label={`${t.title}, done`} onToggle={() => void actions.toggleDone(id)} />
            </div>
            <div role="gridcell" className="heat-td">
              <Heat level={d.heat.level} v={d.heat.v} />
            </div>
            <div role="gridcell" className="heat-td heat-td-title" style={{ paddingLeft: depth * 18 }}>
              {children > 0 ? (
                <button
                  type="button"
                  className="heat-disclose"
                  data-dense
                  aria-expanded={!collapsed.has(id)}
                  aria-label={`Subtasks of ${t.title}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onToggle(id);
                  }}
                >
                  {collapsed.has(id) ? '▸' : '▾'}
                </button>
              ) : (
                <span className="heat-disclose-gap" />
              )}
              <span className="heat-td-name">{t.title}</span>
              {t.rrule && (
                <span className="heat-repeat" aria-label="repeats" title="Repeats">
                  ↻
                </span>
              )}
              {t.source === 'claude' && (
                <span className="heat-source" data-text="secondary">
                  Claude
                </span>
              )}
            </div>
            <div role="gridcell" className="heat-td heat-td-group">
              {groupOf(t, {
                course: idx.course.get.bind(idx.course),
                milestone: idx.milestone.get.bind(idx.milestone),
                project: idx.project.get.bind(idx.project),
              })}
            </div>
            <div role="gridcell" className="heat-td heat-td-type">
              {t.type}
            </div>
            <div role="gridcell" className="heat-td heat-td-due">
              {dueLabel}
            </div>
            <div role="gridcell" className="heat-td heat-td-diff" title={`Difficulty ${t.difficulty} of 5`}>
              {t.difficulty}
            </div>
            <div
              role="gridcell"
              className="heat-td heat-td-time"
              title={`Estimate ${formatMinutes(d.estimate.min)}${d.actualMin > 0 ? `, spent ${formatMinutes(d.actualMin)}` : ''}`}
            >
              {time}
            </div>
            <div role="gridcell" className="heat-td heat-td-when">
              {t.scheduledDate ? dayText(t.scheduledDate, date) : ''}
            </div>
          </div>
        );
      })}
    </div>
  );
}
