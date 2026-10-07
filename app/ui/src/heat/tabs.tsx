// The six tabs of Heat (docs/SPEC.md 3.3), as a plain map from a tab's id to
// its component. Heat's frame mounts each one inside HeatView and keeps them
// all mounted, so a tab's scroll, selection and half-typed text stay where
// you left them. A tab reads the snapshot with useHeat() and tells the frame
// about itself with the hooks in ./frame: useTabActs for its "+" and its one
// secondary act, useTabKeys for the keys it answers, useSidebarSlot for its
// sidebar sections.
//
// Grades, Habits and Mail are stubs here until their screens are built: each
// shows what the tab will hold and nothing else, so the frame and its tests
// run with all six.

import type { ComponentType } from 'react';
import { Calendar } from './calendar/Calendar';
import { copy } from './fmt';
import type { TabId } from './frame';
import { Tasks } from './tasks/Tasks';
import { Today } from './today/Today';

function Stub({ title, line }: { title: string; line: string }) {
  return (
    <div className="heat-stub">
      <h1 className="heat-heading">{title}</h1>
      <p className="why" data-text="secondary">
        {line}
      </p>
    </div>
  );
}

const GradesStub = () => (
  <Stub title="Grades" line="Grades isn’t built yet. Each course will show its percentage and letter here." />
);
const HabitsStub = () => <Stub title="Habits" line={`Habits isn’t built yet. ${copy.habits.advice}`} />;
const MailStub = () => (
  <Stub title="Mail" line="Mail isn’t built yet. It will list the school threads Claude has recorded." />
);

export const HEAT_TABS: Record<TabId, ComponentType> = {
  today: Today,
  tasks: Tasks,
  calendar: Calendar,
  grades: GradesStub,
  habits: HabitsStub,
  mail: MailStub,
};
