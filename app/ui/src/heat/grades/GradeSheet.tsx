// A grade (docs/SPEC.md 3.8): "+" on Grades adds one, and a grade's row opens
// it again. Leave the score empty and the grade is waiting for one, with its
// yellow banner; a typed score makes it a plain grade. A grade waiting for
// its score has no score field here: it takes the score from its banner,
// typed, and by nothing else. With no category named, the core files a new
// grade by the course's keywords.

import { type FormEvent, useEffect, useRef, useState } from 'react';
import type { Grade, Id } from '../client';
import { copy, courseLabel } from '../fmt';
import { useDraft } from '../frame';
import { useHeat } from '../store';

interface Form {
  courseId: Id;
  title: string;
  /** '' lets the core choose from the keywords; 'none' files it under no category. */
  category: string;
  score: string;
  outOf: string;
  dropped: boolean;
}

export function GradeSheet({ grade, courseId, onClose }: { grade?: Grade; courseId?: Id; onClose(): void }) {
  const { snap, client, act, say } = useHeat();
  const courses = snap?.records.course ?? [];
  const waiting = grade?.pending === true;

  const [initial] = useState<Form>(() => ({
    courseId: grade?.courseId ?? courseId ?? courses[0]?.id ?? '',
    title: grade?.title ?? '',
    category: grade ? (grade.categoryId ?? 'none') : '',
    score: grade?.score === null || grade?.score === undefined ? '' : String(grade.score),
    outOf: String(grade?.outOf ?? 100),
    dropped: grade?.dropped ?? false,
  }));
  const [form, setForm, clear] = useDraft<Form>(`grade:${grade?.id ?? 'new'}`, initial);
  const [why, setWhy] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const title = useRef<HTMLInputElement>(null);
  useEffect(() => title.current?.focus(), []);

  const course = courses.find((c) => c.id === form.courseId);
  const set = (change: Partial<Form>) => {
    setForm({ ...form, ...change });
    setWhy(null);
  };

  const save = async (e: FormEvent) => {
    e.preventDefault();
    const name = form.title.trim();
    if (!name) {
      setWhy(copy.gradesUi.gradeNeedsName);
      title.current?.focus();
      return;
    }
    if (!course) return setWhy(copy.gradesUi.addCourseFirst);
    const total = Number(form.outOf);
    if (!Number.isFinite(total) || total <= 0) return setWhy(copy.gradesUi.typeOutOf);
    const typed = form.score.trim() === '' ? null : Number(form.score);
    if (!waiting && typed !== null && (!Number.isFinite(typed) || typed < 0)) return setWhy(copy.gradesUi.typeScore);

    setBusy(true);
    const categoryId = form.category === 'none' ? null : form.category === '' ? undefined : form.category;
    const r = grade
      ? await act(
          client.patch('grade', grade.id, {
            courseId: course.id,
            title: name,
            categoryId: categoryId === undefined ? null : categoryId,
            outOf: total,
            dropped: form.dropped,
            // A grade waiting for its score is scored from its banner, never from here.
            ...(waiting ? {} : { score: typed }),
          }),
        )
      : await act(
          client.put('grade', {
            courseId: course.id,
            // Undefined is "choose for me": the core files it by the course's keywords.
            categoryId: categoryId as Id | null,
            title: name,
            score: typed,
            outOf: total,
            dropped: false,
            pending: typed === null,
            source: 'you',
          }),
        );
    setBusy(false);
    if (!r) return;
    clear();
    say(grade ? `Saved ${name}.` : typed === null ? `Added ${name}, waiting for its score.` : `Added ${name}.`);
    onClose();
  };

  const remove = async () => {
    if (!grade) return;
    if (!(await act(client.delete('grade', grade.id)))) return;
    clear();
    say(`Deleted ${grade.title}.`);
    onClose();
  };

  return (
    <form
      noValidate
      className="sheet heat-sheet"
      role="dialog"
      aria-label={grade ? `Edit grade ${grade.title}` : 'Add grade'}
      onSubmit={(e) => void save(e)}
    >
      <h2 className="sheet-title">{grade ? 'Edit grade' : 'Add grade'}</h2>
      <label className="field">
        <span data-text="secondary">Grade</span>
        <input
          ref={title}
          value={form.title}
          onChange={(e) => set({ title: e.target.value })}
          placeholder="Grammar quiz 4"
        />
      </label>
      <div className="heat-sheet-grid">
        <label className="field">
          <span data-text="secondary">Course</span>
          <select
            value={form.courseId}
            onChange={(e) => set({ courseId: e.target.value, category: grade ? 'none' : '' })}
          >
            {courses.map((c) => (
              <option key={c.id} value={c.id}>
                {snap?.derived.courses[c.id]?.label ?? courseLabel(c)}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span data-text="secondary">Category</span>
          <select value={form.category} onChange={(e) => set({ category: e.target.value })}>
            {!grade && <option value="">Choose from the keywords</option>}
            <option value="none">{copy.gradesUi.noCategory}</option>
            {course?.categories.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        {waiting ? (
          <p className="why heat-grade-waiting" data-text="secondary">
            This grade is waiting for its score. Type it in the yellow banner on the course.
          </p>
        ) : (
          <label className="field">
            <span data-text="secondary">Score</span>
            <input
              type="number"
              inputMode="decimal"
              step="any"
              min={0}
              value={form.score}
              placeholder={grade ? '' : 'Leave empty if not posted'}
              onChange={(e) => set({ score: e.target.value })}
            />
          </label>
        )}
        <label className="field">
          <span data-text="secondary">Out of</span>
          <input
            type="number"
            inputMode="decimal"
            step="any"
            min={1}
            value={form.outOf}
            onChange={(e) => set({ outOf: e.target.value })}
          />
        </label>
      </div>
      {grade && (
        <label className="check" data-dense>
          <input type="checkbox" checked={form.dropped} onChange={(e) => set({ dropped: e.target.checked })} />
          Dropped: it doesn’t count toward the course
        </label>
      )}
      <p className="why heat-sheet-why" data-text="secondary" role="status">
        {why}
      </p>
      <div className="note-actions">
        <button type="submit" className="gel" disabled={busy}>
          {grade ? 'Save' : copy.gradesUi.addGrade}
        </button>
        <button
          type="button"
          className="gel plain"
          onClick={() => {
            clear();
            onClose();
          }}
        >
          Cancel
        </button>
        {grade && (
          <button type="button" className="gel plain" onClick={() => void remove()}>
            Delete grade
          </button>
        )}
      </div>
    </form>
  );
}
