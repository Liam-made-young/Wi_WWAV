// ⌘K: the Claude prompt box, where Search was (docs/ASK.md).
//
// It is still a search. As you type, matches show at once: actions, Learn's
// records from the library's own index (`ask.search`), settings and clips.
// ↓ and Return open one, as before. Return with nothing picked sends what
// you typed to Claude, and the answer draws in the panel under the box with
// every Learn record and Wikipedia article in it as a link.
//
// What Claude would change is never made by asking. It shows as a preview
// ("2 tasks moved") with what each line would do; Apply (or Return on the
// empty box) makes all of it, as one ⌘Z. Anything that leaves this Mac is
// asked about on its own, every time, and only its own button says yes.
//
// With Claude Code missing, signed out or offline the box says so in one
// line and goes on searching. Its keys come through the shell's router.

import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';
import { call } from '../bridge';
import { useCoreEvent } from '../shell/hooks';
import type { Clip } from '../shell/LibraryDrawer';
import { latestOnly, parseQuery, type Query, rank } from '../shell/palette';
import { IS_MAC, keys } from '../shell/platform';
import './ask.css';
import { type Answer, type Applied, ask, type Found } from './client';
import { screen } from './context';
import { Markdown } from './markdown';
import type { Target } from './nav';

export interface Item {
  label: string;
  hint?: string;
  run(): void;
}

export interface PaletteTask {
  id: string;
  title: string;
  level: string;
}

interface Props {
  shown: boolean;
  actions: Item[];
  settings: Item[];
  tasks: PaletteTask[];
  onTask(id: string): void;
  onClip(clip: Clip, other: boolean): void;
  /** A link in an answer, or a result that isn't a task. `background` leaves the box open. */
  onOpen(target: Target, background: boolean): void;
  onClose(): void;
  /** One sentence for the shell's toast: "Applied — 2 tasks moved". */
  onSaid(text: string): void;
}

export interface AskHandle {
  move(by: 1 | -1): void;
  /** Return: open the picked result, else apply a waiting preview, else ask Claude. */
  open(other: boolean): void;
}

interface Result {
  id: string;
  group: string;
  label: string;
  hint?: string;
  run(other: boolean): void;
}

/** One request and what came of it. */
interface Turn {
  n: number;
  prompt: string;
  state: 'asking' | 'done' | 'failed';
  /** While asking: "Reading a table". */
  doing: string;
  /** The core's name for the run, once it has said one: what Stop stops. */
  run: string | null;
  answer: Answer | null;
  error: string | null;
  /** What became of the preview. */
  change: 'waiting' | 'applying' | 'applied' | 'discarded' | null;
  applied: Applied | null;
  /** Each outward change's answer: null while it waits. */
  outward: ('yes' | 'no' | 'asking' | null)[];
  outwardSaid: (string | null)[];
}

// The core's own search reads tag:, key: and bpm: (library.search).
function coreQuery(q: Query): string {
  const parts = [q.words, ...q.tags.map((t) => `tag:${t}`)];
  if (q.key) parts.push(`key:${q.key.replace(/ /g, '-')}`);
  if (q.bpm) parts.push(`bpm:${q.bpm[0]}-${q.bpm[1]}`);
  return parts.filter(Boolean).join(' ');
}

const GROUPS: Record<string, string> = {
  task: 'Tasks',
  course: 'Courses',
  note: 'Notes',
  dailyNote: 'Notes',
  habit: 'Habits',
  mailThread: 'Mail',
  grade: 'Grades',
  project: 'Projects',
  milestone: 'Projects',
  capture: 'Inbox',
  space: 'Spaces',
  dbRow: 'Database',
  dbTable: 'Database',
  dbView: 'Database',
};

const sentence = (e: unknown) => (e instanceof Error ? e.message : String(e));

