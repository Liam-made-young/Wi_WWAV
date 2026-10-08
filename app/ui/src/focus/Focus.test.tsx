// The Focus layout, on Learn's frame and the fake core: Focus alone at
// launch, every tool a key away, Esc back, one interrupt line, and a view
// that registers appearing everywhere.

import { act, createRef } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import type { ScreenStatus } from '../shell/StatusBar';
import { type HeatHandle, HeatView, type Layer } from '../heat/HeatView';
import { $, $$, button, click, escape, mountWith, press, type Rig, settle } from '../heat/testkit';
import { EdgeReveal } from './EdgeReveal';
import { focusMemory } from './fake';
import { KeyMap } from './KeyMap';
import { registerView } from './registry';

let rig: Rig | undefined;
const undo: (() => void)[] = [];
afterEach(() => {
  rig?.unmount();
  rig = undefined;
  while (undo.length) undo.pop()!();
  localStorage.clear();
});

interface Seen {
  layer: Layer;
  view: string;
  status: ScreenStatus;
}

async function mountFocus(options: Parameters<typeof mountWith>[1] = {}): Promise<{ rig: Rig; seen: Seen }> {
  const view = createRef<HeatHandle>();
  const seen: Seen = { layer: 'focus', view: '', status: { count: null, act: null } };
  const r: Rig = await mountWith(
    <HeatView
      ref={view}
      open={null}
      layout="focus"
      onStatus={(s) => (seen.status = s)}
      onSettings={() => {}}
      onLayer={(layer, v) => {
        seen.layer = layer;
        seen.view = v;
      }}
    />,
    options,
  );
  // The kit's keys and Esc go through the rig's handle: this view's own.
  rig = { ...r, view };
  return { rig, seen };
}

const text = (r: Rig, selector: string) => $(r, selector)?.textContent ?? null;

