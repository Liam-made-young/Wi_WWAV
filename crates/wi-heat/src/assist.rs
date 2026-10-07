//! Kept after the 7 Oct cut (`docs/SCOPE_CUT.md`): the app holds no key and
//! calls no model, and wi-core no longer makes these requests. Claude reaches
//! Heat through the MCP server's tools instead, and those tools reuse the
//! rules here: what each job may send, what an answer may say, and how an
//! answer is checked before it becomes a change. What follows is the module as
//! it was written for the `/api/assist` calls.
//!
//! The client side of Claude in Heat (`docs/SPEC.md` 2.11, 3.12, 9.8). Calls
//! go to mi-wwav.com's `/api/assist/:task`, one endpoint per job, which holds
//! the prompts and the founder's key and maps each tier to a model. This
//! module says what each call sends, what an answer may say, how a failure
//! reads and when to retry; wi-core makes the requests.
//!
//! Claude estimates and drafts, never decides: every answer here is a draft
//! for the person to accept, and each job has a fallback that works without
//! Claude.
//!
//! The server answers with the JSON object Claude was asked for. A limit is
//! a 429 whose body has `"code": "rate_limited"`, or `"code": "daily_limit"`
//! with the account's `"limit"`.

use crate::brightspace::due_on;
use crate::mail::{hash_id, notes_from, Message, MORE_EMAILS_LEFT};
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use rand::Rng;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

/// The jobs Heat gives Claude (3.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Score,
    ScoreBatch,
    ReadMail,
    Syllabus,
    WeeklyReview,
    ReleasePlan,
}

/// Heat's two tiers: "quick" for scoring, "default" for reading and drafting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Quick,
    Default,
}

impl Task {
    pub fn path(self) -> &'static str {
        match self {
            Task::Score => "/api/assist/score",
            Task::ScoreBatch => "/api/assist/score-batch",
            Task::ReadMail => "/api/assist/read-mail",
            Task::Syllabus => "/api/assist/syllabus",
            Task::WeeklyReview => "/api/assist/weekly-review",
            Task::ReleasePlan => "/api/assist/release-plan",
        }
    }

    pub fn tier(self) -> Tier {
        match self {
            Task::Score | Task::ScoreBatch => Tier::Quick,
            _ => Tier::Default,
        }
    }
}

/// Shown at the first use of scoring, which covers scoring one task and
/// scoring synced tasks in a batch.
pub const SCORING_CONSENT: &str =
    "Learn will send this task's title, type and notes, and your average minutes per type. Nothing else.";
/// Reading email has its own switch. The spec gives it no sentence, so this
/// one says what 2.11 lists it sending, in scoring's words, until the founder
/// writes one.
pub const MAIL_CONSENT: &str =
    "Learn will send up to 8 school emails per sync, today's date and your time zone, and up to 80 of your task titles. Nothing else.";
pub const TURN_ON_SCORING: &str = "Turn on scoring";
pub const NOT_NOW: &str = "Not now";

/// A feature's switch. Each is off until its first use asks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Consent {
    #[default]
    Unasked,
    On,
    Off,
}

/// What a feature's button does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Show what will be sent, with the turn-on button and "Not now".
    Ask,
    Go,
    /// The button hides; for scoring, with [`Failure::NotGranted`]'s line.
    Hidden,
    /// The button reads "Needs a connection".
    NeedsConnection,
}

pub fn gate(consent: Consent, online: bool) -> Gate {
    match (consent, online) {
        (Consent::Off, _) => Gate::Hidden,
        (_, false) => Gate::NeedsConnection,
        (Consent::Unasked, true) => Gate::Ask,
        (Consent::On, true) => Gate::Go,
    }
}

/// The daily limit 10.4 proposes, for a server that doesn't say its own.
pub const DAILY_LIMIT: u32 = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    NotGranted,
    RateLimited,
    Offline,
    DailyLimit {
        limit: u32,
    },
    /// A server error after the retry, or an answer that can't be read. The
    /// spec gives this case no words; this line holds the place until the
    /// founder writes one.
    Unavailable,
    /// The server refused this request's body (400, 413 or 422): sending it
    /// again won't help. It reads as [`Failure::Unavailable`] does; reading
    /// mail counts it against the batch ([`crate::mail::MailState::refused`]).
    Refused,
}

