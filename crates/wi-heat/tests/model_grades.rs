//! `grades.test.ts`, case for case. (The letter pill's colour and the Grades
//! page's header are display and stay in TypeScript, with their tests.)

use std::sync::atomic::{AtomicUsize, Ordering};
use wi_heat::model::grades::{
    based_on_line, category_pct, current_pct, decided_pct, default_scale, grades_widget,
    guess_category, letter_for, letter_for_default, weights_line, what_it_would_take, GradesWidget,
    LowestCourse,
};
use wi_heat::model::records::{Course, Grade, GradeCategory, GradeSource, LetterStep};

static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn grade_with(
    category_id: Option<&str>,
    score: Option<f64>,
    out_of: f64,
    over: impl FnOnce(&mut Grade),
) -> Grade {
    let n = SERIAL.fetch_add(1, Ordering::SeqCst) + 1;
    let mut g = Grade {
        id: format!("g{n}"),
        course_id: "jpn201".into(),
        category_id: category_id.map(str::to_string),
        title: format!("Item {n}"),
        score,
        out_of,
        dropped: false,
        pending: false,
        link: None,
        source: GradeSource::You,
        ..Default::default()
    };
    over(&mut g);
    g
}

/// A scored grade out of 100.
fn grade(category: &str, score: f64) -> Grade {
    grade_with(Some(category), Some(score), 100.0, |_| {})
}

fn grade_of(category: &str, score: f64, out_of: f64) -> Grade {
    grade_with(Some(category), Some(score), out_of, |_| {})
}

fn cat(id: &str, weight: f64) -> GradeCategory {
    cat_with(id, weight, &[])
}

fn cat_with(id: &str, weight: f64, keywords: &[&str]) -> GradeCategory {
    GradeCategory {
        id: id.into(),
        name: id.into(),
        weight,
        keywords: keywords.iter().map(|k| (*k).to_string()).collect(),
    }
}

fn course(categories: Vec<GradeCategory>) -> Course {
    course_with(categories, |_| {})
}

fn course_with(categories: Vec<GradeCategory>, over: impl FnOnce(&mut Course)) -> Course {
    let mut c = Course {
        id: "jpn201".into(),
        term_id: "fall26".into(),
        code: "JPN 201".into(),
        name: "Intermediate Japanese".into(),
        categories,
        scale: None,
        notes: String::new(),
        ..Default::default()
    };
    over(&mut c);
    c
}

fn scale(steps: &[(&str, f64)]) -> Vec<LetterStep> {
    steps
        .iter()
        .map(|(letter, min)| LetterStep {
            letter: (*letter).into(),
            min: *min,
        })
        .collect()
}

fn close(got: f64, want: f64, digits: i32) -> bool {
    (got - want).abs() < 10f64.powi(-digits) / 2.0
}

// --- the grade maths (3.1), unchanged -----------------------------------------------------------

#[test]
fn category_pct_is_sum_score_over_sum_out_of_over_items_not_dropped_with_out_of_above_0_scored() {
    let grades = vec![
        grade_of("hw", 8.0, 10.0),
        grade_of("hw", 18.0, 20.0),
        grade_with(Some("hw"), Some(0.0), 10.0, |g| g.dropped = true),
        grade_with(Some("hw"), None, 10.0, |_| {}),
        grade_of("hw", 5.0, 0.0),
        grade_with(Some("hw"), None, 10.0, |g| g.pending = true),
        grade_of("quiz", 1.0, 1.0),
    ];
    assert!(close(
        category_pct("hw", &grades).unwrap(),
        (26.0 / 30.0) * 100.0,
        10
    ));
    assert_eq!(category_pct("lab", &grades), None);
}

#[test]
fn current_pct_weighs_graded_categories_only_and_decided_pct_is_their_share_of_all_weight() {
    let c = course(vec![
        cat("hw", 25.0),
        cat("quiz", 20.0),
        cat("mid", 20.0),
        cat("final", 35.0),
    ]);
    let grades = vec![grade("hw", 92.0), grade("quiz", 88.0), grade("mid", 74.8)];
    // Rounded to 1e-9 before any letter is read.
    assert!(close(
        current_pct(&c, &grades).unwrap(),
        (25.0 * 92.0 + 20.0 * 88.0 + 20.0 * 74.8) / 65.0,
        8
    ));
    assert!(close(decided_pct(&c, &grades), 65.0, 10));
    assert_eq!(current_pct(&c, &[]), None);
    assert_eq!(decided_pct(&c, &[]), 0.0);
}

#[test]
fn counts_only_the_courses_own_grades() {
    let c = course(vec![cat("hw", 100.0)]);
    let other = grade_with(Some("hw"), Some(10.0), 100.0, |g| {
        g.course_id = "mth142".into()
    });
    assert_eq!(current_pct(&c, &[grade("hw", 90.0), other]), Some(90.0));
}

