//! Task types: what a kind of work usually takes, wherever the task came
//! from. A type is a name, the words in a title that pick it, and the minutes
//! and difficulty a task of that type starts with.
//!
//! Types live at three levels and the most specific wins: a task's home (its
//! course or its project), then its space, then the defaults below. A task
//! needs no home: one with neither still gets a type from its space or from
//! the defaults.
//!
//! Nothing here is the TypeScript's: the artifact had one difficulty and one
//! hour for every task. Plain functions, no clock.

use super::records::{ser, Task};
use serde::{Deserialize, Serialize};

/// A type as a home or a space keeps it. A number a home doesn't give (a
/// syllabus names "Labs" and says nothing of how long one takes) is taken
/// from the level below.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDef {
    pub name: String,
    /// Words or phrases that pick this type when a title holds them whole,
    /// in any case: "quiz", "problem set".
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub est_min: Option<f64>,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub difficulty: Option<f64>,
    /// The grade category this type's items count toward, by name. Courses only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// The type every level falls back to. It has no patterns: it is what a title
/// that picks nothing gets.
pub const OTHER: &str = "Other";

/// The defaults to start from: (name, minutes, difficulty, patterns). The
/// order breaks a tie between two patterns of the same length.
const DEFAULTS: [(&str, f64, f64, &[&str]); 11] = [
    ("Quiz", 20.0, 2.0, &["quiz", "quizzes"]),
    ("Worksheet", 45.0, 2.0, &["worksheet", "worksheets"]),
    ("Listening", 30.0, 2.0, &["listening"]),
    ("Lab", 120.0, 3.0, &["lab", "labs", "laboratory"]),
    (
        "Homework",
        90.0,
        3.0,
        &[
            "homework",
            "hw",
            "assignment",
            "problem set",
            "pset",
            "exercises",
        ],
    ),
    (
        "Project",
        240.0,
        4.0,
        &["project", "essay", "paper", "presentation", "proposal"],
    ),
    (
        "Email",
        10.0,
        1.0,
        &["email", "e-mail", "reply", "respond", "write back"],
    ),
    (
        "Errand",
        30.0,
        1.0,
        &["errand", "pick up", "drop off", "buy"],
    ),
    (
        "Admin",
        20.0,
        1.0,
        &[
            "form",
            "forms",
            "paperwork",
            "register",
            "registration",
            "renew",
            "sign up",
        ],
    ),
    (
        "Creative session",
        120.0,
        3.0,
        &[
            "session",
            "mix",
            "mixing",
            "recording",
            "songwriting",
            "sketch",
            "studio",
        ],
    ),
    (OTHER, 45.0, 2.0, &[]),
];

/// The global defaults, as types.
pub fn defaults() -> Vec<TypeDef> {
    DEFAULTS
        .iter()
        .map(|(name, minutes, difficulty, patterns)| TypeDef {
            name: (*name).to_string(),
            patterns: patterns.iter().map(|p| (*p).to_string()).collect(),
            est_min: Some(*minutes),
            difficulty: Some(*difficulty),
            category: None,
        })
        .collect()
}

/// A title's words, lower case, with a number split from the letters it
/// touches: "lab3" reads "lab", "3", and "HW2" reads "hw", "2", so a type's
/// word still matches the way a teacher numbers things.
pub fn words(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut digits = false;
    for c in s.chars() {
        if !c.is_alphanumeric() {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            continue;
        }
        let is_digit = c.is_ascii_digit();
        if !word.is_empty() && is_digit != digits {
            out.push(std::mem::take(&mut word));
        }
        digits = is_digit;
        word.extend(c.to_lowercase());
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// How well `def` fits a title already split into words: the longest of its
/// patterns found whole and in order, as (words, letters). None when no
/// pattern is there.
fn fit(def: &TypeDef, title: &[String]) -> Option<(usize, usize)> {
    def.patterns
        .iter()
        .filter_map(|p| {
            let p = words(p);
            (!p.is_empty() && title.windows(p.len()).any(|w| w == p.as_slice()))
                .then(|| (p.len(), p.iter().map(String::len).sum()))
        })
        .max()
}

/// The type of `defs` a title picks: the one whose longest pattern is in the
/// title, the longer pattern winning and the earlier type breaking a tie.
pub fn match_title<'a>(title: &str, defs: &'a [TypeDef]) -> Option<&'a TypeDef> {
    let title = words(title);
    let mut best: Option<(&TypeDef, (usize, usize))> = None;
    for def in defs {
        if let Some(score) = fit(def, &title) {
            if best.map_or(true, |(_, b)| score > b) {
                best = Some((def, score));
            }
        }
    }
    best.map(|(def, _)| def)
}