impl Failure {
    pub fn sentence(&self) -> String {
        match self {
            Failure::NotGranted => "Claude scoring is off. Set difficulty yourself.".into(),
            Failure::RateLimited => "Too many requests. Wait a minute, then try again.".into(),
            Failure::Offline => "Needs a connection".into(),
            Failure::DailyLimit { limit } => {
                format!("Claude's {limit} calls for today are used. They come back at midnight.")
            }
            Failure::Unavailable | Failure::Refused => {
                "Claude couldn't answer. Try again later.".into()
            }
        }
    }
}

/// The one line a failed job shows, if any. Batch scoring of synced items
/// fails silently and keeps the per-type defaults; reading mail leaves the
/// rest for the next sync; a switched-off drafting job just hides.
pub fn failure_line(task: Task, failure: &Failure) -> Option<String> {
    match (task, failure) {
        (Task::ScoreBatch, _) => None,
        (Task::ReadMail, Failure::NotGranted) => None,
        (Task::ReadMail, _) => Some(MORE_EMAILS_LEFT.to_string()),
        (Task::Score, _) => Some(failure.sentence()),
        (_, Failure::NotGranted) => None,
        (_, _) => Some(failure.sentence()),
    }
}

pub const RETRY_AFTER: Duration = Duration::from_millis(1500);
pub const RETRY_JITTER_MS: u64 = 800;

/// How a request ended, as wi-core saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Answered {
        status: u16,
        body: String,
    },
    /// No connection at all.
    Offline,
    /// Sent, but no answer in time.
    TimedOut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Done(Result<String, Failure>),
    RetryAfter(Duration),
}

/// What to do after attempt number `attempt` (from 1). A server error or a
/// timeout gets one retry after 1500 ms plus up to 800 ms of jitter drawn
/// from `rng`; nothing else is retried.
pub fn step<R: Rng + ?Sized>(attempt: u32, outcome: Outcome, rng: &mut R) -> Step {
    let retry = |rng: &mut R| {
        if attempt <= 1 {
            Step::RetryAfter(
                RETRY_AFTER + Duration::from_millis(rng.gen_range(0..=RETRY_JITTER_MS)),
            )
        } else {
            Step::Done(Err(Failure::Unavailable))
        }
    };
    match outcome {
        Outcome::Answered {
            status: 200..=299,
            body,
        } => Step::Done(Ok(body)),
        Outcome::Answered { status: 429, body } => Step::Done(Err(limit(&body))),
        Outcome::Answered {
            status: 408 | 500..=599,
            ..
        } => retry(rng),
        Outcome::Answered {
            status: 400 | 413 | 422,
            ..
        } => Step::Done(Err(Failure::Refused)),
        Outcome::Answered { .. } => Step::Done(Err(Failure::Unavailable)),
        Outcome::Offline => Step::Done(Err(Failure::Offline)),
        Outcome::TimedOut => retry(rng),
    }
}

fn limit(body: &str) -> Failure {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if v["code"] == "daily_limit" {
        let limit = v["limit"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(DAILY_LIMIT);
        Failure::DailyLimit { limit }
    } else {
        Failure::RateLimited
    }
}

// ----- what each job sends -----

/// Score a task: exactly what [`SCORING_CONSENT`] names.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScoreBody {
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub notes: String,
    /// Average minutes per type, from measured time.
    pub averages: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BatchTask {
    pub i: usize,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub notes: String,
}

/// Batch-score synced tasks: an indexed list.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScoreBatchBody {
    pub tasks: Vec<BatchTask>,
    pub averages: BTreeMap<String, u32>,
}

pub const MAIL_TITLES: usize = 80;
/// The most a read-mail body may weigh as JSON: under the server's 100 KB
/// `express.json()` limit, with room to spare. A body the server refuses
/// would fail the same way every sync.
pub const MAIL_BODY_BYTES: usize = 90 * 1024;
/// Each message's text is cut at this many characters, and shorter still
/// when the body would pass [`MAIL_BODY_BYTES`].
pub const MAIL_TEXT_CHARS: usize = 6000;
/// A sender, subject or title is one line; a longer one is cut here.
pub const MAIL_LINE_CHARS: usize = 300;
/// The titles together weigh at most this much as JSON; the rest are left
/// out, so the messages always keep most of the body.
pub const MAIL_TITLES_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MailMessage {
    pub id: String,
    pub from: String,
    pub subject: String,
    /// When it arrived, RFC 3339 in UTC.
    pub date: String,
    pub text: String,
}