#[test]
fn reads_based_on_65_of_the_course_so_far() {
    let c = course(vec![
        cat("hw", 25.0),
        cat("quiz", 20.0),
        cat("mid", 20.0),
        cat("final", 35.0),
    ]);
    assert_eq!(
        based_on_line(
            &c,
            &[grade("hw", 92.0), grade("quiz", 88.0), grade("mid", 74.8)]
        ),
        "Based on 65% of the course so far"
    );
}

// --- letters -----------------------------------------------------------------------------------------

#[test]
fn uses_the_default_scale_at_every_edge() {
    let cases = [
        (100.0, "A"),
        (93.0, "A"),
        (92.99, "A-"),
        (90.0, "A-"),
        (89.99, "B+"),
        (87.0, "B+"),
        (86.9, "B"),
        (83.0, "B"),
        (82.9, "B-"),
        (80.0, "B-"),
        (79.9, "C+"),
        (77.0, "C+"),
        (73.0, "C"),
        (72.9, "C-"),
        (70.0, "C-"),
        (69.9, "D+"),
        (67.0, "D+"),
        (66.9, "D"),
        (60.0, "D"),
        (59.99, "F"),
        (0.0, "F"),
    ];
    for (pct, letter) in cases {
        assert_eq!(letter_for_default(pct), letter, "{pct}");
    }
    let letters: Vec<String> = default_scale().into_iter().map(|s| s.letter).collect();
    assert_eq!(
        letters,
        ["A", "A-", "B+", "B", "B-", "C+", "C", "C-", "D+", "D", "F"]
    );
}

#[test]
fn uses_a_courses_own_scale_when_it_brings_one() {
    let own = scale(&[("A", 90.0), ("B", 80.0), ("C", 70.0), ("F", 0.0)]);
    assert_eq!(letter_for(91.0, &own), "A");
    assert_eq!(letter_for(89.0, &own), "B");
}

// --- the course editor -----------------------------------------------------------------------------------

#[test]
fn says_when_the_weights_dont_add_up() {
    assert_eq!(
        weights_line(&course(vec![cat("a", 60.0), cat("b", 35.0)])).as_deref(),
        Some("Weights add to 95%. The other 5% is unassigned.")
    );
    assert_eq!(
        weights_line(&course(vec![cat("a", 70.0), cat("b", 35.0)])).as_deref(),
        Some("Weights add to 105%. That is 5% more than 100.")
    );
    assert_eq!(
        weights_line(&course(vec![cat("a", 65.0), cat("b", 35.0)])),
        None
    );
    assert_eq!(
        weights_line(&course(vec![
            cat("a", 33.3),
            cat("b", 33.3),
            cat("c", 33.4)
        ])),
        None
    );
}

// --- what it would take (3.8) --------------------------------------------------------------------------------

fn four() -> Course {
    course(vec![
        cat("hw", 25.0),
        cat("quiz", 20.0),
        cat("mid", 20.0),
        cat("final", 35.0),
    ])
}

#[test]
fn gives_the_score_needed_on_what_remains() {
    let grades = vec![grade("hw", 92.0), grade("quiz", 88.0), grade("mid", 74.8)];
    assert_eq!(
        what_it_would_take(&four(), &grades, "B").as_deref(),
        Some("To finish with a B (83%), you need 78.4% on the remaining 35%.")
    );
    // The best possible is 55.56 + 35 = 90.56.
    assert_eq!(
        what_it_would_take(&four(), &grades, "A").as_deref(),
        Some("An A is out of reach; the highest possible is 90.5% (A-).")
    );
}

#[test]
fn says_when_the_target_is_out_of_reach_with_the_highest_possible() {
    let d = course(vec![cat("a", 40.0), cat("b", 40.0), cat("final", 20.0)]);
    let grades = vec![grade("a", 75.0), grade("b", 78.0)];
    assert_eq!(
        what_it_would_take(&d, &grades, "B").as_deref(),
        Some("A B is out of reach; the highest possible is 81.2% (B-).")
    );
}

#[test]
fn rounds_what_you_need_up_and_the_highest_possible_down_so_neither_flatters() {
    let d = course(vec![cat("a", 50.0), cat("final", 50.0)]);
    // 83 = 0.5 × 80.1 + 0.5 × x, so x is 85.9 exactly; 80.13 needs 85.87, shown as 85.9.
    assert_eq!(
        what_it_would_take(&d, &[grade("a", 80.13)], "B").as_deref(),
        Some("To finish with a B (83%), you need 85.9% on the remaining 50%.")
    );
    // The best is 0.5 × 61.97 + 50 = 80.985, shown as 80.9, still a B-.
    assert_eq!(
        what_it_would_take(&d, &[grade("a", 61.97)], "B").as_deref(),
        Some("A B is out of reach; the highest possible is 80.9% (B-).")
    );
    assert_eq!(
        what_it_would_take(&d, &[grade("a", 60.0)], "B").as_deref(),
        Some("A B is out of reach; the highest possible is 80% (B-).")
    );
}

