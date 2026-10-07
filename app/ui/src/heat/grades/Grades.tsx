// Grades (docs/SPEC.md 3.8): global, so the space filter doesn't apply. The
// header reads the current term from a Term record ("Fall 2026 grades"), and
// each course is a card: its percentage and a letter pill, how much of the
// course that rests on, yellow "Enter score" banners for grades Claude or you
// recorded without a score, "What it would take", and the items by category.
// "+" adds a grade; the tab's one secondary act is Add course.
//
// Every number comes from the snapshot (the core works them out); this file
// only lays them out.

import { useState } from 'react';
import type { Course, Id } from '../client';
import { copy, plural } from '../fmt';
import { useSheets, useTabActs, useTabKeys } from '../frame';
import { useHeat } from '../store';
import { CourseCard } from './CourseCard';
import { CourseSheet } from './CourseSheet';
import { GradeSheet } from './GradeSheet';
import './grades.css';

export function Grades() {
  const { snap } = useHeat();
  const { openSheet, closeSheet } = useSheets();
  const courses = snap?.records.course ?? [];
  const terms = snap?.records.term ?? [];
  const pending = snap?.records.grade.filter((g) => g.pending).length ?? 0;
  // The grade whose "Enter score" form is open: one at a time, and Esc closes it.
  const [scoring, setScoring] = useState<Id | null>(null);

  const addCourse = () => openSheet(<CourseSheet onClose={closeSheet} />);
  const addGrade = () => openSheet(<GradeSheet onClose={closeSheet} />);

  useTabActs({
    plus: courses.length === 0 ? { run: addCourse, disabled: copy.gradesUi.addCourseFirst } : { run: addGrade },
    secondary: { run: addCourse },
    count: snap ? (pending > 0 ? copy.grades.toEnter(pending) : plural(courses.length, 'course')) : null,
  });
  useTabKeys({
    escape() {
      if (scoring === null) return false;
      setScoring(null);
      return true;
    },
  });

  if (!snap) return <div className="heat-grades" />;

  // The newest term heads the page; courses of earlier terms sit under their own headings.
  const current = terms.at(-1);
  const termOf = (c: Course) => terms.find((t) => t.id === c.termId);
  const top = courses.filter((c) => !termOf(c) || c.termId === current?.id);
  const earlier = terms.filter((t) => t.id !== current?.id && courses.some((c) => c.termId === t.id));

  const card = (c: Course) => <CourseCard key={c.id} course={c} scoring={scoring} onScoring={setScoring} />;

  return (
    <div className="heat-grades">
      <header className="heat-grades-head">
        <div>
          <h1 className="heat-heading">{current ? copy.grades.header(current.name) : 'Grades'}</h1>
          <p className="heat-subtitle" data-text="secondary">
            {pending > 0 ? copy.grades.toEnter(pending) : plural(courses.length, 'course')}
          </p>
        </div>
        <button type="button" className="gel" onClick={addCourse} title={`${copy.tabs.secondary.Grades} (⇧Return)`}>
          {copy.tabs.secondary.Grades}
        </button>
      </header>
      <div className="heat-grades-body">
        {courses.length === 0 && <p className="heat-empty">{copy.gradesUi.noCourses}</p>}
        {top.map(card)}
        {earlier.map((t) => (
          <section key={t.id} aria-label={copy.grades.header(t.name)} className="heat-grades-term">
            <h2 className="heat-grades-termname" data-text="secondary">
              {copy.grades.header(t.name)}
            </h2>
            {courses.filter((c) => c.termId === t.id).map(card)}
          </section>
        ))}
      </div>
    </div>
  );
}