/// Read school email: up to 8 messages, today's date and zone, and up to 80
/// existing titles so Claude doesn't make a task twice.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReadMailBody {
    pub messages: Vec<MailMessage>,
    pub today: String,
    pub zone: String,
    pub titles: Vec<String>,
}

/// What `c` weighs inside a JSON string, as serde_json writes it.
fn json_len(c: char) -> usize {
    match c {
        '"' | '\\' | '\u{8}' | '\u{c}' | '\n' | '\r' | '\t' => 2,
        c if c < ' ' => 6,
        c => c.len_utf8(),
    }
}

/// The longest start of `s` that weighs at most `bytes` as JSON and holds at
/// most `chars` characters. It ends on a character boundary.
fn cut(s: &str, chars: usize, bytes: usize) -> String {
    let mut weight = 0;
    s.chars()
        .take(chars)
        .take_while(|&c| {
            weight += json_len(c);
            weight <= bytes
        })
        .collect()
}

/// Shares `budget` among items wanting `wants`: each gets what it wants up
/// to an equal share, and what a short one leaves over goes to the longer.
fn share_out(wants: &[usize], budget: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..wants.len()).collect();
    order.sort_by_key(|&i| wants[i]);
    let mut left = budget;
    let mut got = vec![0; wants.len()];
    for (k, &i) in order.iter().enumerate() {
        got[i] = wants[i].min(left / (wants.len() - k));
        left -= got[i];
    }
    got
}

/// The body for the first 8 `messages` and the first 80 `titles`; pass the
/// titles that matter most first (open tasks, nearest due). It never weighs
/// more than [`MAIL_BODY_BYTES`]: titles stop at [`MAIL_TITLES_BYTES`], and
/// the texts share what is left, so one long email can't crowd out the rest.
pub fn read_mail_body(
    messages: &[Message],
    now: Timestamp,
    zone: &TimeZone,
    titles: &[String],
) -> ReadMailBody {
    let line = |s: &str| cut(s.trim(), MAIL_LINE_CHARS, usize::MAX);
    let mut weight = 0;
    let titles = titles
        .iter()
        .take(MAIL_TITLES)
        .map(|t| line(t))
        .take_while(|t| {
            weight += t.chars().map(json_len).sum::<usize>() + 3; // quotes and comma
            weight <= MAIL_TITLES_BYTES
        })
        .collect();
    let messages: Vec<&Message> = messages
        .iter()
        .take(crate::mail::FOR_CLAUDE_PER_SYNC)
        .collect();
    let mut body = ReadMailBody {
        messages: messages
            .iter()
            .map(|m| MailMessage {
                id: m.id.clone(),
                from: line(&m.from),
                subject: line(&m.subject),
                date: m.received.to_string(),
                text: String::new(),
            })
            .collect(),
        today: zone.to_datetime(now).date().to_string(),
        zone: zone
            .iana_name()
            .map_or_else(|| zone.to_offset(now).to_string(), str::to_string),
        titles,
    };
    let bare = serde_json::to_vec(&body).map_or(MAIL_BODY_BYTES, |v| v.len());
    let texts: Vec<String> = messages
        .iter()
        .map(|m| cut(&m.text, MAIL_TEXT_CHARS, usize::MAX))
        .collect();
    let wants: Vec<usize> = texts
        .iter()
        .map(|t| t.chars().map(json_len).sum())
        .collect();
    let shares = share_out(&wants, MAIL_BODY_BYTES.saturating_sub(bare));
    for ((m, text), share) in body.messages.iter_mut().zip(&texts).zip(shares) {
        m.text = cut(text, usize::MAX, share);
    }
    body
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SyllabusBody {
    pub text: String,
}

/// Facts Heat computed for the week, one per line. Claude may only restate
/// their numbers.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReviewBody {
    pub facts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReleasePlanBody {
    pub title: String,
    /// `YYYY-MM-DD`.
    pub release_date: String,
    /// The linked work's title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work: Option<String>,
}

// ----- what an answer may say -----

/// The JSON object in an answer, even when a sentence or a code fence
/// surrounds it.
fn json_in(body: &str) -> Option<Value> {
    let object = |s: &str| match serde_json::from_str::<Value>(s) {
        Ok(v @ Value::Object(_)) => Some(v),
        _ => None,
    };
    object(body).or_else(|| object(body.get(body.find('{')?..=body.rfind('}')?)?))
}

fn number(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str()?.trim().parse().ok())
        .filter(|n: &f64| n.is_finite())
}

