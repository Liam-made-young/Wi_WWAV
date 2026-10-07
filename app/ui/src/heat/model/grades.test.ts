import { describe, expect, it } from 'vitest';
import {
  DEFAULT_SCALE,
  basedOnLine,
  categoryPct,
  currentPct,
  decidedPct,
  gradesHeader,
  gradesWidget,
  guessCategory,
  letterFor,
  pillColour,
  weightsLine,
  whatItWouldTake,
} from './grades';
import type { Course, Grade, GradeCategory } from './records';

let serial = 0;
function grade(categoryId: string | null, score: number | null, outOf = 100, over: Partial<Grade> = {}): Grade {
  serial += 1;
  return {
    id: `g${serial}`,
    courseId: 'jpn201',
    categoryId,
    title: `Item ${serial}`,
    score,
    outOf,
    dropped: false,
    pending: false,
    source: 'you',
    ...over,
  };
}

function cat(id: string, weight: number, keywords: string[] = []): GradeCategory {
  return { id, name: id, weight, keywords };
}

function course(categories: GradeCategory[], over: Partial<Course> = {}): Course {
  return {
    id: 'jpn201',
    termId: 'fall26',
    code: 'JPN 201',
    name: 'Intermediate Japanese',
    categories,
    notes: '',
    ...over,
  };
}

describe('the grade maths (3.1), unchanged', () => {
  it('category % is Σ score / Σ outOf over items not dropped, with outOf > 0, scored', () => {
    const grades = [
      grade('hw', 8, 10),
      grade('hw', 18, 20),
      grade('hw', 0, 10, { dropped: true }),
      grade('hw', null, 10),
      grade('hw', 5, 0),
      grade('hw', null, 10, { pending: true }),
      grade('quiz', 1, 1),
    ];
    expect(categoryPct('hw', grades)).toBeCloseTo((26 / 30) * 100, 10);
    expect(categoryPct('lab', grades)).toBeNull();
  });

  it('current % weighs graded categories only, and decided % is their share of all weight', () => {
    const c = course([cat('hw', 25), cat('quiz', 20), cat('mid', 20), cat('final', 35)]);
    const grades = [grade('hw', 92), grade('quiz', 88), grade('mid', 74.8)];
    expect(currentPct(c, grades)).toBeCloseTo((25 * 92 + 20 * 88 + 20 * 74.8) / 65, 8); // rounded to 1e-9 before any letter is read
    expect(decidedPct(c, grades)).toBeCloseTo(65, 10);
    expect(currentPct(c, [])).toBeNull();
    expect(decidedPct(c, [])).toBe(0);
  });

  it('counts only the course’s own grades', () => {
    const c = course([cat('hw', 100)]);
    expect(currentPct(c, [grade('hw', 90), grade('hw', 10, 100, { courseId: 'mth142' })])).toBe(90);
  });

  it('reads "Based on 65% of the course so far"', () => {
    const c = course([cat('hw', 25), cat('quiz', 20), cat('mid', 20), cat('final', 35)]);
    expect(basedOnLine(c, [grade('hw', 92), grade('quiz', 88), grade('mid', 74.8)])).toBe(
      'Based on 65% of the course so far',
    );
  });
});

describe('letters', () => {
  it('uses the default scale at every edge', () => {
    const cases: [number, string][] = [
      [100, 'A'],
      [93, 'A'],
      [92.99, 'A-'],
      [90, 'A-'],
      [89.99, 'B+'],
      [87, 'B+'],
      [86.9, 'B'],
      [83, 'B'],
      [82.9, 'B-'],
      [80, 'B-'],
      [79.9, 'C+'],
      [77, 'C+'],
      [73, 'C'],
      [72.9, 'C-'],
      [70, 'C-'],
      [69.9, 'D+'],
      [67, 'D+'],
      [66.9, 'D'],
      [60, 'D'],
      [59.99, 'F'],
      [0, 'F'],
    ];
    for (const [pct, letter] of cases) expect(letterFor(pct), String(pct)).toBe(letter);
    expect(DEFAULT_SCALE.map((s) => s.letter)).toEqual(['A', 'A-', 'B+', 'B', 'B-', 'C+', 'C', 'C-', 'D+', 'D', 'F']);
  });

  it('uses a course’s own scale when it brings one', () => {
    const scale = [
      { letter: 'A', min: 90 },
      { letter: 'B', min: 80 },
      { letter: 'C', min: 70 },
      { letter: 'F', min: 0 },
    ];
    expect(letterFor(91, scale)).toBe('A');
    expect(letterFor(89, scale)).toBe('B');
  });

  it('colours the pill green for A, blue for B, amber for C and red for D or F', () => {
    expect(['A', 'A-', 'B+', 'B', 'B-', 'C+', 'C', 'C-', 'D+', 'D', 'F'].map(pillColour)).toEqual([
      'green',
      'green',
      'blue',
      'blue',
      'blue',
      'amber',
      'amber',
      'amber',
      'red',
      'red',
      'red',
    ]);
    expect(pillColour('P')).toBe('grey');
  });
});

describe('the course editor', () => {
  it('says when the weights don’t add up', () => {
    expect(weightsLine(course([cat('a', 60), cat('b', 35)]))).toBe('Weights add to 95%. The other 5% is unassigned.');
    expect(weightsLine(course([cat('a', 70), cat('b', 35)]))).toBe('Weights add to 105%. That is 5% more than 100.');
    expect(weightsLine(course([cat('a', 65), cat('b', 35)]))).toBeNull();
    expect(weightsLine(course([cat('a', 33.3), cat('b', 33.3), cat('c', 33.4)]))).toBeNull();
  });
});

