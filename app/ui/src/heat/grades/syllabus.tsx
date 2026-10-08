// A course's syllabus (docs/HEAT.md): the PDF that gives a course its
// weights and its task types. It comes in by a drop on a course's card (or on
// the Grades page, for a course Learn doesn't know yet) or through the
// system's open panel, and the core reads it into a draft. Everything here is
// drawn from `snap.syllabus.drafts`, so a draft Claude finished in the
// background, or one left from before a relaunch, shows the same as one just
// dropped: "Reading …" while it is read, the sentence if it failed, and the
// preview sheet once it is ready. Nothing is written before Accept, and one
// ⌘Z takes an accepted syllabus back.

import { invoke } from '@tauri-apps/api/core';
import { type FormEvent, useEffect, useRef, useState } from 'react';
import { clockAt, shortMonthDay } from '../../shared/time/format';
import { dayKey } from '../../shared/time/zone';
import { keys } from '../../shell/platform';
import type { Id, SyllabusDraft } from '../client';
import { copy, courseLabel, formatMinutes } from '../fmt';
import { useSheets } from '../frame';
import { useHeat } from '../store';
import { pctText } from './format';

/**
 * The system's open panel, for one PDF. `picker` is false where there is no
 * panel to open (a browser, or a window not allowed the dialog), and the
 * caller says to drop the file instead; `path` is null when it was cancelled.
 */
export async function pickPdf(): Promise<{ picker: boolean; path: string | null }> {
  if (!('__TAURI_INTERNALS__' in window)) return { picker: false, path: null };
  try {
    // The dialog plugin's own command (app/src-tauri: `dialog:allow-open`), asked for one file.
    const picked = await invoke<string | { path?: string } | null>('plugin:dialog|open', {
      options: { multiple: false, directory: false, filters: [{ name: 'PDF', extensions: ['pdf'] }] },
    });
    const path = typeof picked === 'string' ? picked : (picked?.path ?? null);
    return { picker: true, path };
  } catch {
    return { picker: false, path: null };
  }
}

/** The drafts in the snapshot, and the ways a syllabus comes in and goes. */
export function useSyllabus() {
  const { snap, client, act, say, refetch } = useHeat();
  const { openSheet, closeSheet } = useSheets();
  const drafts = snap?.syllabus?.drafts ?? [];

  /** Files dropped or picked: the first PDF is read, for `courseId` or for the course it names. */
  const importPaths = async (paths: string[], courseId?: Id) => {
    const pdf = paths.find((p) => /\.pdf$/i.test(p));
    if (!pdf) return say(copy.syllabus.asPdf);
    const r = await act(client.importSyllabus({ path: pdf, ...(courseId ? { courseId } : {}) }));
    // The draft shows as "Reading …" at once, whether or not the core announces it before it is read.
    if (r) await refetch();
  };

  /** The Import syllabus button: the open panel, or the sheet that says where to drop the file. */
  const pick = async (courseId?: Id) => {
    const { picker, path } = await pickPdf();
    if (!picker) return openSheet(<DropSyllabusSheet onClose={closeSheet} />);
    if (path) await importPaths([path], courseId);
  };

  const discard = async (draftId: Id) => {
    if (await act(client.syllabus.discard(draftId))) await refetch();
  };

  const review = (draftId: Id) => openSheet(<SyllabusSheet draftId={draftId} onClose={closeSheet} />);

  return { drafts, importPaths, pick, discard, review };
}

// --- a draft's line on a card or in the header -------------------------------------------

/** Each draft's state in one quiet line: being read, failed (with Dismiss), or read and waiting (with Review). */
export function SyllabusLines({ drafts }: { drafts: SyllabusDraft[] }) {
  const { discard, review } = useSyllabus();
  if (drafts.length === 0) return null;
  return (
    <>
      {drafts.map((d) => (
        <p key={d.id} className="heat-syllabus-state" data-state={d.state} data-text="secondary" role="status">
          <span className="heat-syllabus-words">
            {d.state === 'reading'
              ? copy.syllabus.reading(d.fileName)
              : d.state === 'failed'
                ? (d.error ?? copy.status.failed)
                : copy.syllabus.ready(d.fileName)}
          </span>
          {d.state === 'failed' && (
            <button type="button" className="gel plain" onClick={() => void discard(d.id)}>
              {copy.syllabus.dismiss}
            </button>
          )}
          {d.state === 'ready' && (
            <button type="button" className="gel plain" onClick={() => review(d.id)}>
              {copy.syllabus.review}
            </button>
          )}
        </p>
      ))}
    </>
  );
}

// --- where to drop it, when there is no open panel ----------------------------------------

function DropSyllabusSheet({ onClose }: { onClose(): void }) {
  const close = useRef<HTMLButtonElement>(null);
  useEffect(() => close.current?.focus(), []);
  return (
    <div className="sheet heat-sheet" role="dialog" aria-label={copy.syllabus.import}>
      <h2 className="sheet-title">{copy.syllabus.import}</h2>
      <p>{copy.syllabus.dropHere}</p>
      <div className="note-actions">
        <button ref={close} type="button" className="gel" onClick={onClose}>
          Done
        </button>
      </div>
    </div>
  );
}

// --- the preview ---------------------------------------------------------------------------