fn text(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn difficulty(n: f64) -> u8 {
    n.round().clamp(1.0, 5.0) as u8
}

fn minutes(n: f64) -> u32 {
    n.round().clamp(5.0, 600.0) as u32
}

/// Claude's estimate for one task: difficulty 1-5, minutes 5-600, and the
/// one-sentence reason shown as the field's hint.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub difficulty: u8,
    pub minutes: u32,
    pub reason: String,
}

/// `{difficulty, minutes, reason}`. An answer without its reason is not a
/// suggestion Heat shows.
pub fn parse_score(body: &str) -> Option<Score> {
    let v = json_in(body)?;
    Some(Score {
        difficulty: difficulty(number(&v["difficulty"])?),
        minutes: minutes(number(&v["minutes"])?),
        reason: text(&v["reason"])?,
    })
}

/// The estimate without Claude: the type's average, else difficulty × 20
/// minutes.
pub fn fallback_minutes(type_average: Option<u32>, difficulty: u8) -> u32 {
    type_average.unwrap_or(u32::from(difficulty.clamp(1, 5)) * 20)
}

/// `{"scores":[{"i", "difficulty", "minutes"}]}`, lined up with the list
/// sent. A task left out keeps its per-type defaults.
pub fn parse_batch(body: &str, count: usize) -> Vec<Option<(u8, u32)>> {
    let mut out = vec![None; count];
    let Some(v) = json_in(body) else { return out };
    for s in v["scores"].as_array().into_iter().flatten() {
        let Some(i) = s["i"]
            .as_u64()
            .and_then(|i| usize::try_from(i).ok())
            .filter(|&i| i < count)
        else {
            continue;
        };
        if let (Some(d), Some(m)) = (number(&s["difficulty"]), number(&s["minutes"])) {
            out[i] = Some((difficulty(d), minutes(m)));
        }
    }
    out
}

/// A deadline Claude found in an email.
#[derive(Clone, Debug, PartialEq)]
pub struct MailTask {
    /// `em-` and a hash of the message id and title.
    pub id: String,
    /// The message it came from, when Claude named one that was sent (or
    /// only one was).
    pub message_id: Option<String>,
    pub title: String,
    pub due: Timestamp,
    pub course: Option<String>,
    pub kind: Option<String>,
    /// "From mail:" and the subject, then Claude's notes.
    pub notes: String,
}

fn deadline(s: &str, zone: &TimeZone) -> Option<Timestamp> {
    let s = s.trim();
    if let Ok(t) = s.parse::<Timestamp>() {
        return Some(t);
    }
    if s.len() == 10 {
        return due_on(s.parse::<Date>().ok()?, zone);
    }
    zone.to_ambiguous_timestamp(s.parse::<DateTime>().ok()?)
        .compatible()
        .ok()
}

