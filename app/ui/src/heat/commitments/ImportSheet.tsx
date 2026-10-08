// Import schedule (docs/COMMITMENTS.md, "Getting commitments in"): a
// timetable, this week's shifts, or an academic calendar's breaks, by one of
// three ways. Pasted text and a photo or screenshot are read by Claude; an
// .ics file or a calendar's address is read by the core, and an address can
// be kept in step. A file comes by a drop on the sheet or through the
// system's open panel, as a path, the way a syllabus does.
//
// Every way ends in a draft, and the sheet turns into its preview, drawn
// from `snap.commitments.drafts`: "Reading …" while it is read, the sentence
// if it failed, and each line once it is ready, with a box to leave it out.
// Nothing is written before Apply, and one ⌘Z takes an applied schedule back.

import { invoke } from '@tauri-apps/api/core';
import { type FormEvent, useEffect, useRef, useState } from 'react';
import { shortMonthDay } from '../../shared/time/format';
import type { DayKey } from '../../shared/time/zone';
import { listenForDrops } from '../../shell/drop';
import { keys } from '../../shell/platform';
import type { CommitmentDraft, Id, ScheduleMode } from '../client';
import { useDraft } from '../frame';
import { useHeat } from '../store';
import * as copy from './copy';
import { mondayOf } from './time';

/** What marks the sheet as where a schedule's file is dropped (shell/drop.ts reads `data-drop`). */
const DROP = 'schedule';
const IMAGE = /\.(png|jpe?g|heic|webp|gif|tiff?|bmp)$/i;
const ICS = /\.ics$/i;

/**
 * The system's open panel, for one photo, screenshot or .ics file. `picker`
 * is false where there is no panel to open (a browser, or a window not
 * allowed the dialog), and the sheet says to drop the file instead.
 */
async function pickFile(): Promise<{ picker: boolean; path: string | null }> {
  if (!('__TAURI_INTERNALS__' in window)) return { picker: false, path: null };
  try {
    const picked = await invoke<string | { path?: string } | null>('plugin:dialog|open', {
      options: {
        multiple: false,
        directory: false,
        filters: [{ name: 'Schedule', extensions: ['png', 'jpg', 'jpeg', 'heic', 'webp', 'gif', 'ics'] }],
      },
    });
    return { picker: true, path: typeof picked === 'string' ? picked : (picked?.path ?? null) };
  } catch {
    return { picker: false, path: null };
  }
}

interface Props {
  /** A draft already waiting: the sheet opens on its preview. */
  draftId?: Id;
  /** What it is read as, to start with. */
  mode?: ScheduleMode;
  onClose(): void;
}

