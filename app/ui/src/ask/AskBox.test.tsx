// The prompt box (ask/AskBox.tsx). What a fail looks like: typing waits on
// Claude to show a result; Return opens something instead of asking, or asks
// when a result was picked; a change is made without the click, or what
// leaves this Mac goes with the rest; Claude missing breaks search.

import { act, createRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import { CoreError } from '../bridge';
import { AskBox, type AskHandle } from './AskBox';
import type { Answer } from './client';
import { setScreen } from './context';
import type { Target } from './nav';
import { standIn, type StandIn } from './testkit';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const FOUND = [
  { kind: 'task', id: 't1', title: 'Kanji quiz 4', hint: 'JPN 101 · Due 2026-10-09', word: 'Task', table: 'task' },
  { kind: 'note', id: 'n1', title: 'Kanji radicals', hint: 'Water, fire, tree.', word: 'Note', table: 'note' },
  { kind: 'dbRow', id: 'r1', title: 'Remembering the Kanji', hint: 'Reading list', word: 'Row', table: 'tbl1' },
];

const ANSWER: Answer = {
  id: 'run1',
  answer: 'Staged moving [Kanji quiz 4](learn://task/t1) to Thursday.\n\nWikipedia: [Kanji](wiki://Kanji)',
  wiki: ['Kanji'],
  opens: [],
  change: { summary: '1 task moved', lines: ['Kanji quiz 4: Due 2026-10-09 23:59 → 2026-10-08 23:59'], count: 1 },
  outward: [{ index: 0, line: 'Make the task ‘Kanji quiz 4’ public: anyone who opens your sun can see it.' }],
  tools: ['list_rows', 'update_rows'],
  seconds: 4.2,
};

interface Rig {
  host: HTMLElement;
  root: Root;
  box: AskHandle;
  core: StandIn;
  tasks: string[];
  opened: Target[];
  closed: number;
  said: string[];
  ran: string[];
}

let rig: Rig | null = null;

async function settle(times = 4) {
  for (let i = 0; i < times; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
  }
}

async function mount(answers: Parameters<typeof standIn>[0] = {}): Promise<Rig> {
  const core = standIn({
    'ask.status': () => ({ available: true }),
    'ask.search': ({ q }) => ({ results: String(q).toLowerCase().startsWith('kan') ? FOUND : [] }),
    'library.search': () => ({ clips: [] }),
    'ask.send': () => ANSWER,
    'ask.apply': () => ({ applied: 1, failed: [], summary: '1 task moved', undo: 'Undo Claude: 1 task moved', steps: 1 }),
    'ask.applyOutward': () => ({ done: true, line: '', undo: 'Undo make public' }),
    'ask.discard': () => ({}),
    ...answers,
  });
  const host = document.createElement('div');
  document.body.append(host);
  const root = createRoot(host);
  const ref = createRef<AskHandle>();
  const out = { host, root, core, tasks: [], opened: [], closed: 0, said: [], ran: [] } as unknown as Rig;
  Object.defineProperty(out, 'box', { get: () => ref.current! });
  act(() =>
    root.render(
      <AskBox
        ref={ref}
        shown
        actions={[{ label: 'Quick capture', run: () => out.ran.push('capture') }]}
        settings={[{ label: 'Text size', run: () => out.ran.push('text size') }]}
        tasks={[
          { id: 't1', title: 'Kanji quiz 4', level: 'Warm' },
          { id: 't2', title: 'Lab report', level: 'Hot' },
        ]}
        onTask={(id) => out.tasks.push(id)}
        onClip={() => {}}
        onOpen={(target) => out.opened.push(target)}
        onClose={() => void out.closed++}
        onSaid={(text) => out.said.push(text)}
      />,
    ),
  );
  await settle();
  rig = out;
  return out;
}

afterEach(() => {
  if (rig) {
    act(() => rig!.root.unmount());
    rig.host.remove();
    rig = null;
  }
  setScreen('learn', null);
});

async function type(r: Rig, value: string) {
  const el = r.host.querySelector<HTMLInputElement>('.ask-input')!;
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
  await act(async () => {
    set.call(el, value);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise((res) => setTimeout(res, 60));
  });
  await settle();
}

const rows = (r: Rig) => [...r.host.querySelectorAll('.palette-row')].map((el) => el.querySelector('.palette-label')!.textContent);
const press = async (r: Rig, what: 'down' | 'up' | 'return' | 'cmd-return') => {
  await act(async () => {
    if (what === 'down') r.box.move(1);
    else if (what === 'up') r.box.move(-1);
    else r.box.open(what === 'cmd-return');
    await new Promise((res) => setTimeout(res, 0));
  });
  await settle();
};
const click = async (el: Element | null | undefined) => {
  if (!el) throw new Error('Nothing to click.');
  await act(async () => {
    (el as HTMLElement).click();
    await new Promise((res) => setTimeout(res, 0));
  });
  await settle();
};
const button = (r: Rig, name: string) => [...r.host.querySelectorAll('button')].find((b) => b.textContent === name);

describe('the prompt box as a search', () => {
  it('shows matches as you type, from this Mac, before Claude is asked anything', async () => {
    const r = await mount();
    await type(r, 'kan');
    expect(rows(r)).toEqual(['Ask Claude: “kan”', 'Kanji quiz 4', 'Kanji radicals', 'Remembering the Kanji']);
    expect([...r.host.querySelectorAll('.palette-group')].map((g) => g.textContent)).toEqual(['Tasks', 'Notes', 'Database']);
    expect(r.host.querySelectorAll('.palette-hint')[1].textContent).toBe('JPN 101 · Due 2026-10-09 · Warm');
    expect(r.core.asked('ask.send')).toHaveLength(0);
    // An action or a setting is still found by a few of its letters.
    await type(r, 'capt');
    expect(rows(r)).toContain('Quick capture');
    await type(r, 'text');
    expect(rows(r)).toContain('Text size');
  });

  it('opens what ↓ picked on Return, and only then', async () => {
    const r = await mount();
    await type(r, 'kan');
    await press(r, 'down');
    await press(r, 'return');
    expect(r.tasks).toEqual(['t1']);
    expect(r.closed).toBe(1);
    expect(r.core.asked('ask.send')).toHaveLength(0);
    // A note or a row opens as a row of its table; ⌘Return opens a task there too.
    await press(r, 'down');
    await press(r, 'return');
    await press(r, 'down');
    await press(r, 'return');
    await press(r, 'up');
    await press(r, 'up');
    await press(r, 'cmd-return');
    expect(r.opened).toEqual([
      { what: 'row', table: 'note', id: 'n1' },
      { what: 'row', table: 'tbl1', id: 'r1' },
      { what: 'row', table: 'task', id: 't1' },
    ]);
  });

  it('still searches when Claude can’t be asked, and says why', async () => {
    const r = await mount({
      'ask.status': () => ({ available: false, reason: 'Claude Code isn’t installed where Wi_WWAV can find it.' }),
      'ask.send': () => {
        throw new CoreError('claude', 'Claude Code isn’t installed where Wi_WWAV can find it.');
      },
    });
    expect(r.host.querySelector('.palette-empty')!.textContent).toBe('Claude Code isn’t installed where Wi_WWAV can find it.');
    await type(r, 'kan');
    expect(rows(r).slice(0, 2)).toEqual(['Claude can’t be asked right now', 'Kanji quiz 4']);
    await press(r, 'return');
    expect(r.host.querySelector('.ask-error')!.textContent).toBe('Claude Code isn’t installed where Wi_WWAV can find it.');
    await type(r, 'kan');
    expect(rows(r)).toContain('Kanji radicals');
  });
});

describe('asking Claude', () => {
  it('sends what was typed with what is on screen, and draws the answer with its links', async () => {
    const r = await mount();
    setScreen('learn', { tab: 'tasks', selection: [{ table: 'task', id: 't1', title: 'Kanji quiz 4' }] });
    setScreen('wiki', () => ({ title: 'Kanji', section: 'History' }));
    await type(r, 'move this to Thursday');
    await press(r, 'return');
    expect(r.core.asked('ask.send')).toEqual([
      {
        prompt: 'move this to Thursday',
        context: {
          tab: 'tasks',
          selection: [{ table: 'task', id: 't1', title: 'Kanji quiz 4' }],
          wiki: { title: 'Kanji', section: 'History' },
        },
        history: [],
      },
    ]);
    setScreen('wiki', null);
    // The field is empty again and the answer shows under it.
    expect(r.host.querySelector<HTMLInputElement>('.ask-input')!.value).toBe('');
    expect(r.host.querySelector('.ask-prompt')!.textContent).toBe('move this to Thursday');
    const links = [...r.host.querySelectorAll<HTMLElement>('.ask-markdown .ask-link')];
    expect(links.map((a) => a.textContent)).toEqual(['Kanji quiz 4', 'Kanji']);
    expect([...r.host.querySelectorAll('.ask-chip')].map((c) => c.textContent)).toEqual(['Kanji']);
    // A link closes the box and shows the thing; the article opens in the Wiki tab.
    await click(links[0]);
    await click(r.host.querySelector('.ask-chip'));
    expect(r.opened).toEqual([
      { what: 'row', table: 'task', id: 't1' },
      { what: 'wiki', title: 'Kanji' },
    ]);
    expect(r.closed).toBe(2);
    // The next request carries what was said before.
    await type(r, 'and the essay?');
    await press(r, 'return');
    expect(r.core.asked('ask.send')[1].history).toEqual([
      { prompt: 'move this to Thursday', answer: ANSWER.answer, applied: false },
    ]);
  });

  it('says what Claude is doing while it works', async () => {
    let finish: (a: Answer) => void = () => {};
    const r = await mount({ 'ask.send': () => new Promise<Answer>((res) => (finish = res)) });
    await type(r, 'what is due');
    await press(r, 'return');
    expect(r.host.querySelector('.ask-doing')!.textContent).toContain('Asking Claude…');
    await act(async () => r.core.emit('ask', { id: 'run9', tool: 'list_rows', doing: 'Reading a table' }));
    expect(r.host.querySelector('.ask-doing')!.textContent).toContain('Reading a table…');
    await click(button(r, 'Stop'));
    expect(r.core.asked('ask.cancel')).toEqual([{ id: 'run9' }]);
    await act(async () => finish({ ...ANSWER, change: null, outward: [] }));
    await settle();
    expect(r.host.querySelector('.ask-doing')).toBeNull();
    expect(r.host.querySelector('.ask-change')).toBeNull();
  });

  it('shows a change as a preview, and makes it on one click or on Return', async () => {
    const r = await mount();
    await type(r, 'move it');
    await press(r, 'return');
    const preview = r.host.querySelector('.ask-change')!;
    expect(preview.querySelector('strong')!.textContent).toBe('1 task moved');
    expect(preview.querySelector('.ask-lines')!.textContent).toBe('Kanji quiz 4: Due 2026-10-09 23:59 → 2026-10-08 23:59');
    expect(preview.textContent).toContain('Not applied yet');
    expect(r.core.asked('ask.apply')).toHaveLength(0);
    // Return on the empty box is the Apply button.
    await press(r, 'return');
    expect(r.core.asked('ask.apply')).toEqual([{ id: 'run1' }]);
    expect(r.said).toEqual(['Applied — 1 task moved']);
    expect(r.host.querySelector('.ask-change')!.textContent).toContain('Applied.');
    expect(button(r, 'Apply')).toBeUndefined();
    // Return again applies nothing twice.
    await press(r, 'return');
    expect(r.core.asked('ask.apply')).toHaveLength(1);
  });

  it('lets a change go, and nothing is made', async () => {
    const r = await mount();
    await type(r, 'move it');
    await press(r, 'return');
    await click(button(r, 'Discard'));
    expect(r.core.asked('ask.discard')).toEqual([{ id: 'run1' }]);
    expect(r.core.asked('ask.apply')).toHaveLength(0);
    expect(r.host.textContent).toContain('Discarded. Nothing changed.');
    await press(r, 'return');
    expect(r.core.asked('ask.apply')).toHaveLength(0);
  });

  it('asks about what leaves this Mac on its own, every time', async () => {
    const r = await mount();
    await type(r, 'make it public');
    await press(r, 'return');
    const outward = r.host.querySelector('.ask-outward')!;
    expect(outward.textContent).toContain('This leaves this Mac');
    expect(outward.textContent).toContain('anyone who opens your sun can see it');
    // Applying the rest doesn't answer it.
    await click(button(r, 'Apply'));
    expect(r.core.asked('ask.applyOutward')).toHaveLength(0);
    await click(button(r, 'Yes, do it'));
    expect(r.core.asked('ask.applyOutward')).toEqual([{ id: 'run1', index: 0 }]);
    expect(r.host.querySelector('.ask-outward')!.textContent).toContain('Done.');
    expect(button(r, 'Yes, do it')).toBeUndefined();
  });

  it('says what couldn’t be made', async () => {
    const r = await mount({
      'ask.apply': () => ({
        applied: 0,
        failed: [{ line: 'Kanji quiz 4: Due → someday', message: '‘someday’ isn’t a date.' }],
        summary: '1 task moved',
        undo: null,
        steps: 1,
      }),
    });
    await type(r, 'move it');
    await press(r, 'return');
    await click(button(r, 'Apply'));
    expect(r.host.querySelector('.ask-failed')!.textContent).toBe('Not made: Kanji quiz 4: Due → someday. ‘someday’ isn’t a date.');
    expect(r.said).toEqual(['Applied 0 of 1']);
  });
});
