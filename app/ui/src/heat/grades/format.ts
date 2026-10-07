// Words for a grade's numbers. The core works the numbers out and the snapshot
// carries them; these only write them down: "78.4", "83", "Fall 2026".

import { type DayKey, keyParts } from '../../shared/time/zone';
import { copy } from '../fmt';

/** A percentage to one decimal, without a trailing ".0". */
export const pctText = (x: number) => String(Math.round(x * 10) / 10);

/** Letter pills are green for A, blue for B, amber for C and red for D or F (3.1). The letter is on the pill too, so colour is never alone. */
export function pillTone(letter: string | null): 'green' | 'blue' | 'amber' | 'red' | 'none' {
  switch (letter?.[0]) {
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
      return 'none';
  }
}

/** The name a new term is offered under: "Fall 2026". */
export function termNameFor(day: DayKey): string {
  const { year, month } = keyParts(day);
  return `${month <= 5 ? 'Spring' : month <= 7 ? 'Summer' : 'Fall'} ${year}`;
}

/**
 * The sentence for weights as the course editor has them typed, before they
 * are saved. A saved course's own comes from the snapshot, with the core's
 * words; this one says the same thing about a draft.
 */
export function draftWeights(total: number): string | null {
  const t = Math.round(total * 10) / 10;
  if (t === 100) return null;
  return t < 100
    ? copy.grades.weightsShort(pctText(t), pctText(100 - t))
    : copy.grades.weightsOver(pctText(t), pctText(t - 100));
}