describe('Focus', () => {
  it('shows only the Now task, its timer and at most one interrupt line', async () => {
    const { rig, seen } = await mountFocus();
    expect(seen.layer).toBe('focus');
    expect(text(rig, '.focus-title')).toBe('Mix the second verse');
    expect(text(rig, '.focus-meta')).toContain('Saturday 6:00 PM');
    expect($(rig, '.focus-digits')!.textContent).toBe('25:00');
    expect($$(rig, '.focus-interrupt')).toHaveLength(1);
    // None of the Classic layout's furniture, and no tool's header.
    for (const gone of ['.heat-toolbar', '.heat-tabs', '.heat-right', '.tool-head']) expect($(rig, gone)).toBeNull();
    expect($(rig, '.heat-main')!.hasAttribute('inert')).toBe(true);
    expect($(rig, '.heat-sidebar')!.hasAttribute('inert')).toBe(true);
    expect(seen.status).toMatchObject({ count: null, act: null });
  });

  it('opens a tool on its number key, full width under its name, and Esc returns to Focus', async () => {
    const { rig, seen } = await mountFocus();
    expect(await press(rig, '2')).toBe(true);
    expect(seen).toMatchObject({ layer: 'tool', view: 'tasks' });
    expect(text(rig, '.tool-name')).toBe('Tasks');
    expect(button(rig, 'Esc to focus')).toBeTruthy();
    expect($(rig, '.focus')).toBeNull();
    expect($(rig, '.heat')!.dataset.sidebar).toBe('true');
    await press(rig, '4');
    expect(text(rig, '.tool-name')).toBe('Grades');
    expect($(rig, '.heat')!.dataset.sidebar).toBe('false');
    await escape(rig);
    expect(seen.layer).toBe('focus');
    expect($(rig, '.focus-title')).toBeTruthy();
  });

  it('closes an open sheet first, and then Esc returns to Focus', async () => {
    const { rig, seen } = await mountFocus();
    await press(rig, '2');
    await press(rig, 'n');
    expect($(rig, '.heat-sheet')).toBeTruthy();
    await escape(rig);
    expect($(rig, '.heat-sheet')).toBeNull();
    expect(seen.layer).toBe('tool');
    await escape(rig);
    expect(seen.layer).toBe('focus');
  });

  it('starts the timer on the Now task with F, and never acts on a tool that is out of sight', async () => {
    const { rig } = await mountFocus();
    await press(rig, 'f');
    const start = rig.calls.find((c) => c.cmd === 'heat.focus.start');
    expect(start?.args).toMatchObject({ taskId: rig.seeded.currentTaskId, length: 25 });
    expect(button($(rig, '.focus'), 'Pause')).toBeTruthy();
    // Backspace deletes the selection in Tasks; in Focus it is nobody's key.
    expect(await press(rig, 'Backspace')).toBe(false);
    expect(rig.calls.some((c) => c.cmd === 'heat.delete')).toBe(false);
  });

  it('completes the task with Done and slides in the next', async () => {
    const { rig } = await mountFocus();
    rig.fake.store.focusSession.set('s', { id: 's', taskId: rig.seeded.currentTaskId, startedAt: 1, endedAt: 2, focusMin: 25, interruptions: 0 });
    await act(async () => rig.fake.emit(['focusSession']));
    await settle();
    await click(button($(rig, '.focus'), 'Done'));
    await settle();
    expect(rig.calls.some((c) => c.cmd === 'heat.done')).toBe(true);
    expect(text(rig, '.focus-title')).not.toBe('Mix the second verse');
    expect($(rig, '.focus-title')).toBeTruthy();
  });

  it('shows a due date Claude moved as one line, opens the task from it, and lets it be dismissed', async () => {
    const { rig, seen } = await mountFocus();
    const quiz = rig.fake.store.task.get('t-quiz4')!;
    await act(async () => {
      rig.fake.write('edit task', ['task'], () => rig.fake.store.task.set(quiz.id, { ...quiz, due: quiz.due! - 3_600_000 }), {
        actor: 'claude',
        tool: 'update_task',
      });
    });
    await settle();
    const lines = $$(rig, '.focus-interrupt');
    expect(lines).toHaveLength(1);
    expect(lines[0].textContent).toContain('Due date moved: Grammar quiz 4 is now due Tomorrow 10:59 PM.');
    expect(lines[0].getAttribute('role')).toBe('status');
    expect(lines[0].getAttribute('aria-label')).toBe('Interrupt');
    // No panel, no dialog, no count.
    expect($(rig, '[role="dialog"]')).toBeNull();
    await click(button(lines[0], 'Dismiss'));
    await settle();
    expect(text(rig, '.focus-interrupt')).not.toContain('Due date moved');
    expect(focusMemory(rig.fake).dismissed).toHaveProperty(`due:${quiz.id}:${quiz.due! - 3_600_000}`);
    expect(seen.layer).toBe('focus');
  });

  it('shows the fix when entropy is high, and Plan my day answers it', async () => {
    const { rig, seen } = await mountFocus();
    await act(async () => {
      rig.fake.state.currentTaskId = null;
      for (const b of [...rig.fake.store.timeBlock.keys()]) rig.fake.store.timeBlock.delete(b);
      rig.fake.emit(['task']);
    });
    await settle();
    expect(text(rig, '.focus-title')).toMatch(/tasks this week have no plan\.$/);
    expect(text(rig, '.focus-meta')).toBe('Plan them?');
    expect($(rig, '.focus')!.dataset.entropy).toBe('high');
    // Today has a Plan my day of its own, mounted out of sight: this is Focus's.
    await click(button($(rig, '.focus'), 'Plan my day'));
    await settle();
    expect(rig.calls.some((c) => c.cmd === 'heat.focus.snooze')).toBe(true);
    expect(rig.calls.some((c) => c.cmd === 'heat.plan.make')).toBe(true);
    expect(seen).toMatchObject({ layer: 'tool', view: 'today' });
  });

  it('says All clear when every loop is closed', async () => {
    const { rig } = await mountFocus({ empty: true });
    expect(text(rig, '.focus-title')).toBe('All clear');
    expect($(rig, '.focus')!.dataset.entropy).toBe('clear');
    expect($(rig, '.focus-interrupt')).toBeNull();
    expect($(rig, '.focus-timer')).toBeNull();
  });

  it('shows the hint under the task at first, and not after the first week', async () => {
    const first = await mountFocus();
    expect(text(first.rig, '.focus-hint')).toMatch(/K for anything · 1–9 for tools · hold .+ for the map/);
    first.rig.unmount();
    localStorage.setItem('wi.focusHint', JSON.stringify({ first: 0, launches: 99 }));
    const later = await mountFocus();
    expect($(later.rig, '.focus-hint')).toBeNull();
  });

  it('shows only the first step of a task that has steps', async () => {
    const { rig } = await mountFocus();
    const id = rig.seeded.currentTaskId;
    await act(async () => {
      rig.fake.store.task.set(id, { ...rig.fake.store.task.get(id)!, notes: 'Plan\n- [x] Bounce stems\n- [ ] Ride the vocal\n- [ ] Print' });
      rig.fake.emit(['task']);
    });
    await settle();
    expect(text(rig, '.focus-step')).toBe('First Ride the vocal');
    expect($$(rig, '.focus-step')).toHaveLength(1);
  });
});