/// A type of `defs` by its name, in any case.
pub fn by_name<'a>(name: &str, defs: &'a [TypeDef]) -> Option<&'a TypeDef> {
    let want = name.trim().to_lowercase();
    (!want.is_empty())
        .then(|| defs.iter().find(|d| d.name.trim().to_lowercase() == want))
        .flatten()
}

/// Where a task's type was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Course,
    Project,
    Space,
    Global,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Course => "course",
            Level::Project => "project",
            Level::Space => "space",
            Level::Global => "global",
        }
    }
}

/// The levels a task's type is looked for in, most specific first. `home` is
/// the task's course's or project's types, with which of the two it is.
#[derive(Clone, Copy, Debug, Default)]
pub struct Levels<'a> {
    pub home: Option<(Level, &'a [TypeDef])>,
    pub space: &'a [TypeDef],
    pub global: &'a [TypeDef],
}

impl<'a> Levels<'a> {
    fn each(&self) -> impl Iterator<Item = (Level, &'a [TypeDef])> {
        self.home
            .into_iter()
            .chain([(Level::Space, self.space), (Level::Global, self.global)])
    }

    /// The type a title picks, and where: home, then space, then the defaults.
    pub fn match_title(&self, title: &str) -> Option<(Level, &'a TypeDef)> {
        self.each()
            .find_map(|(level, defs)| match_title(title, defs).map(|d| (level, d)))
    }

    /// The type of that name, and where it is defined first.
    pub fn by_name(&self, name: &str) -> Option<(Level, &'a TypeDef)> {
        self.each()
            .find_map(|(level, defs)| by_name(name, defs).map(|d| (level, d)))
    }
}

/// What a task starts with, and where the numbers came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Fill {
    pub est_min: f64,
    pub difficulty: f64,
    /// The level the type was found at. None: nothing matched, and the
    /// numbers are the catch-all's, which is what Claude is asked to better.
    pub from: Option<Level>,
    /// The grade category the type counts toward, if its home says.
    pub category: Option<String>,
}

/// The minutes and difficulty for a task of type `kind` titled `title`. The
/// type is found by its name, most specific level first; a name no level
/// defines ("Reading", a space's own word) is found by the title instead. A
/// number the type leaves out comes from the same name further down, and at
/// last from the catch-all.
pub fn fill(kind: &str, title: &str, levels: &Levels<'_>) -> Fill {
    let other = by_name(OTHER, levels.global);
    let catch_all = (
        other.and_then(|d| d.est_min).unwrap_or(45.0),
        other.and_then(|d| d.difficulty).unwrap_or(2.0),
    );
    let is_other = kind.trim().eq_ignore_ascii_case(OTHER) || kind.trim().is_empty();
    let found = if is_other {
        None
    } else {
        levels.by_name(kind).or_else(|| levels.match_title(title))
    };
    let Some((level, def)) = found else {
        return Fill {
            est_min: catch_all.0,
            difficulty: catch_all.1,
            from: None,
            category: None,
        };
    };
    // A number the home leaves out is the same-named type's further down.
    let below = |pick: &dyn Fn(&TypeDef) -> Option<f64>| -> Option<f64> {
        [levels.space, levels.global]
            .into_iter()
            .find_map(|defs| by_name(&def.name, defs).and_then(pick))
            .or_else(|| {
                [levels.space, levels.global]
                    .into_iter()
                    .find_map(|defs| match_title(&def.name, defs).and_then(pick))
            })
    };
    let est_min = def.est_min.or_else(|| below(&|d| d.est_min));
    let difficulty = def.difficulty.or_else(|| below(&|d| d.difficulty));
    Fill {
        // A type that names no minutes anywhere is as good as no match.
        from: est_min.map(|_| level),
        est_min: est_min.unwrap_or(catch_all.0),
        difficulty: difficulty.unwrap_or(catch_all.1),
        category: def.category.clone(),
    }
}