/**
 * What a read syllabus would change, before any of it is written: the
 * course, the weights (with the warning when they don't add to 100), the
 * types, the new tasks and the dates that move. Accept writes it all as one
 * undo step; Discard drops the draft.
 */
export function SyllabusSheet({ draftId, onClose }: { draftId: Id; onClose(): void }) {
  const { snap, tz, client, act, say, refetch } = useHeat();
  const draft = snap?.syllabus?.drafts.find((d) => d.id === draftId);
  const [busy, setBusy] = useState(false);
  const accept = useRef<HTMLButtonElement>(null);
  useEffect(() => accept.current?.focus(), []);
  // Accepted or discarded somewhere else: there is nothing left to preview.
  const gone = snap !== null && (!draft || draft.state !== 'ready');
  useEffect(() => {
    if (gone) onClose();
  }, [gone, onClose]);
  if (!draft || draft.state !== 'ready') return null;

  const when = (ms: number | null) =>
    ms === null ? copy.syllabus.noDue : `${shortMonthDay(dayKey(ms, tz))} ${clockAt(ms, tz)}`;
  const course = draft.course;
  const weights = draft.weights ?? [];
  const types = draft.types ?? [];
  const newTasks = draft.newTasks ?? [];
  const dateChanges = draft.dateChanges ?? [];

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    const r = await act(client.syllabus.accept(draft.id));
    setBusy(false);
    if (!r) return;
    say(copy.syllabus.imported(keys('⌘Z')));
    onClose();
    await refetch();
  };

  const discard = async () => {
    setBusy(true);
    const r = await act(client.syllabus.discard(draft.id));
    setBusy(false);
    if (!r) return;
    say(copy.syllabus.discarded);
    onClose();
    await refetch();
  };

  return (
    <form
      noValidate
      className="sheet heat-sheet heat-sheet-wide heat-syllabus"
      role="dialog"
      aria-label={copy.syllabus.import}
      onSubmit={(e) => void save(e)}
    >
      <h2 className="sheet-title">{copy.syllabus.import}</h2>
      {draft.line && <p className="heat-syllabus-line">{draft.line}</p>}
      {course && (
        <p className="heat-syllabus-course">
          <strong>{course.label || courseLabel(course)}</strong>
          {course.isNew && <span className="heat-tag">{copy.syllabus.newCourse}</span>}
          <span data-text="secondary">{[course.term, draft.fileName].filter(Boolean).join(' · ')}</span>
        </p>
      )}

      <fieldset className="heat-set">
        <legend data-text="secondary">{copy.syllabus.weights}</legend>
        {weights.length === 0 ? (
          <p className="why" data-text="secondary">
            {copy.syllabus.noWeights}
          </p>
        ) : (
          <table className="heat-syllabus-table">
            <tbody>
              {weights.map((w) => (
                <tr key={w.category}>
                  <th scope="row">{w.category}</th>
                  <td>{pctText(w.percent)}%</td>
                  <td data-text="secondary">{w.dropLowest ? copy.syllabus.dropsLowest(w.dropLowest) : ''}</td>
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr>
                <th scope="row" data-text="secondary">
                  {copy.syllabus.total(pctText(draft.weightsTotal ?? weights.reduce((sum, w) => sum + w.percent, 0)))}
                </th>
                <td />
                <td />
              </tr>
            </tfoot>
          </table>
        )}
        {draft.weightsFlag && (
          <p className="heat-syllabus-flag" role="alert">
            {draft.weightsFlag}
          </p>
        )}
      </fieldset>

      {types.length > 0 && (
        <fieldset className="heat-set">
          <legend data-text="secondary">{copy.syllabus.types}</legend>
          <table className="heat-syllabus-table">
            <tbody>
              {types.map((t) => (
                <tr key={t.name}>
                  <th scope="row">{t.name}</th>
                  <td>{t.estMin === null ? '—' : formatMinutes(t.estMin)}</td>
                  <td data-text="secondary">{t.difficulty === null ? '' : `difficulty ${t.difficulty}`}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </fieldset>
      )}

      {newTasks.length > 0 && (
        <fieldset className="heat-set">
          <legend data-text="secondary">{copy.syllabus.newTasks}</legend>
          <table className="heat-syllabus-table">
            <tbody>
              {newTasks.map((t, i) => (
                <tr key={`${t.title}-${i}`}>
                  <th scope="row">{t.title}</th>
                  <td>{when(t.due)}</td>
                  <td data-text="secondary">
                    {t.type} · {formatMinutes(t.estMin)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </fieldset>
      )}

      {dateChanges.length > 0 && (
        <fieldset className="heat-set">
          <legend data-text="secondary">{copy.syllabus.dateChanges}</legend>
          <table className="heat-syllabus-table">
            <tbody>
              {dateChanges.map((c) => (
                <tr key={c.taskId}>
                  <th scope="row">{c.title}</th>
                  <td colSpan={2}>
                    {when(c.from)} → {when(c.to)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </fieldset>
      )}

      <p className="why" data-text="secondary">
        {copy.syllabus.nothingBefore}
      </p>
      <div className="note-actions">
        <button ref={accept} type="submit" className="gel" disabled={busy}>
          {copy.syllabus.accept}
        </button>
        <button type="button" className="gel plain" disabled={busy} onClick={() => void discard()}>
          {copy.syllabus.discard}
        </button>
      </div>
    </form>
  );
}
