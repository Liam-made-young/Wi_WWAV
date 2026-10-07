//! The school mail path of a sync (`docs/SPEC.md` 3.1, 3.10): Heat searches
//! Gmail for `brightspace newer_than:Nd`, turns grade notices into pending
//! grades, and hands up to 8 announcements per sync to Claude, which pulls
//! out deadlines. The rest wait for the next sync, and the last 400 message
//! ids are remembered so nothing is read twice. A batch the server refuses
//! three times is given up on, so it can't hold up the mail behind it.
//!
//! The grade-notice phrasings below were written without a stored notice
//! from URI's Brightspace, the same Open item as the feed (11.2).

use crate::brightspace::{course_code, School};
use jiff::Timestamp;
use regex::Regex;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::OnceLock;

/// Message ids remembered, as the artifact does.
pub const REMEMBERED: usize = 400;
/// Announcements handed to Claude in one sync.
pub const FOR_CLAUDE_PER_SYNC: usize = 8;
pub const MORE_EMAILS_LEFT: &str = "More emails left, they'll come in on the next sync.";
/// A batch the server refuses this many times running is given up on, so it
/// can't hold up every message behind it.
pub const REFUSALS_BEFORE_GIVING_UP: u8 = 3;

const DAY_SECS: i64 = 86_400;

/// The Gmail search for a sync. N is the days since mail was last read
/// through, plus one so the boundary day is covered, kept within 2 to 14;
/// with no earlier sync it is 14.
pub fn query(read_through: Option<Timestamp>, now: Timestamp) -> String {
    let days = match read_through {
        None => 14,
        Some(t) => (now.duration_since(t).as_secs().div_euclid(DAY_SECS) + 1).clamp(2, 14),
    };
    format!("brightspace newer_than:{days}d")
}

/// One message from the search, as wi-core reads it from Gmail.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub from: String,
    pub subject: String,
    /// The plain-text body.
    pub text: String,
    pub received: Timestamp,
}

fn grade_notice() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(new grades?(:|\s+(posted|released|available|in|for)\b)|grades? (item )?(released|posted|updated|available)\b|grades? (has|have) been (released|posted|updated)\b|has been graded\b)",
        )
        .expect("valid")
    })
}

/// Brightspace's notice that a grade was released: "Grade released: Quiz 4",
/// "New grade in MTH 142", "Homework 5 has been graded". "New grade" alone
/// isn't enough: "New grade scale for the final project" is an announcement.
pub fn is_grade_notice(m: &Message) -> bool {
    grade_notice().is_match(&m.subject)
}

/// A grade Brightspace says is out. It carries no score: the notice doesn't
/// say it, so the person enters it, with a link to look it up.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingGrade {
    /// `gp-` and a hash of the message id, so a grade is made once.
    pub id: String,
    pub message_id: String,
    pub course: Option<String>,
    pub title: String,
    pub link: String,
    pub received: Timestamp,
}

pub(crate) fn hash_id(prefix: &str, key: &str) -> String {
    format!("{prefix}-{:016x}", wwav_ids::fnv1a64(key.as_bytes()))
}

/// The course named anywhere in `text`: the school's pattern tried at the
/// start of each word, since a subject puts the course where it likes.
fn course_in(school: &School, text: &str) -> Option<String> {
    let mut in_word = false;
    for (i, c) in text.char_indices() {
        let starts = c.is_alphanumeric() && !in_word;
        in_word = c.is_alphanumeric();
        if starts {
            if let Some(code) = course_code(&school.course_pattern, &text[i..]) {
                return Some(code);
            }
        }
    }
    None
}

fn pending_grade(m: &Message, school: &School) -> PendingGrade {
    PendingGrade {
        id: hash_id("gp", &m.id),
        message_id: m.id.clone(),
        course: course_in(school, &m.subject).or_else(|| course_in(school, &m.text)),
        title: m.subject.trim().to_string(),
        link: format!("https://{}", school.host),
        received: m.received,
    }
}

/// A task's notes when it comes from a message: they start "From mail:"
/// with the subject (3.10), and the caller links the task to the thread.
pub fn notes_from(m: &Message, notes: &str) -> String {
    let (subject, notes) = (m.subject.trim(), notes.trim());
    if notes.is_empty() {
        format!("From mail: {subject}")
    } else {
        format!("From mail: {subject}\n\n{notes}")
    }
}

/// Make a task with Claude off: the subject becomes the title and the body
/// becomes the notes. Returns (title, notes).
pub fn task_without_claude(m: &Message) -> (String, String) {
    (m.subject.trim().to_string(), notes_from(m, &m.text))
}