/// Minutes as an estimate holds them: to the nearest 5, kept within 5–600.
pub fn tidy_minutes(minutes: f64) -> f64 {
    ((minutes / 5.0).round() * 5.0).clamp(5.0, 600.0)
}

// ----- calibration ---------------------------------------------------------

/// How many of a type's latest finished tasks the ratio reads.
pub const ROLLING: usize = 8;
/// A ratio needs this many finished tasks before it moves an estimate.
pub const MIN_SAMPLES: usize = 2;
/// One task's ratio is kept within this, so a timer left running overnight
/// can't triple every lab; and what an estimate is multiplied by within this.
const ONE_RATIO: (f64, f64) = (0.25, 4.0);
const APPLIED: (f64, f64) = (0.5, 3.0);

/// One finished task with measured time: what its type said it would take,
/// and what it took.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// The task's home: `c:<courseId>` or `p:<projectId>`.
    pub home: Option<String>,
    pub space_id: String,
    /// The type's name, lower case.
    pub kind: String,
    pub base_min: f64,
    pub actual_min: f64,
    pub done_at: f64,
}

/// A task's home as [`Sample::home`] keys it.
pub fn home_key(task: &Task) -> Option<String> {
    let id = |v: &Option<String>| v.as_deref().filter(|s| !s.is_empty()).map(str::to_string);
    id(&task.course_id)
        .map(|c| format!("c:{c}"))
        .or_else(|| id(&task.project_id).map(|p| format!("p:{p}")))
}

/// The rolling ratio of time taken to the type's estimate: the mean over the
/// latest [`ROLLING`] samples that `keep` lets through. None with fewer than
/// [`MIN_SAMPLES`].
fn rolling(samples: &[Sample], keep: &dyn Fn(&Sample) -> bool) -> Option<f64> {
    let mut mine: Vec<&Sample> = samples
        .iter()
        .filter(|s| s.base_min > 0.0 && s.actual_min > 0.0 && keep(s))
        .collect();
    if mine.len() < MIN_SAMPLES {
        return None;
    }
    mine.sort_by(|a, b| b.done_at.total_cmp(&a.done_at));
    mine.truncate(ROLLING);
    let sum: f64 = mine
        .iter()
        .map(|s| (s.actual_min / s.base_min).clamp(ONE_RATIO.0, ONE_RATIO.1))
        .sum();
    Some(sum / mine.len() as f64)
}