describe('a view that registers', () => {
  const Notes = () => <p className="notes-body">Notes are here.</p>;

  it('is on its number key, in the edge reveal and on the map, with nothing else edited', async () => {
    const { rig, seen } = await mountFocus();
    await act(async () => {
      undo.push(registerView({ id: 'notes', title: 'Notes', component: Notes, plus: 'New note' }));
    });
    await settle();
    // The Database and the Wiki are on 7 and 8, so the next view to register takes 9.
    expect(await press(rig, '9')).toBe(true);
    expect(seen).toMatchObject({ layer: 'tool', view: 'notes' });
    expect(text(rig, '.tool-name')).toBe('Notes');
    expect(text(rig, '.heat-tabpanel[data-current="true"] .notes-body')).toBe('Notes are here.');

    const summoned: string[] = [];
    rig.rerender(
      <>
        <EdgeReveal open current={null} onOpen={() => {}} onSummon={(v) => summoned.push(v.id)} onFocus={() => summoned.push('focus')} />
        <KeyMap shown />
      </>,
    );
    const tools = $(rig, 'nav[aria-label="Tools"]')!;
    expect($$(tools, '.edge-row').map((row) => row.textContent)).toEqual([
      'FocusEsc',
      'Today1',
      'Tasks2',
      'Calendar3',
      'Grades4',
      'Habits5',
      'Mail6',
      'Database7',
      'Wiki8',
      'Notes9',
    ]);
    await click(button(tools, /^Notes/));
    await click(button(tools, /^Focus/));
    expect(summoned).toEqual(['notes', 'focus']);
    const map = $(rig, '[aria-label="Map of tools and shortcuts"]')!;
    expect(map.textContent).toContain('9Notes');
  });
});

describe('the Classic layout', () => {
  it('still draws the toolbar, the tabs and the right column, with a registered view as one more tab', async () => {
    undo.push(registerView({ id: 'notes', title: 'Notes', component: () => null }));
    const view = createRef<HeatHandle>();
    rig = await mountWith(<HeatView ref={view} open={null} onStatus={() => {}} onSettings={() => {}} />);
    expect($(rig, '.heat-toolbar')).toBeTruthy();
    expect($(rig, '.heat-right')).toBeTruthy();
    expect($(rig, '.focus')).toBeNull();
    expect($$(rig, '.heat-tabs [role="tab"]').map((t) => t.textContent)).toEqual([
      'Today',
      'Tasks',
      'Calendar',
      'Grades',
      'Habits',
      'Mail',
      'Database',
      'Wiki',
      'Notes',
    ]);
  });
});
