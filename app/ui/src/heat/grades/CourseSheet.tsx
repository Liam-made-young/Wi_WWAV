// The course editor (docs/SPEC.md 3.8): code, name, categories with their
// weights and keywords, the assignment types, the scale, and notes. If the
// weights don't add up it says so in a line as they are typed: "Weights add
// to 95%. The other 5% is unassigned." The keywords are what file a new grade
// into a category (they replace Heat's hard-coded rules). A type gives the
// course's tasks their minutes and difficulty, picked by the words in a
// title; saving applies the types again to the tasks nothing was set by hand
// on. Esc keeps what was typed for next time; Cancel throws it away. One Save
// is one undo step.

import { type FormEvent, useEffect, useRef, useState } from 'react';
import type { Course, GradeCategory, Id, LetterStep, TypeDef } from '../client';
import { copy } from '../fmt';
import { useDraft } from '../frame';
import { DEFAULT_SCALE } from '../model/grades';
import { useHeat } from '../store';
import { draftWeights, termNameFor } from './format';

interface CategoryRow {
  key: string;
  id?: Id;
  name: string;
  weight: string;
  keywords: string;
}

/** One assignment type as it is typed: the words comma-separated, the numbers as text, '' for none. */
interface TypeRow {
  key: string;
  name: string;
  patterns: string;
  estMin: string;
  difficulty: string;
  category: string;
}

interface Form {
  code: string;
  name: string;
  /** An existing term's id, or '' for a new one named in `newTerm`. */
  termId: string;
  newTerm: string;
  categories: CategoryRow[];
  types: TypeRow[];
  ownScale: boolean;
  scale: { key: string; letter: string; min: string }[];
  notes: string;
}