/// What a new task's estimate is multiplied by: the ratio for its home and
/// type, else for its space and type, else for the type anywhere, else 1.
/// It is measured against what the type says, never against an estimate this
/// ratio already moved, so it settles instead of chasing itself.
pub fn ratio(samples: &[Sample], home: Option<&str>, space_id: &str, kind: &str) -> f64 {
    let kind = kind.trim().to_lowercase();
    let of_kind = |s: &Sample| s.kind == kind;
    home.and_then(|h| rolling(samples, &|s| of_kind(s) && s.home.as_deref() == Some(h)))
        .or_else(|| rolling(samples, &|s| of_kind(s) && s.space_id == space_id))
        .or_else(|| rolling(samples, &of_kind))
        .map_or(1.0, |r| r.clamp(APPLIED.0, APPLIED.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(name: &str, patterns: &[&str], est: Option<f64>, diff: Option<f64>) -> TypeDef {
        TypeDef {
            name: name.into(),
            patterns: patterns.iter().map(|p| p.to_string()).collect(),
            est_min: est,
            difficulty: diff,
            category: None,
        }
    }

    #[test]
    fn a_title_picks_the_default_type_a_teacher_would() {
        let g = defaults();
        let name = |t: &str| match_title(t, &g).map(|d| d.name.as_str());
        assert_eq!(name("Online Vocabulary Quiz (第2課)"), Some("Quiz"));
        assert_eq!(name("Worksheet 3"), Some("Worksheet"));
        assert_eq!(name("Listening Comprehension Task_Ch.2"), Some("Listening"));
        assert_eq!(name("lab3"), Some("Lab"), "a number run on to the word");
        assert_eq!(name("Lab 5a report"), Some("Lab"));
        assert_eq!(name("Homework 3"), Some("Homework"));
        assert_eq!(name("HW2"), Some("Homework"));
        assert_eq!(name("Problem set 6"), Some("Homework"));
        assert_eq!(name("Video Project 1 (draft)"), Some("Project"));
        assert_eq!(name("Reply to Dana about the venue"), Some("Email"));
        assert_eq!(name("Pick up the PCB order"), Some("Errand"));
        assert_eq!(name("Renew passport"), Some("Admin"));
        assert_eq!(name("Mix the second verse"), Some("Creative session"));
        assert_eq!(
            name("Syllabus check"),
            None,
            "\"lab\" inside a word doesn't count"
        );
        assert_eq!(name("Asynchronous Task (1%)  = October 12th class"), None);
    }

    #[test]
    fn the_longer_pattern_wins_and_the_earlier_type_breaks_a_tie() {
        let defs = vec![
            def("Quiz", &["quiz"], Some(20.0), Some(2.0)),
            def("Lab quiz", &["lab quiz"], Some(15.0), Some(1.0)),
            def("Lab", &["lab"], Some(120.0), Some(3.0)),
        ];
        assert_eq!(match_title("Lab quiz 2", &defs).unwrap().name, "Lab quiz");
        assert_eq!(
            match_title("Quiz on lab safety", &defs).unwrap().name,
            "Quiz"
        );
    }

    #[test]
    fn the_most_specific_level_wins() {
        let global = defaults();
        let course = vec![def("Lab", &["lab", "recitation"], Some(180.0), Some(4.0))];
        let space = vec![def("Quiz", &["quiz"], Some(30.0), None)];
        let levels = Levels {
            home: Some((Level::Course, &course)),
            space: &space,
            global: &global,
        };
        let lab = fill("Lab", "lab3", &levels);
        assert_eq!(
            (lab.est_min, lab.difficulty, lab.from),
            (180.0, 4.0, Some(Level::Course))
        );
        // The space's quiz gives the minutes; its difficulty comes from the default of that name.
        let quiz = fill("Quiz", "Quiz 2", &levels);
        assert_eq!(
            (quiz.est_min, quiz.difficulty, quiz.from),
            (30.0, 2.0, Some(Level::Space))
        );
        let hw = fill("Homework", "Homework 3", &levels);
        assert_eq!(
            (hw.est_min, hw.difficulty, hw.from),
            (90.0, 3.0, Some(Level::Global))
        );
        // No home at all: the same task still gets the default.
        let bare = Levels {
            home: None,
            space: &[],
            global: &global,
        };
        assert_eq!(
            fill("Errand", "Update device for URI Wi-Fi", &bare).est_min,
            30.0
        );
    }

    #[test]
    fn a_type_no_level_defines_is_read_from_the_title_then_left_to_claude() {
        let global = defaults();
        let levels = Levels {
            home: None,
            space: &[],
            global: &global,
        };
        // "Music" is a word of the WWAV space; the title says what kind of work it is.
        let mix = fill("Music", "Mix the second verse", &levels);
        assert_eq!((mix.est_min, mix.from), (120.0, Some(Level::Global)));
        let unknown = fill("Reading", "Reading response 3", &levels);
        assert_eq!(
            (unknown.est_min, unknown.difficulty, unknown.from),
            (45.0, 2.0, None)
        );
        assert_eq!(
            fill("Other", "Quiz 4", &levels).from,
            None,
            "Other is never a match"
        );
    }

    #[test]
    fn a_home_type_without_numbers_takes_them_from_below() {
        let global = defaults();
        let course = vec![
            def("Labs", &["lab"], None, None),
            def("Recitation", &["recitation"], None, None),
        ];
        let levels = Levels {
            home: Some((Level::Course, &course)),
            space: &[],
            global: &global,
        };
        let lab = fill("Labs", "lab3", &levels);
        assert_eq!(
            (lab.est_min, lab.difficulty, lab.from),
            (120.0, 3.0, Some(Level::Course))
        );
        let rec = fill("Recitation", "Recitation 4", &levels);
        assert_eq!(
            (rec.est_min, rec.from),
            (45.0, None),
            "nothing anywhere says how long"
        );
    }

    #[test]
    fn the_ratio_falls_back_from_home_to_space_to_type() {
        let s =
            |home: Option<&str>, space: &str, kind: &str, base: f64, actual: f64, at: f64| Sample {
                home: home.map(String::from),
                space_id: space.into(),
                kind: kind.into(),
                base_min: base,
                actual_min: actual,
                done_at: at,
            };
        let samples = vec![
            s(Some("c:jpn"), "classes", "quiz", 20.0, 40.0, 1.0),
            s(Some("c:jpn"), "classes", "quiz", 20.0, 40.0, 2.0),
            s(Some("c:ele"), "classes", "quiz", 20.0, 20.0, 3.0),
            s(None, "personal", "errand", 30.0, 30.0, 4.0),
        ];
        assert_eq!(ratio(&samples, Some("c:jpn"), "classes", "Quiz"), 2.0);
        // ELE has one quiz: not enough of its own, so the space's three speak.
        let space = ratio(&samples, Some("c:ele"), "classes", "quiz");
        assert!((space - (2.0 + 2.0 + 1.0) / 3.0).abs() < 1e-9);
        assert!((ratio(&samples, None, "wwav", "quiz") - 5.0 / 3.0).abs() < 1e-9);
        assert_eq!(
            ratio(&samples, None, "personal", "errand"),
            1.0,
            "one sample moves nothing"
        );
        assert_eq!(ratio(&samples, None, "personal", "lab"), 1.0);
    }

    #[test]
    fn the_ratio_reads_the_latest_eight_and_is_kept_in_bounds() {
        let mut samples: Vec<Sample> = (0..8)
            .map(|i| Sample {
                home: None,
                space_id: "s".into(),
                kind: "lab".into(),
                base_min: 100.0,
                actual_min: 100.0,
                done_at: 100.0 + i as f64,
            })
            .collect();
        // Older than the eight: it no longer counts.
        samples.push(Sample {
            home: None,
            space_id: "s".into(),
            kind: "lab".into(),
            base_min: 100.0,
            actual_min: 400.0,
            done_at: 1.0,
        });
        assert_eq!(ratio(&samples, None, "s", "lab"), 1.0);
        let wild: Vec<Sample> = (0..2)
            .map(|i| Sample {
                home: None,
                space_id: "s".into(),
                kind: "quiz".into(),
                base_min: 10.0,
                actual_min: 900.0,
                done_at: i as f64,
            })
            .collect();
        assert_eq!(ratio(&wild, None, "s", "quiz"), 3.0);
        assert_eq!(tidy_minutes(20.0 * 1.6), 30.0);
        assert_eq!(tidy_minutes(2.0), 5.0);
    }
}
