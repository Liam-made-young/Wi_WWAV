// One course (docs/SPEC.md 3.1, 3.8): code and name, its percentage and a
// letter pill, "Based on 65% of the course so far", the weights warning when
// the weights don't add up, yellow banners for grades waiting for a score,
// "What it would take", the items by category, and the course's Public
// switch. The snapshot carries every number; the card writes them out.

import { type FormEvent, useEffect, useRef, useState } from 'react';
import type { Course, Grade, GradeCategory, Id } from '../client';
import { openExternal } from '../external';
import { copy, sentenceOf } from '../fmt';
import { useSheets } from '../frame';
import { PublicSwitch } from '../public/PublicSwitch';
import { useHeat } from '../store';
import { CourseSheet } from './CourseSheet';
import { GradeSheet } from './GradeSheet';
import { pctText, pillTone } from './format';
import { DEFAULT_SCALE } from '../model/grades';

export function CourseCard({
  course,
  scoring,
  onScoring,
}: {
  course: Course;
  scoring: Id | null;
  onScoring(id: Id | null): void;
}) {
  const { snap } = useHeat();
  const { openSheet, closeSheet } = useSheets();
  const [open, setOpen] = useState<Id | null>(null);
  if (!snap) return null;

  const d = snap.derived.courses[course.id];
  const grades = snap.records.grade.filter((g) => g.courseId === course.id);
  const waiting = grades.filter((g) => g.pending);
  const known = new Set(course.categories.map((c) => c.id));
  const loose = grades.filter((g) => !g.pending && (g.categoryId === null || !known.has(g.categoryId)));

  return (
    <article className="heat-course" aria-label={`${course.code}, ${course.name}`}>
      <header className="heat-course-head">
        <div className="heat-course-name">
          <h2 className="heat-course-code">{course.code}</h2>
          <span>{course.name}</span>
        </div>
        <div className="heat-course-mark">
          <span className="heat-course-pct">{d?.currentPct == null ? '—' : `${pctText(d.currentPct)}%`}</span>
          {d?.letter && (
            <span className="heat-letter" data-tone={pillTone(d.letter)} title={`Letter ${d.letter}`}>
              {d.letter}
            </span>
          )}
        </div>
      </header>
      <p className="heat-course-based" data-text="secondary">
        {d && d.decidedPct > 0 ? copy.grades.basedOn(pctText(d.decidedPct)) : copy.gradesUi.nothingGraded}
      </p>
      {d?.weights && (
        <p className="heat-course-weights" data-text="secondary" role="status">
          {d.weights}
        </p>
      )}

      {waiting.map((g) => (
        <PendingBanner
          key={g.id}
          grade={g}
          category={course.categories.find((c) => c.id === g.categoryId)}
          scoring={scoring === g.id}
          onScoring={(on) => onScoring(on ? g.id : null)}
        />
      ))}

      <WhatItWouldTake course={course} />

      <div className="heat-cats">
        {course.categories.map((c) => (
          <Category
            key={c.id}
            category={c}
            pct={d?.categories?.[c.id] ?? null}
            items={grades.filter((g) => !g.pending && g.categoryId === c.id)}
            open={open}
            onOpen={setOpen}
          />
        ))}
        {loose.length > 0 && <Category category={null} pct={null} items={loose} open={open} onOpen={setOpen} />}
      </div>

      <footer className="heat-course-foot">
        <PublicSwitch kind="course" id={course.id} on={course.public === true} name={course.code} />
        <button
          type="button"
          className="gel"
          onClick={() => openSheet(<CourseSheet course={course} onClose={closeSheet} />)}
        >
          Edit course…
        </button>
      </footer>
    </article>
  );
}

// --- the yellow banner ------------------------------------------------------------

/**
 * A grade posted without a score. It is scored by typing the score here and
 * by nothing else: Claude can record the notice but never the number (3.8).
 */
function PendingBanner({
  grade,
  category,
  scoring,
  onScoring,
}: {
  grade: Grade;
  category: GradeCategory | undefined;
  scoring: boolean;
  onScoring(on: boolean): void;
}) {
  const { client, act, say } = useHeat();
  const { openSheet, closeSheet } = useSheets();
  const [score, setScore] = useState('');
  const [outOf, setOutOf] = useState(String(grade.outOf));
  const [why, setWhy] = useState<string | null>(null);
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (scoring) field.current?.focus();
  }, [scoring]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const n = score.trim() === '' ? NaN : Number(score);
    const total = Number(outOf);
    if (!Number.isFinite(n) || n < 0) return setWhy(copy.gradesUi.typeScore);
    if (!Number.isFinite(total) || total <= 0) return setWhy(copy.gradesUi.typeOutOf);
    // A pending grade's "out of" starts at 100 until the real one is typed with the score.
    if (total !== grade.outOf && !(await act(client.patch('grade', grade.id, { outOf: total })))) return;
    if (await act(client.score(grade.id, n))) {
      say(copy.gradesUi.entered(grade.title, n, total));
      setScore('');
      setWhy(null);
      onScoring(false);
    }
  };

  return (
    <div className="heat-sticky" role="group" aria-label={`${copy.grades.newGradePosted}: ${grade.title}`}>
      <div className="heat-sticky-line">
        <span className="heat-sticky-text">
          <strong>{copy.grades.newGradePosted}</strong>
          {category ? ` · ${category.name}` : ''}
          <span className="heat-sticky-title">{grade.title}</span>
        </span>
        {grade.link && (
          <button
            type="button"
            className="heat-link"
            data-dense
            onClick={() => openExternal(grade.link!)}
            title="Opens Brightspace in your browser"
          >
            Open in Brightspace
          </button>
        )}
        <button
          type="button"
          className="gel plain"
          aria-label={`Edit grade: ${grade.title}`}
          onClick={() => openSheet(<GradeSheet grade={grade} onClose={closeSheet} />)}
        >
          Edit…
        </button>
        <button
          type="button"
          className="gel"
          aria-expanded={scoring}
          aria-label={`${copy.grades.enterScore}: ${grade.title}`}
          onClick={() => onScoring(!scoring)}
        >
          {copy.grades.enterScore}
        </button>
      </div>
      {scoring && (
        <form className="heat-sticky-form" noValidate onSubmit={(e) => void submit(e)}>
          <label className="heat-sticky-field">
            <span data-text="secondary">Score</span>
            <input
              ref={field}
              type="number"
              inputMode="decimal"
              step="any"
              min={0}
              value={score}
              aria-label={`Score for ${grade.title}`}
              onChange={(e) => {
                setScore(e.target.value);
                setWhy(null);
              }}
            />
          </label>
          <label className="heat-sticky-field">
            <span data-text="secondary">Out of</span>
            <input
              type="number"
              inputMode="decimal"
              step="any"
              min={1}
              value={outOf}
              aria-label={`${grade.title} is out of`}
              onChange={(e) => {
                setOutOf(e.target.value);
                setWhy(null);
              }}
            />
          </label>
          <button type="submit" className="gel">
            {copy.grades.save}
          </button>
          <button type="button" className="gel plain" onClick={() => onScoring(false)}>
            Cancel
          </button>
          <p className="heat-sticky-why" data-text="secondary" role="status">
            {why}
          </p>
        </form>
      )}
    </div>
  );
}

