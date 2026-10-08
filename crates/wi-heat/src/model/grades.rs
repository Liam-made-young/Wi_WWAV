//! Grades (`docs/SPEC.md` 3.1 and 3.8). The maths stays exactly as Heat has it:
//!
//! ```text
//!   category % = Σ score / Σ outOf   (items not dropped, outOf > 0, scored)
//!   current %  = Σ (weight × category %) / Σ weight   (graded categories only)
//!   decided %  = graded weight / total weight × 100
//!   letter     = ≥93 A, ≥90 A-, ≥87 B+, ≥83 B, ≥80 B-, ≥77 C+, ≥73 C, ≥70 C-, ≥67 D+, ≥60 D, else F
//! ```
//!
//! "What it would take" is plain arithmetic on the same numbers: the final
//! grade is current % × decided share + x × the remaining share.
//!
//! The formulas are exact, and floating point is not: 0.2 × 68 + 0.8 × 58 comes
//! out as 59.99999999999999, not 60. So every percentage that is compared with
//! a letter's threshold is first rounded to 1e-9, far finer than any score
//! can tell apart and far coarser than the arithmetic's error.
//!
//! A port of `grades.ts`. The pill colour and the page's header are display
//! and stay in TypeScript.

use super::records::{ser, Course, Grade, GradeCategory, Id, LetterStep};
use super::{copy, js};
use serde::{Deserialize, Serialize};

const SCALE: [(&str, f64); 11] = [
    ("A", 93.0),
    ("A-", 90.0),
    ("B+", 87.0),
    ("B", 83.0),
    ("B-", 80.0),
    ("C+", 77.0),
    ("C", 73.0),
    ("C-", 70.0),
    ("D+", 67.0),
    ("D", 60.0),
    ("F", 0.0),
];

/// The default letter scale, highest first.
pub fn default_scale() -> Vec<LetterStep> {
    SCALE
        .iter()
        .map(|(letter, min)| LetterStep {
            letter: (*letter).to_string(),
            min: *min,
        })
        .collect()
}

fn scale_of(course: &Course) -> Vec<LetterStep> {
    course.scale.clone().unwrap_or_else(default_scale)
}

fn counts(g: &Grade) -> bool {
    !g.dropped && g.out_of > 0.0 && g.score.is_some()
}

pub fn category_pct(category_id: &str, grades: &[Grade]) -> Option<f64> {
    let items: Vec<&Grade> = grades
        .iter()
        .filter(|g| g.category_id.as_deref() == Some(category_id) && counts(g))
        .collect();
    if items.is_empty() {
        return None;
    }
    let score = items
        .iter()
        .fold(0.0, |sum, g| sum + g.score.unwrap_or(0.0));
    let out_of = items.iter().fold(0.0, |sum, g| sum + g.out_of);
    Some((score / out_of) * 100.0)
}

/// A category's percentage with its lowest scores dropped, when the course
/// drops any (`dropLowest`, from a syllabus). The lowest are the lowest by
/// score over what it was out of, and one item always stays: a category with
/// two quizzes that drops two still counts its better one.
pub fn category_pct_of(category: &GradeCategory, grades: &[Grade]) -> Option<f64> {
    let drop = category
        .drop_lowest
        .map_or(0, |n| js::max2(0.0, n.floor()) as usize);
    if drop == 0 {
        return category_pct(&category.id, grades);
    }
    let mut items: Vec<&Grade> = grades
        .iter()
        .filter(|g| g.category_id.as_deref() == Some(category.id.as_str()) && counts(g))
        .collect();
    if items.is_empty() {
        return None;
    }
    let share = |g: &Grade| g.score.unwrap_or(0.0) / g.out_of;
    items.sort_by(|a, b| share(a).total_cmp(&share(b)));
    let kept = &items[drop.min(items.len() - 1)..];
    let score = kept.iter().fold(0.0, |sum, g| sum + g.score.unwrap_or(0.0));
    let out_of = kept.iter().fold(0.0, |sum, g| sum + g.out_of);
    Some((score / out_of) * 100.0)
}

fn graded<'a>(course: &'a Course, grades: &[Grade]) -> Vec<(&'a GradeCategory, f64)> {
    let own: Vec<Grade> = grades
        .iter()
        .filter(|g| g.course_id == course.id)
        .cloned()
        .collect();
    course
        .categories
        .iter()
        .filter_map(|category| category_pct_of(category, &own).map(|pct| (category, pct)))
        .collect()
}

fn total_weight(course: &Course) -> f64 {
    course.categories.iter().fold(0.0, |sum, c| sum + c.weight)
}

fn tidy(x: f64) -> f64 {
    js::round(x * 1e9) / 1e9
}

// The graded weight, and the points it holds: Σ weight × category %.
fn banked(course: &Course, grades: &[Grade]) -> (f64, f64) {
    let g = graded(course, grades);
    (
        g.iter().fold(0.0, |sum, (c, _)| sum + c.weight),
        g.iter().fold(0.0, |sum, (c, pct)| sum + c.weight * pct),
    )
}

pub fn current_pct(course: &Course, grades: &[Grade]) -> Option<f64> {
    let (weight, points) = banked(course, grades);
    if weight <= 0.0 {
        None
    } else {
        Some(tidy(points / weight))
    }
}

pub fn decided_pct(course: &Course, grades: &[Grade]) -> f64 {
    let total = total_weight(course);
    if total <= 0.0 {
        return 0.0;
    }
    tidy((banked(course, grades).0 / total) * 100.0)
}

