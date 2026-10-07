// The four rooms, in the switcher's order (docs/SPEC.md 2.1), and the
// register each one's inside wears (8.2). The case around them is the same
// in every room.

import type { Register } from '../shared/stems/mix';

export type RoomId = 'heat' | 'space' | 'console' | 'unquantized';

export const ROOMS: readonly RoomId[] = ['heat', 'space', 'console', 'unquantized'];

export const ROOM_NAMES: Record<RoomId, string> = {
  heat: 'Heat',
  space: 'Space',
  console: 'Console',
  unquantized: 'Unquantized',
};

export const REGISTERS: Record<RoomId, Register> = {
  heat: 'desk',
  space: 'night',
  console: 'desk',
  unquantized: 'interior',
};