#[test]
fn says_when_the_target_is_already_safe() {
    let d = course(vec![cat("a", 90.0), cat("final", 10.0)]);
    assert_eq!(
        what_it_would_take(&d, &[grade("a", 95.0)], "B").as_deref(),
        Some("You keep a B even with 0% on the remaining 10%.")
    );
}

#[test]
fn works_before_anything_is_graded_and_after_everything_is() {
    assert_eq!(
        what_it_would_take(&four(), &[], "C").as_deref(),
        Some("To finish with a C (73%), you need 73% on the remaining 100%.")
    );
    let all = vec![
        grade("hw", 90.0),
        grade("quiz", 90.0),
        grade("mid", 80.0),
        grade("final", 85.0),
    ];
    assert_eq!(
        what_it_would_take(&four(), &all, "A").as_deref(),
        Some("Every category is graded. The course stands at 86.3% (B).")
    );
}

#[test]
fn uses_the_courses_scale_for_the_target() {
    let d = course_with(vec![cat("a", 50.0), cat("final", 50.0)], |c| {
        c.scale = Some(scale(&[("A", 90.0), ("F", 0.0)]));
    });
    assert_eq!(
        what_it_would_take(&d, &[grade("a", 90.0)], "A").as_deref(),
        Some("To finish with an A (90%), you need 90% on the remaining 50%.")
    );
}

#[test]
fn returns_nothing_for_a_letter_the_scale_doesnt_have() {
    assert_eq!(what_it_would_take(&four(), &[], "Z"), None);
}

// --- category keywords (3.8) ---------------------------------------------------------------------------------------

fn keyword_cats() -> Vec<GradeCategory> {
    vec![
        cat_with("tickets", 10.0, &["exit ticket"]),
        cat_with("homework", 20.0, &["Edfinity", "homework"]),
        cat_with("kanji", 10.0, &["kanji"]),
        cat_with("labs", 30.0, &["lab"]),
        cat_with("lab5a", 5.0, &["Lab 5a"]),
    ]
}

#[test]
fn guesses_a_grades_category_from_its_keywords_ignoring_case() {
    let cats = keyword_cats();
    assert_eq!(
        guess_category("Exit Ticket 12", &cats).as_deref(),
        Some("tickets")
    );
    assert_eq!(
        guess_category("EDFINITY 4.2", &cats).as_deref(),
        Some("homework")
    );
    assert_eq!(
        guess_category("Kanji quiz 7", &cats).as_deref(),
        Some("kanji")
    );
    assert_eq!(
        guess_category("Lab 3: Titration", &cats).as_deref(),
        Some("labs")
    );
}

#[test]
fn prefers_the_longest_keyword_that_matches() {
    assert_eq!(
        guess_category("Lab 5a write-up", &keyword_cats()).as_deref(),
        Some("lab5a")
    );
}

#[test]
fn matches_whole_words_only_and_gives_nothing_when_no_keyword_matches() {
    let cats = keyword_cats();
    assert_eq!(guess_category("Labor history essay", &cats), None);
    assert_eq!(guess_category("Midterm", &cats), None);
}

// --- the Grades widget -------------------------------------------------------------------------------------------------

#[test]
fn shows_the_lowest_course_and_its_letter_and_the_pending_grades_to_enter() {
    let jpn = course(vec![cat("hw", 100.0)]);
    let mth = course_with(vec![cat("hw", 100.0)], |c| {
        c.id = "mth142".into();
        c.code = "MTH 142".into();
    });
    let empty = course_with(vec![cat("hw", 100.0)], |c| {
        c.id = "his101".into();
        c.code = "HIS 101".into();
    });
    let grades = vec![
        grade("hw", 91.0),
        grade_with(Some("hw"), Some(78.0), 100.0, |g| {
            g.course_id = "mth142".into()
        }),
        grade_with(Some("hw"), None, 100.0, |g| {
            g.course_id = "mth142".into();
            g.pending = true;
        }),
        grade_with(Some("hw"), None, 100.0, |g| g.pending = true),
    ];
    assert_eq!(
        grades_widget(&[jpn.clone(), mth, empty], &grades),
        Some(GradesWidget {
            lowest: Some(LowestCourse {
                course_id: "mth142".into(),
                code: "MTH 142".into(),
                pct: 78.0,
                letter: "C+".into()
            }),
            pending_line: Some("2 new grades to enter".into()),
        })
    );
    assert_eq!(grades_widget(&[], &[]), None);
    assert_eq!(
        grades_widget(&[jpn], &[grade("hw", 91.0)]),
        Some(GradesWidget {
            lowest: Some(LowestCourse {
                course_id: "jpn201".into(),
                code: "JPN 201".into(),
                pct: 91.0,
                letter: "A-".into()
            }),
            pending_line: None,
        })
    );
}