/// What a sync does with the messages its search returned.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MailPlan {
    pub grades: Vec<PendingGrade>,
    /// Oldest first, at most 8.
    pub for_claude: Vec<Message>,
    /// Announcements left for the next sync, oldest first.
    pub carried: Vec<Message>,
}

/// What mail reading remembers between syncs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MailState {
    /// The last 400 message ids handled, oldest first.
    pub processed: VecDeque<String>,
    /// Every message before this has been handled. It stays on the oldest
    /// message carried over, so the next search still reaches back to it.
    pub read_through: Option<Timestamp>,
    /// Refusals counted against each message of the last refused batch
    /// ([`MailState::refused`]). Save it with the rest, or the count starts
    /// over at every launch.
    pub refusals: BTreeMap<String, u8>,
}

impl MailState {
    pub fn seen(&self, id: &str) -> bool {
        self.processed.iter().any(|p| p == id)
    }

    pub fn remember(&mut self, id: &str) {
        if self.seen(id) {
            return;
        }
        self.processed.push_back(id.to_string());
        while self.processed.len() > REMEMBERED {
            self.processed.pop_front();
        }
    }

    /// Sorts new messages into pending grades and announcements. With
    /// reading off, announcements are left alone: nothing is sent and
    /// nothing waits for Claude.
    pub fn plan(&self, messages: &[Message], school: &School, reading_on: bool) -> MailPlan {
        // A message listed twice (Gmail's pages shift under new mail) counts once.
        let mut ids = HashSet::new();
        let mut fresh: Vec<&Message> = messages
            .iter()
            .filter(|m| !self.seen(&m.id) && ids.insert(m.id.as_str()))
            .collect();
        fresh.sort_by_key(|m| m.received);
        let mut plan = MailPlan::default();
        for m in fresh {
            if is_grade_notice(m) {
                plan.grades.push(pending_grade(m, school));
            } else if reading_on && plan.for_claude.len() < FOR_CLAUDE_PER_SYNC {
                plan.for_claude.push(m.clone());
            } else if reading_on {
                plan.carried.push(m.clone());
            }
        }
        plan
    }