export const AskBox = forwardRef<AskHandle, Props>(function AskBox(
  { shown, actions, settings, tasks, onTask, onClip, onOpen, onClose, onSaid },
  ref,
) {
  const [text, setText] = useState('');
  const [found, setFound] = useState<Found[]>([]);
  const [clips, setClips] = useState<Clip[]>([]);
  // -1: nothing picked, so Return asks Claude.
  const [at, setAt] = useState(-1);
  const [turns, setTurns] = useState<Turn[]>([]);
  const [unavailable, setUnavailable] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const thread = useRef<HTMLDivElement>(null);
  const count = useRef(0);
  const query = useMemo(() => parseQuery(text), [text]);
  const filtered = query.tags.length > 0 || query.key !== null || query.bpm !== null;
  const typing = text.trim().length > 0;

  const searchClips = useMemo(
    () =>
      latestOnly(
        (q) => call<{ clips: Clip[] }>('library.search', { q, limit: 20 }).then((r) => r.clips),
        (_q, list) => setClips(list),
      ),
    [],
  );
  // Learn's records come from the index on this Mac: no reason to wait on the keys.
  const searchLearn = useMemo(
    () =>
      latestOnly(
        (q) => ask.search(q, 14),
        (_q, list) => setFound(list),
        30,
      ),
    [],
  );

  useEffect(() => {
    if (!shown) {
      searchClips.cancel();
      searchLearn.cancel();
      return;
    }
    input.current?.focus();
    input.current?.select();
    // Each opening starts with nothing picked, so Return asks.
    setAt(-1);
    // Says once per opening whether Claude can be asked at all.
    ask.status().then(
      (s) => setUnavailable(s.available ? null : (s.reason ?? 'Claude Code isn’t available.')),
      () => setUnavailable(null),
    );
  }, [shown, searchClips, searchLearn]);

  useEffect(() => {
    setAt(-1);
    setClips([]);
    const plainWords = !query.remix && !query.hot && query.person === null;
    const q = coreQuery(query);
    if (shown && q && plainWords) searchClips.ask(q);
    else searchClips.cancel();
    if (shown && query.words && plainWords && !filtered) searchLearn.ask(query.words);
    else {
      searchLearn.cancel();
      setFound([]);
    }
  }, [query, shown, filtered, searchClips, searchLearn]);

  const results = useMemo<Result[]>(() => {
    if (!typing) return [];
    const out: Result[] = [];
    const plain = !filtered && !query.hot && !query.remix && query.person === null;
    if (plain) {
      for (const a of rank(query.words, actions, (a) => a.label).slice(0, 4)) {
        out.push({ id: `a:${a.label}`, group: 'Actions', label: a.label, hint: a.hint, run: () => a.run() });
      }
    }
    // is:hot is the open tasks Learn calls hot, in its own order.
    if (query.hot && !filtered) {
      const pool = tasks.filter((t) => t.level === 'Hot' || t.level === 'Overdue');
      for (const t of rank(query.words, pool, (t) => t.title).slice(0, 8)) {
        out.push({ id: `t:${t.id}`, group: 'Tasks', label: t.title, hint: t.level, run: () => onTask(t.id) });
      }
    }
    if (plain) {
      const level = new Map(tasks.map((t) => [t.id, t.level]));
      for (const f of found) {
        const hint = f.kind === 'task' ? [f.hint, level.get(f.id)].filter(Boolean).join(' · ') : f.hint;
        out.push({
          id: `${f.kind}:${f.id}`,
          group: GROUPS[f.kind] ?? f.word,
          label: f.title,
          hint: hint || f.word,
          run: (other) => {
            if (f.kind === 'task' && !other) onTask(f.id);
            else if (f.kind === 'dbTable') onOpen({ what: 'table', table: f.id }, false);
            else if (f.kind === 'dbView') onOpen({ what: 'table', table: f.table, view: f.id }, false);
            else onOpen({ what: 'row', table: f.table, id: f.id }, false);
          },
        });
      }
      // A task typed by a few of its letters still shows, as it did before.
      if (!found.some((f) => f.kind === 'task')) {
        for (const t of rank(query.words, tasks, (t) => t.title).slice(0, 5)) {
          out.push({ id: `t:${t.id}`, group: 'Tasks', label: t.title, hint: t.level, run: () => onTask(t.id) });
        }
      }
      for (const s of rank(query.words, settings, (s) => s.label).slice(0, 4)) {
        out.push({ id: `s:${s.label}`, group: 'Settings', label: s.label, run: () => s.run() });
      }
    }
    for (const c of clips) {
      const hint = [c.kind === 'wwav' ? 'song' : c.kind, c.key, c.bpm ? `${c.bpm} BPM` : null]
        .filter(Boolean)
        .join(' · ');
      out.push({ id: `c:${c.id}`, group: 'Clips', label: c.title, hint, run: (other) => onClip(c, other) });
    }
    // One group's rows sit together under one heading.
    const order: string[] = [];
    for (const r of out) if (!order.includes(r.group)) order.push(r.group);
    return order.flatMap((g) => out.filter((r) => r.group === g));
  }, [typing, query, filtered, actions, settings, tasks, found, clips, onTask, onClip, onOpen]);

  const update = (n: number, change: (t: Turn) => Turn) =>
    setTurns((all) => all.map((t) => (t.n === n ? change(t) : t)));

  useCoreEvent<{ id: string; doing?: string }>('ask', ({ id, doing }) => {
    if (!doing) return;
    setTurns((all) => {
      const last = all[all.length - 1];
      return last?.state === 'asking' ? [...all.slice(0, -1), { ...last, doing, run: id }] : all;
    });
  });

  // The newest turn stays in view as it grows.
  const last = turns[turns.length - 1];
  useEffect(() => {
    if (!typing && thread.current) thread.current.scrollTop = thread.current.scrollHeight;
  }, [turns, typing]);

  const send = () => {
    const prompt = text.trim();
    if (!prompt || last?.state === 'asking') return;
    const n = ++count.current;
    const history = turns
      .filter((t) => t.state === 'done' && t.answer)
      .slice(-4)
      .map((t) => ({
        prompt: t.prompt,
        answer: t.answer!.answer,
        applied: t.change === null ? undefined : t.change === 'applied',
      }));
    setTurns((all) => [
      ...all.slice(-11),
      {
        n,
        prompt,
        state: 'asking',
        doing: 'Asking Claude',
        run: null,
        answer: null,
        error: null,
        change: null,
        applied: null,
        outward: [],
        outwardSaid: [],
      },
    ]);
    setText('');
    ask.send(prompt, screen(), history).then(
      (answer) => {
        update(n, (t) => ({
          ...t,
          state: 'done',
          answer,
          change: answer.change ? 'waiting' : null,
          outward: answer.outward.map(() => null),
          outwardSaid: answer.outward.map(() => null),
        }));
        // What Claude was asked to show, shown: the last thing it opened.
        const open = answer.opens[answer.opens.length - 1];
        if (open?.what === 'wiki' && open.title) onOpen({ what: 'wiki', title: open.title }, true);
        else if (open?.what === 'row' && open.table && open.id) onOpen({ what: 'row', table: open.table, id: open.id }, true);
        else if (open?.what === 'table' && open.table) onOpen({ what: 'table', table: open.table }, true);
        else if (open?.what === 'tab' && open.tab) onOpen({ what: 'tab', tab: open.tab }, true);
      },
      (e) => {
        update(n, (t) => ({ ...t, state: 'failed', error: sentence(e) }));
        if ((e as { code?: string }).code === 'claude') setUnavailable(null);
      },
    );
  };

  const apply = (turn: Turn) => {
    if (!turn.answer || turn.change !== 'waiting') return;
    update(turn.n, (t) => ({ ...t, change: 'applying' }));
    ask.apply(turn.answer.id).then(
      (applied) => {
        update(turn.n, (t) => ({ ...t, change: 'applied', applied }));
        onSaid(applied.failed.length ? `Applied ${applied.applied} of ${applied.applied + applied.failed.length}` : `Applied — ${applied.summary}`);
      },
      (e) => update(turn.n, (t) => ({ ...t, change: 'waiting', error: sentence(e) })),
    );
  };

  const discard = (turn: Turn) => {
    if (!turn.answer) return;
    update(turn.n, (t) => ({ ...t, change: 'discarded' }));
    void ask.discard(turn.answer.id).catch(() => {});
  };

  const answerOutward = (turn: Turn, index: number, yes: boolean) => {
    if (!turn.answer) return;
    const set = (v: Turn['outward'][number], said: string | null = null) =>
      update(turn.n, (t) => ({
        ...t,
        outward: t.outward.map((o, i) => (i === index ? v : o)),
        outwardSaid: t.outwardSaid.map((o, i) => (i === index ? said : o)),
      }));
    if (!yes) return set('no');
    set('asking');
    ask.applyOutward(turn.answer.id, index).then(
      (r) => set('yes', r.sentence ?? null),
      (e) => set(null, sentence(e)),
    );
  };

  useImperativeHandle(ref, () => ({
    move(by) {
      if (!typing) return;
      setAt((i) => Math.min(Math.max(-1, i + by), results.length - 1));
    },
    open(other) {
      const picked = typing ? results[at] : undefined;
      if (picked) {
        onClose();
        picked.run(other);
        return;
      }
      if (typing) return send();
      // Return on the empty box says yes to the preview that waits.
      if (last?.change === 'waiting') apply(last);
    },
  }));

  const why = query.remix
    ? 'is:remix finds nothing yet: the library doesn’t record which songs are remixes.'
    : query.person !== null
      ? '@name finds people in Space, which isn’t in this build yet.'
      : null;

  const openLink = (target: Target, background: boolean) => {
    if (!background) onClose();
    onOpen(target, background);
  };

  let group = '';
  return (
    <div className="sheet palette ask" role="dialog" aria-label="Ask Claude or search" hidden={!shown}>
      <div className="ask-field">
        <Spark />
        <input
          ref={input}
          className="palette-input ask-input"
          type="search"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-results"
          aria-activedescendant={typing && results[at] ? `palette-${at}` : undefined}
          placeholder="Search, or ask Claude"
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
      </div>

      {typing ? (
        <div id="palette-results" className="palette-results" role="listbox" aria-label="Results">
          <div
            className="palette-row ask-send"
            role="option"
            id="palette--1"
            aria-selected={at === -1}
            onPointerEnter={() => setAt(-1)}
            onClick={send}
          >
            <span className="palette-label">
              {unavailable ? 'Claude can’t be asked right now' : `Ask Claude: “${text.trim()}”`}
            </span>
            <span className="palette-hint" data-text="secondary">
              Return
            </span>
          </div>
          {results.map((r, i) => {
            const heading = r.group !== group ? r.group : null;
            group = r.group;
            return (
              <div key={r.id}>
                {heading && (
                  <div className="palette-group" role="presentation" data-text="secondary">
                    {heading}
                  </div>
                )}
                <div
                  id={`palette-${i}`}
                  role="option"
                  aria-selected={i === at}
                  className="palette-row"
                  onPointerEnter={() => setAt(i)}
                  onClick={(e) => {
                    onClose();
                    r.run(IS_MAC ? e.metaKey : e.ctrlKey);
                  }}
                >
                  <span className="palette-label">{r.label}</span>
                  {r.hint && (
                    <span className="palette-hint" data-text="secondary">
                      {r.hint}
                    </span>
                  )}
                </div>
              </div>
            );
          })}
          {results.length === 0 && <p className="palette-empty">{why ?? 'Nothing on this Mac matches. Return asks Claude.'}</p>}
        </div>
      ) : (
        <div id="palette-results" className="ask-thread" ref={thread} aria-live="polite" aria-label="Claude’s answers">
          {turns.length === 0 && (
            <p className="palette-empty">
              {unavailable ??
                'Type to search tasks, courses, notes, mail and your tables. Press Return to ask Claude about them, or to change them.'}
            </p>
          )}
          {turns.map((t) => (
            <article key={t.n} className="ask-turn" data-state={t.state}>
              <p className="ask-prompt">{t.prompt}</p>
              {t.state === 'asking' && (
                <p className="ask-doing" role="status">
                  <span className="ask-dots" aria-hidden="true" />
                  {t.doing}…
                  {t.run && (
                    <button type="button" className="ask-stop" onClick={() => void ask.cancel(t.run!).catch(() => {})}>
                      Stop
                    </button>
                  )}
                </p>
              )}
              {t.error && (
                <p className="ask-error" role="alert">
                  {t.error}
                </p>
              )}
              {t.answer && <Markdown text={t.answer.answer} onOpen={openLink} />}
              {t.answer && t.answer.wiki.length > 0 && (
                <p className="ask-wiki">
                  <span data-text="secondary">Wikipedia</span>
                  {t.answer.wiki.map((title) => (
                    <a
                      key={title}
                      className="ask-chip"
                      href="#"
                      onClick={(e) => {
                        e.preventDefault();
                        openLink({ what: 'wiki', title }, e.metaKey || e.ctrlKey);
                      }}
                    >
                      {title}
                    </a>
                  ))}
                </p>
              )}
              {t.answer?.change && t.change !== 'discarded' && (
                <section className="ask-change" data-state={t.change ?? undefined} aria-label="Preview of changes">
                  <header className="ask-change-head">
                    <strong>{t.answer.change.summary || `${t.answer.change.count} changes`}</strong>
                    <span data-text="secondary">
                      {t.change === 'applied'
                        ? `Applied. ${keys('⌘Z')} undoes all of it.`
                        : t.change === 'applying'
                          ? 'Applying…'
                          : 'Not applied yet'}
                    </span>
                  </header>
                  <ul className="ask-lines">
                    {t.answer.change.lines.map((line, i) => (
                      <li key={i}>{line}</li>
                    ))}
                  </ul>
                  {t.applied && t.applied.failed.length > 0 && (
                    <ul className="ask-lines ask-failed" role="alert">
                      {t.applied.failed.map((f, i) => (
                        <li key={i}>
                          Not made: {f.line}. {f.message}
                        </li>
                      ))}
                    </ul>
                  )}
                  {t.change === 'waiting' && (
                    <div className="ask-buttons">
                      <button type="button" className="gel ask-apply" data-default onClick={() => apply(t)}>
                        Apply
                      </button>
                      <button type="button" className="gel" onClick={() => discard(t)}>
                        Discard
                      </button>
                      {t === last && (
                        <span data-text="secondary" className="ask-hint">
                          Return applies
                        </span>
                      )}
                    </div>
                  )}
                </section>
              )}
              {t.change === 'discarded' && (
                <p className="ask-note" data-text="secondary">
                  Discarded. Nothing changed.
                </p>
              )}
              {t.answer?.outward.map((o, i) => (
                <section key={o.index} className="ask-change ask-outward" aria-label="Leaves this Mac">
                  <header className="ask-change-head">
                    <strong>This leaves this Mac</strong>
                  </header>
                  <p className="ask-outward-line">{o.line}</p>
                  {t.outwardSaid[i] && <p className="ask-note">{t.outwardSaid[i]}</p>}
                  {t.outward[i] === null || t.outward[i] === 'asking' ? (
                    <div className="ask-buttons">
                      <button
                        type="button"
                        className="gel"
                        disabled={t.outward[i] === 'asking'}
                        onClick={() => answerOutward(t, i, true)}
                      >
                        Yes, do it
                      </button>
                      <button type="button" className="gel" onClick={() => answerOutward(t, i, false)}>
                        No
                      </button>
                    </div>
                  ) : (
                    <p className="ask-note" data-text="secondary">
                      {t.outward[i] === 'yes' ? 'Done.' : 'Not done.'}
                    </p>
                  )}
                </section>
              ))}
            </article>
          ))}
        </div>
      )}
      <p className="palette-foot" data-text="secondary">
        {typing
          ? `Return asks Claude · ↓ then Return opens a result · ${keys('⌘')}Return opens it in its other view · Esc closes`
          : unavailable && turns.length > 0
            ? unavailable
            : 'Changes show as a preview first, and apply as one undo · Esc closes'}
      </p>
    </div>
  );
});

function Spark() {
  return (
    <svg className="ask-spark" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
      <path
        d="M8 1.2l1.5 4.1a1.2 1.2 0 0 0 .7.7L14.8 8l-4.6 2a1.2 1.2 0 0 0-.7.7L8 14.8l-1.5-4.1a1.2 1.2 0 0 0-.7-.7L1.2 8l4.6-2a1.2 1.2 0 0 0 .7-.7z"
        fill="currentColor"
      />
    </svg>
  );
}