/// The letter for a percentage; F below the lowest step of a scale that has no 0 floor.
pub fn letter_for(pct: f64, scale: &[LetterStep]) -> String {
    scale
        .iter()
        .find(|s| pct >= s.min)
        .map_or_else(|| "F".to_string(), |s| s.letter.clone())
}

/// [`letter_for`] on the default scale.
pub fn letter_for_default(pct: f64) -> String {
    letter_for(pct, &default_scale())
}

// "83", "78.4": a percentage to one decimal, without a trailing ".0".
fn pct(x: f64) -> String {
    js::num_to_string(js::round(x * 10.0) / 10.0)
}
// What you need rounds up and the best you can do rounds down, so neither flatters.
fn pct_up(x: f64) -> String {
    js::num_to_string((x * 10.0 - 1e-9).ceil() / 10.0)
}
fn pct_down(x: f64) -> String {
    js::num_to_string((x * 10.0 + 1e-9).floor() / 10.0)
}

/// "Based on 65% of the course so far"
pub fn based_on_line(course: &Course, grades: &[Grade]) -> String {
    copy::grades::based_on(&pct(decided_pct(course, grades)))
}

/// The weights' sum, to one decimal: 0 for a course no one has given weights.
pub fn weights_total(course: &Course) -> f64 {
    js::round(total_weight(course) * 10.0) / 10.0
}

/// "Weights add to 95%. The other 5% is unassigned." None when they add to 100.
pub fn weights_line(course: &Course) -> Option<String> {
    let total = weights_total(course);
    if total == 100.0 {
        return None;
    }
    Some(if total < 100.0 {
        copy::grades::weights_short(&pct(total), &pct(100.0 - total))
    } else {
        copy::grades::weights_over(&pct(total), &pct(total - 100.0))
    })
}

/// "To finish with a B (83%), you need 78.4% on the remaining 35%." When the
/// target can't be reached, it says so, with the highest possible. None for a
/// letter the course's scale doesn't have.
pub fn what_it_would_take(course: &Course, grades: &[Grade], letter: &str) -> Option<String> {
    let scale = scale_of(course);
    let target = scale.iter().find(|s| s.letter == letter)?;
    let total = total_weight(course);
    let (weight, points) = banked(course, grades);
    // Shares of the final grade: what is banked (current % × decided share) and what is left.
    let banked_share = if total > 0.0 { points / total } else { 0.0 };
    let remaining = if total > 0.0 {
        1.0 - weight / total
    } else {
        1.0
    };
    if remaining <= 1e-9 {
        let now = current_pct(course, grades).unwrap_or(0.0);
        return Some(copy::grades::all_graded(
            &pct(now),
            &letter_for(now, &scale),
        ));
    }
    // Both from the same two numbers, so the sentence can't contradict itself.
    let best = tidy(banked_share + 100.0 * remaining);
    let need = tidy((target.min - banked_share) / remaining);
    let left = pct(remaining * 100.0);
    if best < target.min {
        // The letter comes from the number printed, so the sentence reads true.
        let shown = pct_down(best);
        return Some(copy::grades::out_of_reach(
            letter,
            &shown,
            &letter_for(js::to_number(&shown), &scale),
        ));
    }
    if need <= 0.0 {
        return Some(copy::grades::safe(letter, &left));
    }
    Some(copy::grades::need(
        letter,
        &pct(target.min),
        &pct_up(js::min2(100.0, need)),
        &left,
    ))
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

// `new RegExp(`(^|[^a-z0-9])${escaped}($|[^a-z0-9])`).test(text)`: the keyword
// somewhere in the text with no letter or digit on either side.
fn has_whole_word(text: &str, keyword: &str) -> bool {
    text.char_indices().any(|(i, _)| {
        let rest = &text[i..];
        rest.starts_with(keyword)
            && !text[..i].chars().next_back().is_some_and(is_word_char)
            && !rest[keyword.len()..]
                .chars()
                .next()
                .is_some_and(is_word_char)
    })
}

/// The category a grade's title suggests, from each category's keywords:
/// whole words, any case, the longest matching keyword winning.
pub fn guess_category(title: &str, categories: &[GradeCategory]) -> Option<Id> {
    let text = title.to_lowercase();
    let mut best: Option<(&Id, usize)> = None;
    for c in categories {
        for raw in &c.keywords {
            let k = js::trim(raw).to_lowercase();
            if k.is_empty() || best.is_some_and(|(_, length)| js::utf16_len(&k) <= length) {
                continue;
            }
            if has_whole_word(&text, &k) {
                best = Some((&c.id, js::utf16_len(&k)));
            }
        }
    }
    best.map(|(id, _)| id.clone())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LowestCourse {
    pub course_id: Id,
    pub code: String,
    #[serde(serialize_with = "ser::num")]
    pub pct: f64,
    pub letter: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradesWidget {
    pub lowest: Option<LowestCourse>,
    pub pending_line: Option<String>,
}

/// The right column's Grades widget: the lowest course, and the grades
/// waiting for a score. None until a course exists.
pub fn grades_widget(courses: &[Course], grades: &[Grade]) -> Option<GradesWidget> {
    if courses.is_empty() {
        return None;
    }
    let mut lowest: Option<LowestCourse> = None;
    for c in courses {
        if let Some(p) = current_pct(c, grades) {
            if lowest.as_ref().map_or(true, |l| p < l.pct) {
                lowest = Some(LowestCourse {
                    course_id: c.id.clone(),
                    code: c.code.clone(),
                    pct: p,
                    letter: letter_for(p, &scale_of(c)),
                });
            }
        }
    }
    let pending = grades.iter().filter(|g| g.pending).count();
    Some(GradesWidget {
        lowest,
        pending_line: (pending > 0).then(|| copy::grades::to_enter(pending as f64)),
    })
}