    /// Records a finished sync. Grade notices are handled; the announcements
    /// sent to Claude are handled only if Claude read them. `searched_at` is
    /// when the search ran. Returns the line to show when mail is left over.
    pub fn finish(
        &mut self,
        plan: &MailPlan,
        claude_read: bool,
        searched_at: Timestamp,
    ) -> Option<&'static str> {
        for g in &plan.grades {
            self.remember(&g.message_id);
        }
        if claude_read {
            for m in &plan.for_claude {
                self.remember(&m.id);
            }
        }
        let mut left: Vec<&Message> = plan.carried.iter().collect();
        if claude_read {
            self.refusals.clear();
        } else {
            left.extend(&plan.for_claude);
        }
        let oldest_left = left
            .into_iter()
            .filter(|m| !self.seen(&m.id))
            .map(|m| m.received)
            .min();
        self.read_through = Some(oldest_left.unwrap_or(searched_at));
        oldest_left.map(|_| MORE_EMAILS_LEFT)
    }

    /// Records a sync whose batch the server refused ([`Failure::Refused`])
    /// or whose answer couldn't be read. Each message in the batch counts a
    /// refusal; one refused 3 times running is given up: remembered as
    /// handled, so the messages behind it come in. It stays in Mail, where
    /// "Make a task" still reads it.
    ///
    /// [`Failure::Refused`]: crate::assist::Failure::Refused
    pub fn refused(&mut self, plan: &MailPlan, searched_at: Timestamp) -> Option<&'static str> {
        let batch: HashSet<&str> = plan.for_claude.iter().map(|m| m.id.as_str()).collect();
        // A message no longer in the batch was read or has gone.
        self.refusals.retain(|id, _| batch.contains(id.as_str()));
        for m in &plan.for_claude {
            let n = self.refusals.entry(m.id.clone()).or_insert(0);
            *n += 1;
            if *n >= REFUSALS_BEFORE_GIVING_UP {
                self.refusals.remove(&m.id);
                self.remember(&m.id);
            }
        }
        self.finish(plan, false, searched_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brightspace::DEFAULT_COURSE_PATTERN;
    use jiff::tz::TimeZone;

    fn ts(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    fn school() -> School {
        School::new(
            "brightspace.uri.edu",
            DEFAULT_COURSE_PATTERN,
            TimeZone::get("America/New_York").unwrap(),
        )
        .unwrap()
    }

    fn msg(id: &str, subject: &str, received: &str) -> Message {
        Message {
            id: id.into(),
            thread_id: id.into(),
            from: "Brightspace <noreply@brightspace.uri.edu>".into(),
            subject: subject.into(),
            text: String::new(),
            received: ts(received),
        }
    }

    #[test]
    fn n_counts_days_since_the_last_read_within_2_to_14() {
        let now = ts("2026-10-06T12:40:00Z");
        assert_eq!(query(None, now), "brightspace newer_than:14d");
        assert_eq!(
            query(Some(ts("2026-10-06T11:40:00Z")), now),
            "brightspace newer_than:2d"
        );
        assert_eq!(
            query(Some(ts("2026-10-03T12:40:00Z")), now),
            "brightspace newer_than:4d"
        );
        assert_eq!(
            query(Some(ts("2026-10-03T13:00:00Z")), now),
            "brightspace newer_than:3d"
        );
        assert_eq!(
            query(Some(ts("2026-08-01T00:00:00Z")), now),
            "brightspace newer_than:14d"
        );
        assert_eq!(
            query(Some(ts("2026-10-07T00:00:00Z")), now),
            "brightspace newer_than:2d",
            "a clock ahead"
        );
    }

    #[test]
    fn grade_notices_become_pending_grades_with_a_link_and_no_score() {
        let s = school();
        let notices = [
            "Grade released: Grammar quiz 4 - JPN 201",
            "New grade posted in MTH 142",
            "Homework 5 has been graded",
            "Grades have been released for PHY 203",
            "New grade: Lab 5a",
            "New grade in WRT 104",
        ];
        for subject in notices {
            assert!(
                is_grade_notice(&msg("x", subject, "2026-10-05T14:00:00Z")),
                "{subject}"
            );
        }
        for subject in [
            "New announcement: office hours moved",
            "Grade expectations for the essay",
            "Upgrade your app",
            "New grade scale for the final project, due Friday",
            "New grading policy for late labs",
        ] {
            assert!(
                !is_grade_notice(&msg("x", subject, "2026-10-05T14:00:00Z")),
                "{subject}"
            );
        }
        let g = pending_grade(&msg("18c2a9", notices[0], "2026-10-05T14:00:00Z"), &s);
        assert_eq!(g.course.as_deref(), Some("JPN 201"));
        assert_eq!(g.link, "https://brightspace.uri.edu");
        assert_eq!(g.id, hash_id("gp", "18c2a9"));
        assert!(g.id.starts_with("gp-"));
        let mut quiet = msg(
            "18c2b0",
            "Homework 5 has been graded",
            "2026-10-05T14:00:00Z",
        );
        quiet.text = "Your instructor graded Homework 5 in MTH142 Sec 0004.".into();
        assert_eq!(
            pending_grade(&quiet, &s).course.as_deref(),
            Some("MTH 142"),
            "from the body when the subject has none"
        );
    }

    #[test]
    fn eight_announcements_go_to_claude_and_the_rest_wait() {
        let s = school();
        let mut state = MailState::default();
        let mut inbox: Vec<Message> = (0..11)
            .map(|n| {
                msg(
                    &format!("a{n:02}"),
                    &format!("Announcement {n}"),
                    &format!("2026-10-01T{n:02}:00:00Z"),
                )
            })
            .collect();
        inbox.push(msg("g1", "Grade released: Lab 5a", "2026-10-02T09:00:00Z"));
        inbox.reverse(); // Gmail lists newest first

        let plan = state.plan(&inbox, &s, true);
        assert_eq!(plan.grades.len(), 1);
        let sent: Vec<&str> = plan.for_claude.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            sent,
            ["a00", "a01", "a02", "a03", "a04", "a05", "a06", "a07"],
            "oldest first"
        );
        assert_eq!(plan.carried.len(), 3);

        let searched = ts("2026-10-06T12:40:00Z");
        assert_eq!(state.finish(&plan, true, searched), Some(MORE_EMAILS_LEFT));
        assert_eq!(state.read_through, Some(plan.carried[0].received));
        assert_eq!(state.processed.len(), 9);

        let next = state.plan(&inbox, &s, true);
        assert!(next.grades.is_empty(), "a grade notice is read once");
        assert_eq!(next.for_claude.len(), 3);
        assert_eq!(state.finish(&next, true, ts("2026-10-06T13:40:00Z")), None);
        assert_eq!(state.read_through, Some(ts("2026-10-06T13:40:00Z")));
    }

    #[test]
    fn when_claude_fails_the_announcements_wait() {
        let s = school();
        let mut state = MailState::default();
        let inbox = vec![msg("a1", "Lab moved to Friday", "2026-10-05T14:00:00Z")];
        let plan = state.plan(&inbox, &s, true);
        assert_eq!(
            state.finish(&plan, false, ts("2026-10-06T12:40:00Z")),
            Some(MORE_EMAILS_LEFT)
        );
        assert!(!state.seen("a1"));
        assert_eq!(state.read_through, Some(ts("2026-10-05T14:00:00Z")));
        assert_eq!(state.plan(&inbox, &s, true).for_claude.len(), 1);
    }

    #[test]
    fn a_batch_refused_three_times_is_given_up_and_the_rest_come_in() {
        let s = school();
        let mut state = MailState::default();
        let inbox: Vec<Message> = (0..10)
            .map(|n| {
                msg(
                    &format!("a{n:02}"),
                    &format!("Announcement {n}"),
                    &format!("2026-10-01T{n:02}:00:00Z"),
                )
            })
            .collect();
        let at = ts("2026-10-06T12:40:00Z");

        // Offline and limits aren't the batch's fault: they never count.
        for _ in 0..5 {
            let plan = state.plan(&inbox, &s, true);
            assert_eq!(state.finish(&plan, false, at), Some(MORE_EMAILS_LEFT));
        }
        for n in 1..=REFUSALS_BEFORE_GIVING_UP {
            let plan = state.plan(&inbox, &s, true);
            assert_eq!(plan.for_claude[0].id, "a00", "refusal {n}: the same batch");
            assert_eq!(state.refused(&plan, at), Some(MORE_EMAILS_LEFT));
        }
        assert!(state.seen("a07"), "given up after three refusals");
        assert!(state.refusals.is_empty());
        assert_eq!(state.read_through, Some(ts("2026-10-01T08:00:00Z")));
        let next = state.plan(&inbox, &s, true);
        let ids: Vec<&str> = next.for_claude.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["a08", "a09"], "the messages behind it come in");
        assert_eq!(state.finish(&next, true, at), None);
    }

    #[test]
    fn a_refusal_count_starts_over_when_the_batch_is_read() {
        let s = school();
        let mut state = MailState::default();
        let inbox = vec![msg("a1", "Lab moved to Friday", "2026-10-05T14:00:00Z")];
        let at = ts("2026-10-06T12:40:00Z");
        let plan = state.plan(&inbox, &s, true);
        state.refused(&plan, at);
        state.refused(&plan, at);
        assert_eq!(state.refusals.get("a1"), Some(&2));
        let more = vec![
            inbox[0].clone(),
            msg("a2", "Quiz moved to Monday", "2026-10-05T15:00:00Z"),
        ];
        // a1 went in with the batch that Claude read.
        let plan = state.plan(&more, &s, true);
        assert_eq!(state.finish(&plan, true, at), None);
        assert!(state.refusals.is_empty());
    }

    #[test]
    fn with_reading_off_only_grades_are_read() {
        let s = school();
        let mut state = MailState::default();
        let inbox = vec![
            msg("a1", "Lab moved to Friday", "2026-10-05T14:00:00Z"),
            msg("g1", "New grade posted in MTH 142", "2026-10-05T15:00:00Z"),
        ];
        let plan = state.plan(&inbox, &s, false);
        assert_eq!(
            (plan.grades.len(), plan.for_claude.len(), plan.carried.len()),
            (1, 0, 0)
        );
        assert_eq!(state.finish(&plan, false, ts("2026-10-06T12:40:00Z")), None);
    }

    #[test]
    fn with_claude_off_the_subject_is_the_title_and_the_body_the_notes() {
        let mut m = msg("a1", " Lab moved to Friday ", "2026-10-05T14:00:00Z");
        m.text = "Lab 6 meets Friday at 2 PM in East Hall.".into();
        let (title, notes) = task_without_claude(&m);
        assert_eq!(title, "Lab moved to Friday");
        assert_eq!(
            notes,
            "From mail: Lab moved to Friday\n\nLab 6 meets Friday at 2 PM in East Hall."
        );
    }

    #[test]
    fn the_last_400_ids_are_remembered() {
        let mut state = MailState::default();
        for n in 0..450 {
            state.remember(&format!("m{n}"));
        }
        state.remember("m449");
        assert_eq!(state.processed.len(), REMEMBERED);
        assert!(!state.seen("m49"));
        assert!(state.seen("m50"));
        assert_eq!(state.processed.back().map(String::as_str), Some("m449"));
    }
}