/// `{"tasks":[{message, title, due, course?, type?, notes?}]}`. Only items
/// with a title and a deadline still ahead are kept: Claude is asked for
/// future deadlines, and the client holds it to that. A date alone is due
/// 11:59 PM; a time without an offset is the person's.
pub fn parse_mail_tasks(
    body: &str,
    sent: &[Message],
    now: Timestamp,
    zone: &TimeZone,
) -> Option<Vec<MailTask>> {
    let v = json_in(body)?;
    let mut tasks = Vec::new();
    for t in v.get("tasks")?.as_array()? {
        let (Some(title), Some(due)) = (
            text(&t["title"]),
            t["due"].as_str().and_then(|s| deadline(s, zone)),
        ) else {
            continue;
        };
        if due <= now {
            continue;
        }
        let message = t["message"]
            .as_str()
            .and_then(|id| sent.iter().find(|m| m.id == id))
            .or(if sent.len() == 1 { sent.first() } else { None });
        let notes = text(&t["notes"]).unwrap_or_default();
        let message_id = message.map(|m| m.id.clone());
        let id = hash_id(
            "em",
            &format!("{}\n{title}", message_id.as_deref().unwrap_or("")),
        );
        tasks.push(MailTask {
            id,
            message_id,
            title,
            due,
            course: text(&t["course"]),
            kind: text(&t["type"]),
            notes: match message {
                Some(m) => notes_from(m, &notes),
                None => notes,
            },
        });
    }
    Some(tasks)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    pub name: String,
    pub weight: f64,
    pub keywords: Vec<String>,
}

/// Categories, weights, scale and keywords, as a list to check. Nothing is
/// saved until the person presses Save.
#[derive(Clone, Debug, PartialEq)]
pub struct SyllabusDraft {
    pub categories: Vec<Category>,
    /// (letter, lowest percent), highest first.
    pub scale: Vec<(String, f64)>,
}

impl SyllabusDraft {
    /// "Claude found 5 categories adding to 100%. Check them against the
    /// syllabus."
    pub fn check_line(&self) -> String {
        let n = self.categories.len();
        let total = (self.categories.iter().map(|c| c.weight).sum::<f64>() * 10.0).round() / 10.0;
        let noun = if n == 1 { "category" } else { "categories" };
        format!("Claude found {n} {noun} adding to {total}%. Check them against the syllabus.")
    }
}

pub fn parse_syllabus(body: &str) -> Option<SyllabusDraft> {
    let v = json_in(body)?;
    let strings = |v: &Value| {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(text)
            .collect::<Vec<_>>()
    };
    let categories: Vec<Category> = v["categories"]
        .as_array()?
        .iter()
        .filter_map(|c| {
            Some(Category {
                name: text(&c["name"])?,
                weight: number(&c["weight"])?,
                keywords: strings(&c["keywords"]),
            })
        })
        .collect();
    let scale = v["scale"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some((text(&s["letter"])?, number(&s["min"])?)))
        .collect();
    (!categories.is_empty()).then_some(SyllabusDraft { categories, scale })
}

/// How a number word combines with the words before it.
#[derive(Clone, Copy)]
enum Joins {
    /// "twenty-three": adds.
    Adds,
    /// "three hundred", "two dozen": multiplies what came before.
    Times,
    /// "four thousand five hundred": closes a group of three digits.
    Group,
}

fn number_word(w: &str) -> Option<(u64, Joins)> {
    const SMALL: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    if let Some(n) = SMALL.iter().position(|s| *s == w) {
        return Some((n as u64, Joins::Adds));
    }
    if let Some(n) = TENS.iter().position(|s| *s == w) {
        return Some((n as u64 * 10 + 20, Joins::Adds));
    }
    match w {
        "dozen" => Some((12, Joins::Times)),
        "hundred" => Some((100, Joins::Times)),
        "thousand" => Some((1_000, Joins::Group)),
        "million" => Some((1_000_000, Joins::Group)),
        "billion" => Some((1_000_000_000, Joins::Group)),
        _ => None,
    }
}

/// The value of a run of number words. "one" on its own has none: it is as
/// often the article ("one thing", as in the review's own "Next week's one
/// thing"), and "a task" says the same without being checked either.
fn words_value(run: &[&str]) -> Option<u64> {
    if run.is_empty() || run == ["one"] {
        return None;
    }
    let (mut total, mut current) = (0u64, 0u64);
    for (v, joins) in run.iter().filter_map(|w| number_word(w)) {
        match joins {
            Joins::Adds => current = current.saturating_add(v),
            Joins::Times => current = current.max(1).saturating_mul(v),
            Joins::Group => {
                total = total.saturating_add(current.max(1).saturating_mul(v));
                current = 0;
            }
        }
    }
    Some(total.saturating_add(current))
}