describe('what it would take (3.8)', () => {
  const c = course([cat('hw', 25), cat('quiz', 20), cat('mid', 20), cat('final', 35)]);

  it('gives the score needed on what remains', () => {
    const grades = [grade('hw', 92), grade('quiz', 88), grade('mid', 74.8)];
    expect(whatItWouldTake(c, grades, 'B')).toBe('To finish with a B (83%), you need 78.4% on the remaining 35%.');
    // The best possible is 55.56 + 35 = 90.56.
    expect(whatItWouldTake(c, grades, 'A')).toBe('An A is out of reach; the highest possible is 90.5% (A-).');
  });

  it('says when the target is out of reach, with the highest possible', () => {
    const d = course([cat('a', 40), cat('b', 40), cat('final', 20)]);
    const grades = [grade('a', 75), grade('b', 78)];
    expect(whatItWouldTake(d, grades, 'B')).toBe('A B is out of reach; the highest possible is 81.2% (B-).');
  });

  it('rounds what you need up and the highest possible down, so neither flatters', () => {
    const d = course([cat('a', 50), cat('final', 50)]);
    // 83 = 0.5 × 80.1 + 0.5 × x, so x is 85.9 exactly; 80.13 needs 85.87, shown as 85.9.
    expect(whatItWouldTake(d, [grade('a', 80.13)], 'B')).toBe(
      'To finish with a B (83%), you need 85.9% on the remaining 50%.',
    );
    // The best is 0.5 × 61.97 + 50 = 80.985, shown as 80.9, still a B-.
    expect(whatItWouldTake(d, [grade('a', 61.97)], 'B')).toBe(
      'A B is out of reach; the highest possible is 80.9% (B-).',
    );
    expect(whatItWouldTake(d, [grade('a', 60)], 'B')).toBe('A B is out of reach; the highest possible is 80% (B-).');
  });

  it('says when the target is already safe', () => {
    const d = course([cat('a', 90), cat('final', 10)]);
    expect(whatItWouldTake(d, [grade('a', 95)], 'B')).toBe('You keep a B even with 0% on the remaining 10%.');
  });

  it('works before anything is graded, and after everything is', () => {
    expect(whatItWouldTake(c, [], 'C')).toBe('To finish with a C (73%), you need 73% on the remaining 100%.');
    const all = [grade('hw', 90), grade('quiz', 90), grade('mid', 80), grade('final', 85)];
    expect(whatItWouldTake(c, all, 'A')).toBe('Every category is graded. The course stands at 86.3% (B).');
  });

  it('uses the course’s scale for the target', () => {
    const d = course([cat('a', 50), cat('final', 50)], {
      scale: [
        { letter: 'A', min: 90 },
        { letter: 'F', min: 0 },
      ],
    });
    expect(whatItWouldTake(d, [grade('a', 90)], 'A')).toBe(
      'To finish with an A (90%), you need 90% on the remaining 50%.',
    );
  });

  it('returns nothing for a letter the scale doesn’t have', () => {
    expect(whatItWouldTake(c, [], 'Z')).toBeNull();
  });
});

describe('category keywords (3.8)', () => {
  const cats = [
    cat('tickets', 10, ['exit ticket']),
    cat('homework', 20, ['Edfinity', 'homework']),
    cat('kanji', 10, ['kanji']),
    cat('labs', 30, ['lab']),
    cat('lab5a', 5, ['Lab 5a']),
  ];

  it('guesses a grade’s category from its keywords, ignoring case', () => {
    expect(guessCategory('Exit Ticket 12', cats)).toBe('tickets');
    expect(guessCategory('EDFINITY 4.2', cats)).toBe('homework');
    expect(guessCategory('Kanji quiz 7', cats)).toBe('kanji');
    expect(guessCategory('Lab 3: Titration', cats)).toBe('labs');
  });

  it('prefers the longest keyword that matches', () => {
    expect(guessCategory('Lab 5a write-up', cats)).toBe('lab5a');
  });

  it('matches whole words only, and gives nothing when no keyword matches', () => {
    expect(guessCategory('Labor history essay', cats)).toBeNull();
    expect(guessCategory('Midterm', cats)).toBeNull();
  });
});

describe('the Grades tab and widget', () => {
  it('reads the term from its record: "Fall 2026 grades"', () => {
    expect(gradesHeader({ id: 'fall26', name: 'Fall 2026' })).toBe('Fall 2026 grades');
  });

  it('shows the lowest course and its letter, and the pending grades to enter', () => {
    const jpn = course([cat('hw', 100)]);
    const mth = course([cat('hw', 100)], { id: 'mth142', code: 'MTH 142' });
    const empty = course([cat('hw', 100)], { id: 'his101', code: 'HIS 101' });
    const grades = [
      grade('hw', 91),
      grade('hw', 78, 100, { courseId: 'mth142' }),
      grade('hw', null, 100, { courseId: 'mth142', pending: true }),
      grade('hw', null, 100, { pending: true }),
    ];
    expect(gradesWidget([jpn, mth, empty], grades)).toEqual({
      lowest: { courseId: 'mth142', code: 'MTH 142', pct: 78, letter: 'C+' },
      pendingLine: '2 new grades to enter',
    });
    expect(gradesWidget([], [])).toBeNull();
    expect(gradesWidget([jpn], [grade('hw', 91)])).toEqual({
      lowest: { courseId: 'jpn201', code: 'JPN 201', pct: 91, letter: 'A-' },
      pendingLine: null,
    });
  });
});