export function ImportSheet({ draftId: waiting, mode: first = 'schedule', onClose }: Props) {
  const { snap, client, act, say, refetch, date } = useHeat();
  const [mode, setMode] = useState<ScheduleMode>(first);
  const [weekOf, setWeekOf] = useState<DayKey>(() => mondayOf(date));
  const [text, setText, clearText] = useDraft('schedule:text', '');
  const [url, setUrl] = useState('');
  const [subscribe, setSubscribe] = useState(false);
  const [why, setWhy] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /** The lines left out of a ready draft, by their place in `items`. */
  const [left, setLeft] = useState<number[]>([]);

  // The draft this sheet is on: the one it was opened for, or the one a way in just answered.
  const [draftId, setDraftId] = useState<Id | null>(waiting ?? null);
  const [answered, setAnswered] = useState<CommitmentDraft | null>(null);
  const inSnap = snap?.commitments?.drafts.find((d) => d.id === draftId);
  const seen = useRef(waiting !== undefined);
  if (inSnap) seen.current = true;
  // Until the snapshot has it, the draft is as the call answered it.
  const draft = inSnap ?? (seen.current ? undefined : (answered ?? undefined));
  // Applied or discarded somewhere else: there is nothing left to preview.
  const gone = draftId !== null && snap !== null && !draft;
  useEffect(() => {
    if (gone) onClose();
  }, [gone, onClose]);

  const begin = async (way: Promise<{ draft?: CommitmentDraft }>) => {
    setWhy(null);
    setBusy(true);
    const r = await act(way);
    setBusy(false);
    if (!r?.draft) return;
    setAnswered(r.draft);
    setDraftId(r.draft.id);
    setLeft([]);
    // The draft shows as "Reading …" at once, whether or not the core announces it before it is read.
    await refetch();
  };
  const week = mode === 'week' ? weekOf : undefined;

  /** A file dropped or picked: an .ics is read by the core, a picture by Claude. */
  const importPath = (path: string | undefined) => {
    if (!path || !(ICS.test(path) || IMAGE.test(path))) return setWhy(copy.importing.asFile);
    return begin(ICS.test(path) ? client.commitments.importIcs({ path, mode }) : client.commitments.importImage(path, mode, week));
  };
  const dropped = useRef(importPath);
  dropped.current = importPath;
  const entering = draftId === null;
  useEffect(() => {
    if (!entering) return;
    // The sheet is its own drop target; shell/drop.ts hands every listener the target's name.
    return listenForDrops((target, paths) => {
      if ((target as string) === DROP) void dropped.current(paths.find((p) => ICS.test(p) || IMAGE.test(p)) ?? paths[0]);
    });
  }, [entering]);

  const choose = async () => {
    const { picker, path } = await pickFile();
    if (!picker) return setWhy(copy.importing.noPicker);
    if (path) await importPath(path);
  };

  const readText = () => {
    if (!text.trim()) return setWhy(copy.importing.pasteFirst);
    return begin(client.commitments.importText(text, mode, week));
  };

  const readAddress = async () => {
    const address = url.trim();
    if (!address) return setWhy(copy.importing.addressFirst);
    if (!subscribe) return begin(client.commitments.importIcs({ url: address, mode }));
    // Kept in step, a calendar's commitments are written at once, as one entry.
    setBusy(true);
    const r = await act(client.commitments.importIcs({ url: address, mode, subscribe: true }));
    setBusy(false);
    if (!r) return;
    say(copy.importing.subscribed(r.feed?.name ?? address, r.changed ?? 0, keys('⌘Z')));
    onClose();
    await refetch();
  };

  const apply = async (e: FormEvent) => {
    e.preventDefault();
    if (!draft || draft.state !== 'ready') return;
    setBusy(true);
    const r = await act(client.commitments.accept(draft.id, left));
    setBusy(false);
    if (!r) return;
    clearText();
    say(
      copy.importing.applied(
        { commitments: r.commitments, breaks: r.breaks, replaced: r.replaced, courses: r.courses.length },
        keys('⌘Z'),
      ),
    );
    onClose();
    await refetch();
  };

  const discard = async () => {
    if (!draft) return;
    setBusy(true);
    const r = await act(client.commitments.discard(draft.id));
    setBusy(false);
    if (!r) return;
    say(copy.importing.discarded);
    onClose();
    await refetch();
  };

  if (gone) return null;

  // --- the preview --------------------------------------------------------------------------

  if (draft) {
    const items = draft.items ?? [];
    const breaks = draft.breaks ?? [];
    return (
      <form
        noValidate
        className="sheet heat-sheet heat-sheet-wide heat-schedule"
        role="dialog"
        aria-label={copy.importing.title}
        data-state={draft.state}
        onSubmit={(e) => void apply(e)}
      >
        <h2 className="sheet-title">{copy.importing.title}</h2>
        {draft.state === 'reading' && <p role="status">{copy.importing.reading(draft.fileName)}</p>}
        {draft.state === 'failed' && (
          <p className="heat-syllabus-flag" role="alert">
            {draft.error ?? copy.importing.failed}
          </p>
        )}
        {draft.state === 'ready' && (
          <>
            {draft.line && <p className="heat-syllabus-line">{draft.line}</p>}
            <p data-text="secondary">
              {[copy.modes[draft.mode], draft.mode === 'week' ? shortMonthDay(draft.weekOf) : null, draft.fileName]
                .filter(Boolean)
                .join(' · ')}
            </p>
            {items.length > 0 && (
              <fieldset className="heat-set">
                <legend data-text="secondary">{copy.importing.items}</legend>
                <table className="heat-syllabus-table heat-schedule-table">
                  <tbody>
                    {items.map((item, i) => (
                      <tr key={i} data-item={i} data-new={item.isNew}>
                        <td>
                          <label className="check" data-dense>
                            <input
                              type="checkbox"
                              checked={item.isNew && !left.includes(i)}
                              disabled={!item.isNew}
                              onChange={(e) => setLeft(e.target.checked ? left.filter((n) => n !== i) : [...left, i])}
                            />
                            <span className="heat-sr">{copy.importing.include(item.title)}</span>
                          </label>
                        </td>
                        <th scope="row">{item.title}</th>
                        <td>{item.when}</td>
                        <td data-text="secondary">{[item.course, item.location].filter(Boolean).join(' · ')}</td>
                        <td>
                          {!item.isNew && <span className="heat-tag">{copy.importing.already}</span>}
                          {item.isNew && item.newCourse && <span className="heat-tag">{copy.importing.newCourse}</span>}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </fieldset>
            )}
            {breaks.length > 0 && (
              <fieldset className="heat-set">
                <legend data-text="secondary">{copy.importing.breaks}</legend>
                <table className="heat-syllabus-table heat-schedule-table">
                  <tbody>
                    {breaks.map((b) => (
                      <tr key={`${b.from}-${b.title}`} data-break={b.from} data-new={b.isNew}>
                        <th scope="row">{b.title}</th>
                        <td>{b.from === b.to ? shortMonthDay(b.from) : `${shortMonthDay(b.from)} to ${shortMonthDay(b.to)}`}</td>
                        <td>{!b.isNew && <span className="heat-tag">{copy.importing.already}</span>}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </fieldset>
            )}
            {items.length === 0 && breaks.length === 0 && <p data-text="secondary">{copy.importing.nothing}</p>}
            {(draft.unread ?? 0) > 0 && (
              <p className="why" data-text="secondary">
                {copy.importing.unread(draft.unread ?? 0)}
              </p>
            )}
            <p className="why" data-text="secondary">
              {copy.importing.nothingBefore}
            </p>
          </>
        )}
        <div className="note-actions">
          {draft.state === 'ready' && (
            <button type="submit" className="gel" disabled={busy}>
              {copy.importing.apply}
            </button>
          )}
          <button type="button" className="gel plain" disabled={busy} onClick={() => void discard()}>
            {copy.importing.discard}
          </button>
          {draft.state === 'reading' && (
            <button type="button" className="gel plain" onClick={onClose}>
              {copy.importing.later}
            </button>
          )}
        </div>
      </form>
    );
  }

  // --- the three ways in ----------------------------------------------------------------------

  return (
    <div className="sheet heat-sheet heat-sheet-wide heat-schedule" role="dialog" aria-label={copy.importing.title} data-drop={DROP}>
      <h2 className="sheet-title">{copy.importing.title}</h2>
      <p className="why" data-text="secondary">
        {copy.importing.hint}
      </p>
      <div className="heat-schedule-modes" role="radiogroup" aria-label="What it is">
        {(Object.keys(copy.modes) as ScheduleMode[]).map((m) => (
          <label key={m} className="check" data-dense>
            <input type="radio" name="heat-schedule-mode" checked={mode === m} onChange={() => setMode(m)} />
            {copy.modes[m]}
          </label>
        ))}
        {mode === 'week' && (
          <label className="field">
            <span data-text="secondary">{copy.importing.weekOf}</span>
            <input type="date" value={weekOf} onChange={(e) => e.target.value && setWeekOf(mondayOf(e.target.value))} />
          </label>
        )}
      </div>

      <fieldset className="heat-set">
        <legend data-text="secondary">{copy.importing.paste}</legend>
        <label className="field">
          <span className="heat-sr">{copy.importing.paste}</span>
          <textarea
            rows={5}
            value={text}
            placeholder={copy.importing.pasteHint}
            onChange={(e) => {
              setText(e.target.value);
              setWhy(null);
            }}
          />
        </label>
        <button type="button" className="gel" disabled={busy} onClick={() => void readText()}>
          {copy.importing.read}
        </button>
      </fieldset>

      <fieldset className="heat-set">
        <legend data-text="secondary">{copy.importing.file}</legend>
        <div className="heat-schedule-drop">
          <span data-text="secondary">{copy.importing.dropHere}</span>
          <button type="button" className="gel" disabled={busy} onClick={() => void choose()}>
            {copy.importing.choose}
          </button>
        </div>
      </fieldset>

      <fieldset className="heat-set">
        <legend data-text="secondary">{copy.importing.address}</legend>
        <label className="field">
          <span className="heat-sr">{copy.importing.address}</span>
          <input
            type="url"
            value={url}
            placeholder="https://…/calendar.ics"
            onChange={(e) => {
              setUrl(e.target.value);
              setWhy(null);
            }}
          />
        </label>
        <label className="check" data-dense>
          <input type="checkbox" checked={subscribe} onChange={(e) => setSubscribe(e.target.checked)} />
          {copy.importing.keep}
        </label>
        <p className="why" data-text="secondary">
          {copy.importing.keepHint}
        </p>
        <button type="button" className="gel" disabled={busy} onClick={() => void readAddress()}>
          {subscribe ? copy.importing.subscribe : copy.importing.read}
        </button>
      </fieldset>

      <p className="why" data-text="secondary" role="status">
        {why}
      </p>
      <div className="note-actions">
        <button type="button" className="gel plain" onClick={onClose}>
          Cancel
        </button>
      </div>
    </div>
  );
}