/// Every number `s` states, as digits: digit runs ("14", the "1" and "5" of
/// "1.5") and number words ("twenty", "forty-two", "two hundred and five").
/// Number words run together across spaces and hyphens; anything else ends
/// the number.
fn numbers(s: &str) -> Vec<String> {
    let mut out: Vec<String> = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|n| !n.is_empty())
        .map(|n| match n.trim_start_matches('0') {
            "" => "0".to_string(),
            n => n.to_string(),
        })
        .collect();

    // Each word, and whether only spaces and hyphens part it from the last.
    let lower = s.to_lowercase();
    let mut words = Vec::new();
    let mut rest = lower.as_str();
    while let Some(start) = rest.find(char::is_alphabetic) {
        let joined = rest[..start].chars().all(|c| c == ' ' || c == '-');
        let tail = &rest[start..];
        let end = tail
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(tail.len());
        words.push((&tail[..end], joined));
        rest = &tail[end..];
    }

    let mut run: Vec<&str> = Vec::new();
    let mut and = false; // an "and" follows the run so far
    let mut flush = |run: &mut Vec<&str>| {
        out.extend(words_value(run).map(|v| v.to_string()));
        run.clear();
    };
    for (word, joined) in words {
        let continues = joined && !run.is_empty();
        if number_word(word).is_some() {
            if !continues {
                flush(&mut run);
            }
            run.push(word);
            and = false;
        } else if word == "and" && continues && !and {
            and = true;
        } else {
            flush(&mut run);
            and = false;
        }
    }
    flush(&mut run);
    out
}

/// `{"draft": "..."}`, kept only if every number in it, in digits or in
/// words, is one of the facts' ("NEVER invent metrics"); otherwise the facts
/// stand alone. Ordinals and words such as "half" or "twice" aren't read;
/// the server's prompt asks for numbers in digits.
pub fn check_review(body: &str, facts: &[String]) -> Option<String> {
    let draft = text(&json_in(body)?["draft"])?;
    let known: HashSet<String> = facts.iter().flat_map(|f| numbers(f)).collect();
    let honest = numbers(&draft).iter().all(|n| known.contains(n));
    honest.then_some(draft)
}

/// Tasks for each phase of a release, in Ripple Creator's phases.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleasePlanDraft {
    pub pre: Vec<String>,
    pub launch: Vec<String>,
    pub post: Vec<String>,
}

pub fn parse_release_plan(body: &str) -> Option<ReleasePlanDraft> {
    let v = json_in(body)?;
    let list = |k: &str| {
        v[k].as_array()
            .into_iter()
            .flatten()
            .filter_map(text)
            .collect::<Vec<_>>()
    };
    let plan = ReleasePlanDraft {
        pre: list("pre"),
        launch: list("launch"),
        post: list("post"),
    };
    (!(plan.pre.is_empty() && plan.launch.is_empty() && plan.post.is_empty())).then_some(plan)
}

/// The three milestones of a release plan (3.14), which are also the
/// template when Claude can't draft: Pre-release 28 days before, Release
/// day, Post-release 28 days after.
pub fn release_milestones(release: Date) -> [(&'static str, Date); 3] {
    let shift = |days: i64| {
        release
            .checked_add(Span::new().days(days))
            .unwrap_or(release)
    };
    [
        ("Pre-release", shift(-28)),
        ("Release day", release),
        ("Post-release", shift(28)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_weighs_what_serde_json_writes() {
        let odd = ['é', '漢', '🎵', '\u{7f}', '\u{2028}', '/'];
        for c in (0..=0x7f).filter_map(char::from_u32).chain(odd) {
            let written = serde_json::to_string(&c.to_string()).unwrap().len() - 2;
            assert_eq!(json_len(c), written, "{c:?}");
        }
    }

    #[test]
    fn shares_go_to_whoever_needs_them() {
        assert_eq!(share_out(&[10, 500, 40], 300), vec![10, 250, 40]);
        assert_eq!(share_out(&[500, 500], 300), vec![150, 150]);
        assert_eq!(share_out(&[], 300), Vec::<usize>::new());
        assert_eq!(cut("漢字", 9, 5), "漢", "a cut never splits a character");
    }
}