let rowCount = 0;
const rowKey = () => `row-${(rowCount += 1)}`;
const newCategoryId = () => `cat-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;

const rowOf = (c: GradeCategory): CategoryRow => ({
  key: rowKey(),
  id: c.id,
  name: c.name,
  weight: String(c.weight),
  keywords: c.keywords.join(', '),
});

const typeRowOf = (t: TypeDef): TypeRow => ({
  key: rowKey(),
  name: t.name,
  patterns: t.patterns.join(', '),
  estMin: t.estMin === null ? '' : String(t.estMin),
  difficulty: t.difficulty === null ? '' : String(t.difficulty),
  category: t.category ?? '',
});

const stepsOf = (scale: readonly LetterStep[]) =>
  scale.map((s) => ({ key: rowKey(), letter: s.letter, min: String(s.min) }));

/** "exit ticket, Edfinity" or one per line: the words, trimmed, none empty. */
const words = (text: string) =>
  text
    .split(/[,\n]/)
    .map((w) => w.trim())
    .filter(Boolean);

export function CourseSheet({ course, onClose }: { course?: Course; onClose(): void }) {
  const { snap, client, act, say, date } = useHeat();
  const terms = snap?.records.term ?? [];
  const current = terms.at(-1);

  // Built once, so the rows keep their keys until something is typed.
  const [initial] = useState<Form>(() =>
    course
      ? {
          code: course.code,
          name: course.name,
          termId: course.termId,
          newTerm: '',
          categories: course.categories.map(rowOf),
          types: (course.types ?? []).map(typeRowOf),
          ownScale: course.scale !== undefined && course.scale !== null,
          scale: stepsOf(course.scale ?? DEFAULT_SCALE),
          notes: course.notes,
        }
      : {
          code: '',
          name: '',
          termId: current?.id ?? '',
          newTerm: termNameFor(date),
          categories: [],
          types: [],
          ownScale: false,
          scale: stepsOf(DEFAULT_SCALE),
          notes: '',
        },
  );
  const [form, setForm, clear] = useDraft<Form>(`course:${course?.id ?? 'new'}`, initial);
  const [why, setWhy] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const code = useRef<HTMLInputElement>(null);
  useEffect(() => code.current?.focus(), []);

  const set = (change: Partial<Form>) => {
    setForm({ ...form, ...change });
    setWhy(null);
  };
  const setRow = (key: string, change: Partial<CategoryRow>) =>
    set({ categories: form.categories.map((r) => (r.key === key ? { ...r, ...change } : r)) });
  const setType = (key: string, change: Partial<TypeRow>) =>
    set({ types: form.types.map((t) => (t.key === key ? { ...t, ...change } : t)) });
  const setStep = (key: string, change: Partial<Form['scale'][number]>) =>
    set({ scale: form.scale.map((s) => (s.key === key ? { ...s, ...change } : s)) });

  const total = form.categories.reduce((sum, r) => sum + (Number(r.weight) || 0), 0);
  const weights = form.categories.length > 0 ? draftWeights(total) : null;
  const saved = course ? snap?.derived.courses[course.id]?.weights : null;

  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (!form.code.trim()) {
      setWhy(copy.gradesUi.courseNeedsCode);
      code.current?.focus();
      return;
    }
    if (form.categories.some((r) => !r.name.trim())) return setWhy(copy.gradesUi.categoryNeedsName);
    if (form.types.some((t) => !t.name.trim())) return setWhy(copy.types.needsName);
    if (form.types.some((t) => t.estMin.trim() !== '' && !(Number(t.estMin) >= 5 && Number(t.estMin) <= 600))) {
      return setWhy(copy.types.minutesRange);
    }
    const steps = form.scale.map((s) => ({ letter: s.letter.trim(), min: Number(s.min) }));
    if (form.ownScale) {
      if (
        steps.some((s) => !s.letter || s.min.toString() === '' || !Number.isFinite(s.min) || s.min < 0 || s.min > 100)
      ) {
        return setWhy(copy.gradesUi.scaleNeedsLetters);
      }
      if (steps.some((s, i) => i > 0 && s.min > steps[i - 1].min)) return setWhy(copy.gradesUi.scaleOrder);
    }
    setBusy(true);
    let termId = form.termId;
    if (!termId) {
      const made = await act(client.put('term', { name: form.newTerm.trim() || termNameFor(date) }));
      if (!made) return setBusy(false);
      termId = made.record.id;
    }
    // The sheet has no field for how many of a category's lowest scores are dropped: what a
    // syllabus set stays as it is.
    const dropped = (id?: Id) => course?.categories.find((c) => c.id === id)?.dropLowest;
    const categories: GradeCategory[] = form.categories.map((r) => ({
      id: r.id ?? newCategoryId(),
      name: r.name.trim(),
      weight: Number(r.weight) || 0,
      keywords: words(r.keywords),
      ...(dropped(r.id) ? { dropLowest: dropped(r.id) } : {}),
    }));
    // A type's category is kept by name, and only while the course still has a category of that name.
    const types: TypeDef[] = form.types.map((t) => ({
      name: t.name.trim(),
      patterns: words(t.patterns).map((w) => w.toLowerCase()),
      estMin: t.estMin.trim() === '' ? null : Math.round(Number(t.estMin)),
      difficulty: t.difficulty === '' ? null : Number(t.difficulty),
      ...(categories.some((c) => c.name === t.category) ? { category: t.category } : {}),
    }));
    const fields = {
      termId,
      code: form.code.trim(),
      name: form.name.trim(),
      categories,
      notes: form.notes,
      // A course that never had types keeps none, so a save that didn't touch them changes nothing there.
      ...(types.length > 0 || course?.types ? { types } : {}),
    };
    // An existing course is saved through its own command, which applies its types to its tasks again.
    const updated = course
      ? await act(client.updateCourse(course.id, { ...fields, scale: form.ownScale ? steps : (null as never) }))
      : null;
    const added = course
      ? null
      : await act(client.put('course', { ...fields, ...(form.ownScale ? { scale: steps } : {}), public: false }));
    setBusy(false);
    if (!updated && !added) return;
    clear();
    const retimed = updated?.retimed ?? 0;
    say(
      course
        ? `Saved ${fields.code}.${retimed > 0 ? ` ${copy.types.retimed(retimed)}.` : ''}`
        : `Added ${fields.code}.`,
    );
    onClose();
  };

  const remove = async () => {
    if (!course) return;
    const r = await act(client.delete('course', course.id));
    if (!r) return;
    clear();
    say(`Deleted ${course.code}.`);
    onClose();
  };

  return (
    <form
      noValidate
      className="sheet heat-sheet heat-sheet-wide"
      role="dialog"
      aria-label={course ? `Edit course ${course.code}` : 'Add course'}
      onSubmit={(e) => void save(e)}
    >
      <h2 className="sheet-title">{course ? `Edit ${course.code}` : 'Add course'}</h2>
      <div className="heat-sheet-grid">
        <label className="field">
          <span data-text="secondary">Code</span>
          <input ref={code} value={form.code} onChange={(e) => set({ code: e.target.value })} placeholder="JPN 201" />
        </label>
        <label className="field">
          <span data-text="secondary">Name</span>
          <input
            value={form.name}
            onChange={(e) => set({ name: e.target.value })}
            placeholder="Intermediate Japanese"
          />
        </label>
        {terms.length > 0 ? (
          <label className="field">
            <span data-text="secondary">Term</span>
            <select value={form.termId} onChange={(e) => set({ termId: e.target.value })}>
              {terms.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
              <option value="">A new term…</option>
            </select>
          </label>
        ) : null}
        {form.termId === '' && (
          <label className="field">
            <span data-text="secondary">{terms.length > 0 ? 'New term' : 'Term'}</span>
            <input value={form.newTerm} onChange={(e) => set({ newTerm: e.target.value })} />
          </label>
        )}
      </div>

      <fieldset className="heat-set">
        <legend data-text="secondary">Categories</legend>
        <p className="why" data-text="secondary">
          {copy.gradesUi.weightsHint} {copy.gradesUi.keywordsHint}
        </p>
        <div className="heat-cat-rows">
          {form.categories.map((r, i) => (
            <div key={r.key} className="heat-cat-row">
              <label className="field">
                <span data-text="secondary">Name</span>
                <input
                  value={r.name}
                  aria-label={`Category ${i + 1} name`}
                  onChange={(e) => setRow(r.key, { name: e.target.value })}
                />
              </label>
              <label className="field">
                <span data-text="secondary">Weight %</span>
                <input
                  type="number"
                  min={0}
                  max={100}
                  step="any"
                  value={r.weight}
                  aria-label={`Category ${i + 1} weight`}
                  onChange={(e) => setRow(r.key, { weight: e.target.value })}
                />
              </label>
              <label className="field heat-cat-keywords">
                <span data-text="secondary">Keywords</span>
                <input
                  value={r.keywords}
                  aria-label={`Category ${i + 1} keywords`}
                  placeholder="exit ticket, Edfinity"
                  onChange={(e) => setRow(r.key, { keywords: e.target.value })}
                />
              </label>
              <button
                type="button"
                className="gel plain heat-cat-remove"
                aria-label={`Remove category ${i + 1}`}
                onClick={() => set({ categories: form.categories.filter((x) => x.key !== r.key) })}
              >
                Remove
              </button>
            </div>
          ))}
        </div>
        <button
          type="button"
          className="gel"
          onClick={() =>
            set({ categories: [...form.categories, { key: rowKey(), name: '', weight: '', keywords: '' }] })
          }
        >
          Add category
        </button>
        <p className="heat-weights" data-text="secondary" role="status">
          {weights ?? (course ? saved : null)}
        </p>
      </fieldset>

      <fieldset className="heat-set">
        <legend data-text="secondary">{copy.types.heading}</legend>
        <p className="why" data-text="secondary">
          {copy.types.hint}
        </p>
        <div className="heat-cat-rows">
          {form.types.map((t, i) => (
            <div key={t.key} className="heat-type-row">
              <label className="field">
                <span data-text="secondary">Name</span>
                <input
                  value={t.name}
                  aria-label={`Type ${i + 1} name`}
                  placeholder="Lab"
                  onChange={(e) => setType(t.key, { name: e.target.value })}
                />
              </label>
              <label className="field">
                <span data-text="secondary">Words</span>
                <input
                  value={t.patterns}
                  aria-label={`Type ${i + 1} words`}
                  placeholder="lab, lab report"
                  onChange={(e) => setType(t.key, { patterns: e.target.value })}
                />
              </label>
              <label className="field">
                <span data-text="secondary">Minutes</span>
                <input
                  type="number"
                  min={5}
                  max={600}
                  value={t.estMin}
                  aria-label={`Type ${i + 1} minutes`}
                  onChange={(e) => setType(t.key, { estMin: e.target.value })}
                />
              </label>
              <label className="field">
                <span data-text="secondary">Difficulty</span>
                <select
                  value={t.difficulty}
                  aria-label={`Type ${i + 1} difficulty`}
                  onChange={(e) => setType(t.key, { difficulty: e.target.value })}
                >
                  <option value="">—</option>
                  {[1, 2, 3, 4, 5].map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span data-text="secondary">Category</span>
                <select
                  value={form.categories.some((c) => c.name.trim() === t.category) ? t.category : ''}
                  aria-label={`Type ${i + 1} category`}
                  onChange={(e) => setType(t.key, { category: e.target.value })}
                >
                  <option value="">{copy.types.noCategory}</option>
                  {form.categories
                    .filter((c) => c.name.trim())
                    .map((c) => (
                      <option key={c.key} value={c.name.trim()}>
                        {c.name.trim()}
                      </option>
                    ))}
                </select>
              </label>
              <button
                type="button"
                className="gel plain heat-cat-remove"
                aria-label={`Remove type ${i + 1}`}
                onClick={() => set({ types: form.types.filter((x) => x.key !== t.key) })}
              >
                Remove
              </button>
            </div>
          ))}
        </div>
        <button
          type="button"
          className="gel"
          onClick={() =>
            set({
              types: [...form.types, { key: rowKey(), name: '', patterns: '', estMin: '', difficulty: '', category: '' }],
            })
          }
        >
          {copy.types.add}
        </button>
      </fieldset>

      <fieldset className="heat-set">
        <legend data-text="secondary">Scale</legend>
        <label className="check" data-dense>
          <input type="checkbox" checked={form.ownScale} onChange={(e) => set({ ownScale: e.target.checked })} />
          {copy.gradesUi.scaleOwn}
        </label>
        {form.ownScale ? (
          <div className="heat-scale">
            {form.scale.map((s, i) => (
              <div key={s.key} className="heat-scale-row">
                <input
                  value={s.letter}
                  aria-label={`Step ${i + 1} letter`}
                  onChange={(e) => setStep(s.key, { letter: e.target.value })}
                />
                <span data-text="secondary">from</span>
                <input
                  type="number"
                  min={0}
                  max={100}
                  step="any"
                  value={s.min}
                  aria-label={`Step ${i + 1} from percent`}
                  onChange={(e) => setStep(s.key, { min: e.target.value })}
                />
                <span data-text="secondary">%</span>
                <button
                  type="button"
                  className="gel plain"
                  aria-label={`Remove step ${i + 1}`}
                  onClick={() => set({ scale: form.scale.filter((x) => x.key !== s.key) })}
                >
                  Remove
                </button>
              </div>
            ))}
            <button
              type="button"
              className="gel"
              onClick={() => set({ scale: [...form.scale, { key: rowKey(), letter: '', min: '' }] })}
            >
              Add step
            </button>
          </div>
        ) : (
          <p className="why" data-text="secondary">
            {copy.gradesUi.noScale}
          </p>
        )}
      </fieldset>

      <label className="field">
        <span data-text="secondary">Notes</span>
        <textarea rows={2} value={form.notes} onChange={(e) => set({ notes: e.target.value })} />
      </label>

      <p className="why heat-sheet-why" data-text="secondary" role="status">
        {why}
      </p>
      <div className="note-actions">
        <button type="submit" className="gel" disabled={busy}>
          {course ? 'Save' : 'Add course'}
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
        {course && (
          <button type="button" className="gel plain" onClick={() => void remove()}>
            Delete course
          </button>
        )}
      </div>
    </form>
  );
}
