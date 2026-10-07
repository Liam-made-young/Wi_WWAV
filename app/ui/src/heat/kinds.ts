// The record kinds Heat keeps in the core's journaled store (docs/SPEC.md
// 3.15, docs/COMMANDS.md records.*). HeatState is one record, id "heat".

export const KINDS = {
  task: 'task',
  occurrence: 'taskOccurrence',
  session: 'focusSession',
  course: 'course',
  milestone: 'milestone',
  state: 'heatState',
  capture: 'capture',
} as const;

export const HEAT_STATE_ID = 'heat';
