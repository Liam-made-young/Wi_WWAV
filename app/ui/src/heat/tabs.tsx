// The six tabs of Heat (docs/SPEC.md 3.3), as a plain map from a tab's id to
// its component. Heat's frame mounts each one inside HeatView and keeps them
// all mounted, so a tab's scroll, selection and half-typed text stay where
// you left them. A tab reads the snapshot with useHeat() and tells the frame
// about itself with the hooks in ./frame: useTabActs for its "+" and its one
// secondary act, useTabKeys for the keys it answers, useSidebarSlot for its
// sidebar sections.

import type { ComponentType } from 'react';
import { Calendar } from './calendar/Calendar';
import { Database } from './database/Database';
import type { TabId } from './frame';
import { Grades } from './grades/Grades';
import { Habits } from './habits/Habits';
import { Mail } from './mail/Mail';
import { Tasks } from './tasks/Tasks';
import { Today } from './today/Today';
import { Wiki } from './wiki/Wiki';

export const HEAT_TABS: Record<TabId, ComponentType> = {
  today: Today,
  tasks: Tasks,
  calendar: Calendar,
  grades: Grades,
  habits: Habits,
  mail: Mail,
  database: Database,
  wiki: Wiki,
};
