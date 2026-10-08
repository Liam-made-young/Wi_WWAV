// The month grid (docs/SPEC.md 3.1, 3.7): from Sunday, six weeks, 3 pills a
// cell with a 3 px heat border and "N more", today in a red circle. A day is
// selected by a click (or ⇧← ⇧→ ⇧↑ ⇧↓); a task dragged onto a day is
// scheduled for it. "N more" opens that day.

import { type DragEvent } from 'react';
import { longDay } from '../../shared/time/format';
import { type DayKey, keyParts } from '../../shared/time/zone';
import { useActions } from '../actions';
import { CommitmentDots } from '../commitments/CommitmentLayer';
import { copy } from '../fmt';
import { useSelection } from '../frame';
import { useHeat } from '../store';
import { carriesTask, draggedTask } from '../ui';
import type { DueItem } from './items';
import { Pill } from './Pill';

const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const PILLS = 3;

interface Props {
  days: DayKey[];
  anchor: DayKey;
  selected: DayKey;
  due: Map<DayKey, DueItem[]>;
  onPick(day: DayKey): void;
  onMore(day: DayKey): void;
}

export function MonthView({ days, anchor, selected, due, onPick, onMore }: Props) {
  const { date: today } = useHeat();
  const { taskId, selectTask } = useSelection();
  const actions = useActions();
  const month = keyParts(anchor).month;

  const over = (e: DragEvent) => {
    if (carriesTask(e)) e.preventDefault();
  };
  const drop = (e: DragEvent, day: DayKey) => {
    const id = draggedTask(e);
    if (!id) return;
    e.preventDefault();
    void actions.schedule(id, day);
  };

  return (
    <div className="heat-month" role="grid" aria-label="Month">
      <div role="row" className="heat-month-heads">
        {WEEKDAYS.map((d) => (
          <div key={d} role="columnheader" className="heat-month-head" data-text="secondary">
            {d}
          </div>
        ))}
      </div>
      <div className="heat-month-grid">
        {days.map((day) => {
          const items = due.get(day) ?? [];
          return (
            <div
              key={day}
              role="gridcell"
              className="heat-cell"
              data-day={day}
              data-today={day === today}
              data-selected={day === selected}
              data-out={keyParts(day).month !== month}
              aria-selected={day === selected}
              onClick={() => onPick(day)}
              onDragOver={over}
              onDrop={(e) => drop(e, day)}
            >
              <button
                type="button"
                className="heat-cell-day"
                data-dense
                aria-label={longDay(day)}
                onClick={() => onPick(day)}
              >
                {keyParts(day).day}
              </button>
              <CommitmentDots day={day} />
              {items.slice(0, PILLS).map((i) => (
                <Pill
                  key={`${i.taskId}-${i.day}`}
                  item={i}
                  selected={taskId === i.taskId}
                  onSelect={() => selectTask(i.taskId)}
                />
              ))}
              {items.length > PILLS && (
                <button
                  type="button"
                  className="heat-more"
                  data-dense
                  onClick={(e) => {
                    e.stopPropagation();
                    onMore(day);
                  }}
                >
                  {copy.calendar.more(items.length - PILLS)}
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