// --- what it would take -----------------------------------------------------------

/** "To finish with a B (83%), you need 78.4% on the remaining 35%." The core words it. */
function WhatItWouldTake({ course }: { course: Course }) {
  const { client, snap } = useHeat();
  const scale = course.scale ?? DEFAULT_SCALE;
  const d = snap?.derived.courses[course.id];
  // Aim one step above where the course stands, or at the top.
  const above = () => {
    const at = scale.findIndex((s) => s.letter === d?.letter);
    return (scale[Math.max(0, at - 1)] ?? scale[0])?.letter ?? 'A';
  };
  const [letter, setLetter] = useState(above);
  const [text, setText] = useState('');
  const aim = scale.some((s) => s.letter === letter) ? letter : (scale[0]?.letter ?? letter);

  // Asked again whenever what it rests on changes.
  const rests = `${d?.currentPct}|${d?.decidedPct}|${scale.map((s) => `${s.letter}${s.min}`).join()}`;
  useEffect(() => {
    let live = true;
    client.whatItWouldTake(course.id, aim).then(
      (r) => live && setText(r.text),
      (e) => live && setText(sentenceOf(e)),
    );
    return () => {
      live = false;
    };
  }, [client, course.id, aim, rests]);

  return (
    <div className="heat-take" role="group" aria-label={copy.gradesUi.whatItWouldTake}>
      <label className="heat-take-aim">
        <span data-text="secondary">{copy.gradesUi.aimFor}</span>
        <select
          value={aim}
          onChange={(e) => setLetter(e.target.value)}
          aria-label={`${copy.gradesUi.aimFor} in ${course.code}`}
        >
          {scale.map((s) => (
            <option key={s.letter} value={s.letter}>
              {s.letter}
            </option>
          ))}
        </select>
      </label>
      <p className="heat-take-text" role="status" aria-live="polite">
        {text}
      </p>
    </div>
  );
}

// --- a category and its items -----------------------------------------------------

function Category({
  category,
  pct,
  items,
  open,
  onOpen,
}: {
  category: GradeCategory | null;
  pct: number | null;
  items: Grade[];
  open: Id | null;
  onOpen(id: Id | null): void;
}) {
  const name = category?.name ?? copy.gradesUi.noCategory;
  return (
    <section className="heat-cat" aria-label={name}>
      <h3 className="heat-cat-head" data-text="secondary">
        <span className="heat-cat-name">{name}</span>
        {category && <span>{pctText(category.weight)}% of the course</span>}
        {category && <span>{pct === null ? 'Not graded yet' : `${pctText(pct)}% so far`}</span>}
      </h3>
      {items.length === 0 ? (
        <p className="heat-cat-empty" data-text="secondary">
          {copy.gradesUi.noItems}
        </p>
      ) : (
        <ul className="heat-grade-list">
          {items.map((g) => (
            <GradeRow key={g.id} grade={g} open={open === g.id} onToggle={() => onOpen(open === g.id ? null : g.id)} />
          ))}
        </ul>
      )}
    </section>
  );
}

function GradeRow({ grade, open, onToggle }: { grade: Grade; open: boolean; onToggle(): void }) {
  const { openSheet, closeSheet } = useSheets();
  return (
    <li className="heat-grade" data-dropped={grade.dropped} data-open={open}>
      <button type="button" className="heat-grade-main" data-dense aria-expanded={open} onClick={onToggle}>
        <span className="heat-grade-title">{grade.title}</span>
        {grade.dropped && <span className="heat-tag">Dropped</span>}
        {grade.public && <span className="heat-tag">Public</span>}
        <span className="heat-grade-score">
          {grade.score ?? '—'} / {grade.outOf}
        </span>
      </button>
      {open && (
        <div className="heat-grade-more">
          <PublicSwitch kind="grade" id={grade.id} on={grade.public === true} name={grade.title} />
          <button
            type="button"
            className="gel plain"
            onClick={() => openSheet(<GradeSheet grade={grade} onClose={closeSheet} />)}
          >
            Edit grade…
          </button>
        </div>
      )}
    </li>
  );
}
