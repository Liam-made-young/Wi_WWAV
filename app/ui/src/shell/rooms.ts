// The three views, in the switcher's order (docs/SPEC.md 2.1): Heat, the
// profile view; Space, the social view; the Console, the creation view. Each
// wears one register inside (7.2): the desk or the night. The case around
// them is the same in every view. (The code and the core's commands still
// call a view a room: the wire names are `room`, and renaming them is
// recorded in docs/DECISIONS.md.)

import type { Register } from '../shared/stems/mix';

export type RoomId = 'heat' | 'space' | 'console';

export const ROOMS: readonly RoomId[] = ['heat', 'space', 'console'];

export const ROOM_NAMES: Record<RoomId, string> = {
  heat: 'Heat',
  space: 'Space',
  console: 'Console',
};

export const REGISTERS: Record<RoomId, Register> = {
  heat: 'desk',
  space: 'night',
  console: 'desk',
};
