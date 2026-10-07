// Grades (docs/SPEC.md 3.1 and 3.8). The maths stays exactly as Heat has it:
//
//   category % = Σ score / Σ outOf   (items not dropped, outOf > 0, scored)
//   current %  = Σ (weight × category %) / Σ weight   (graded categories only)
//   decided %  = graded weight / total weight × 100
//   letter     = ≥93 A, ≥90 A-, ≥87 B+, ≥83 B, ≥80 B-, ≥77 C+, ≥73 C, ≥70 C-, ≥67 D+, ≥60 D, else F
//
// "What it would take" is plain arithmetic on the same numbers: the final
// grade is current % × decided share + x × the remaining share.

import * as copy from './copy';
import type { Course, Grade, GradeCategory, Id, LetterStep, Term } from './records';

export const DEFAULT_SCALE: LetterStep[] = [
  { letter: 'A', min: 93 },
  { letter: 'A-', min: 90 },
  { letter: 'B+', min: 87 },
  { letter: 'B', min: 83 },
  { letter: 'B-', min: 80 },
  { letter: 'C+', min: 77 },
  { letter: 'C', min: 73 },
  { letter: 'C-', min: 70 },
  { letter: 'D+', min: 67 },
  { letter: 'D', min: 60 },
  { letter: 'F', min: 0 },
];

const scaleOf = (course: Course) => course.scale ?? DEFAULT_SCALE;

const counts = (g: Grade) => !g.dropped && g.outOf > 0 && g.score !== null;

export function categoryPct(categoryId: Id, grades: readonly Grade[]): number | null {
  const items = grades.filter((g) => g.categoryId === categoryId && counts(g));
  if (items.length === 0) return null;
  const score = items.reduce((sum, g) => sum + g.score!, 0);
  const outOf = items.reduce((sum, g) => sum + g.outOf, 0);
  return (score / outOf) * 100;
}

function graded(course: Course, grades: readonly Grade[]): { category: GradeCategory; pct: number }[] {
  const own = grades.filter((g) => g.courseId === course.id);
  return course.categories.flatMap((category) => {
    const pct = categoryPct(category.id, own);
    return pct === null ? [] : [{ category, pct }];
  });
}

const totalWeight = (course: Course) => course.categories.reduce((sum, c) => sum + c.weight, 0);

export function currentPct(course: Course, grades: readonly Grade[]): number | null {
  const g = graded(course, grades);
  const weight = g.reduce((sum, x) => sum + x.category.weight, 0);
  if (weight <= 0) return null;
  return g.reduce((sum, x) => sum + x.category.weight * x.pct, 0) / weight;
}

export function decidedPct(course: Course, grades: readonly Grade[]): number {
  const total = totalWeight(course);
  if (total <= 0) return 0;
  return (graded(course, grades).reduce((sum, x) => sum + x.category.weight, 0) / total) * 100;
}

export function letterFor(pct: number, scale: readonly LetterStep[] = DEFAULT_SCALE): string {
  return (scale.find((s) => pct >= s.min) ?? scale[scale.length - 1]).letter;
}

/** Letter pills are green for A, blue for B, amber for C and red for D or F. */
export function pillColour(letter: string): 'green' | 'blue' | 'amber' | 'red' | 'grey' {
  switch (letter[0]) {
    case 'A':
      return 'green';
    case 'B':
      return 'blue';
    case 'C':
      return 'amber';
    case 'D':
    case 'F':
      return 'red';
    default:
      return 'grey';
  }
}

// "83", "78.4": a percentage to one decimal, without a trailing ".0".
const pct = (x: number) => String(Math.round(x * 10) / 10);
// What you need rounds up and the best you can do rounds down, so neither flatters.
const pctUp = (x: number) => String(Math.ceil(x * 10 - 1e-9) / 10);
const pctDown = (x: number) => String(Math.floor(x * 10 + 1e-9) / 10);

/** "Based on 65% of the course so far" */
export function basedOnLine(course: Course, grades: readonly Grade[]): string {
  return copy.grades.basedOn(pct(decidedPct(course, grades)));
}

/** "Weights add to 95%. The other 5% is unassigned." Null when they add to 100. */
export function weightsLine(course: Course): string | null {
  const total = Math.round(totalWeight(course) * 10) / 10;
  if (total === 100) return null;
  return total < 100 ? copy.grades.weightsShort(pct(total), pct(100 - total)) : copy.grades.weightsOver(pct(total), pct(total - 100));
}

/**
 * "To finish with a B (83%), you need 78.4% on the remaining 35%." When the
 * target can't be reached, it says so, with the highest possible. Null for a
 * letter the course's scale doesn't have.
 */
export function whatItWouldTake(course: Course, grades: readonly Grade[], letter: string): string | null {
  const scale = scaleOf(course);
  const target = scale.find((s) => s.letter === letter);
  if (!target) return null;
  const decided = decidedPct(course, grades) / 100;
  const banked = (currentPct(course, grades) ?? 0) * decided;
  const remaining = 1 - decided;
  if (remaining <= 1e-9) {
    const now = currentPct(course, grades) ?? 0;
    return copy.grades.allGraded(pct(now), letterFor(now, scale));
  }
  const need = (target.min - banked) / remaining;
  const left = pct(remaining * 100);
  if (need <= 0) return copy.grades.safe(letter, left);
  if (need > 100) {
    const best = banked + 100 * remaining;
    return copy.grades.outOfReach(letter, pctDown(best), letterFor(best, scale));
  }
  return copy.grades.need(letter, pct(target.min), pctUp(need), left);
}

/**
 * The category a grade's title suggests, from each category's keywords:
 * whole words, any case, the longest matching keyword winning.
 */
export function guessCategory(title: string, categories: readonly GradeCategory[]): Id | null {
  const text = title.toLowerCase();
  let best: { id: Id; length: number } | null = null;
  for (const c of categories) {
    for (const raw of c.keywords) {
      const k = raw.trim().toLowerCase();
      if (!k || (best && k.length <= best.length)) continue;
      const escaped = k.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      if (new RegExp(`(^|[^a-z0-9])${escaped}($|[^a-z0-9])`).test(text)) best = { id: c.id, length: k.length };
    }
  }
  return best?.id ?? null;
}

/** "Fall 2026 grades" */
export function gradesHeader(term: Term): string {
  return copy.grades.header(term.name);
}

/** The right column's Grades widget: the lowest course, and the grades waiting for a score. Hidden until a course exists. */
export function gradesWidget(
  courses: readonly Course[],
  grades: readonly Grade[],
): { lowest: { courseId: Id; code: string; pct: number; letter: string } | null; pendingLine: string | null } | null {
  if (courses.length === 0) return null;
  let lowest: { courseId: Id; code: string; pct: number; letter: string } | null = null;
  for (const c of courses) {
    const p = currentPct(c, grades);
    if (p !== null && (lowest === null || p < lowest.pct)) {
      lowest = { courseId: c.id, code: c.code, pct: p, letter: letterFor(p, scaleOf(c)) };
    }
  }
  const pending = grades.filter((g) => g.pending).length;
  return { lowest, pendingLine: pending > 0 ? copy.grades.toEnter(pending) : null };
}
