//! Heat's records, as `docs/SPEC.md` 3.16 gives them: a port of
//! `app/ui/src/heat/model/records.ts`, which predates 3.16, brought up to it.
//! Instants are epoch ms. Days are `"YYYY-MM-DD"` in the person's time zone. A
//! block's `start` is minutes after that day's local midnight, as the time
//! column reads it.
//!
//! The serde names are the TypeScript JSON field names, so a record read from
//! the store or written for the app round-trips as it is. A number is an `f64`
//! because the TypeScript's is one number type, and the model does its sums on
//! it as JavaScript does; a whole number is written as `45`, not `45.0`, as
//! `JSON.stringify` writes it. An optional field that is absent in TypeScript
//! is `None` here and left out of the JSON; a field that is `null` there is an
//! `Option` that is written as `null`.
//!
//! What 3.16 added to the TypeScript's records is read when it is absent and
//! left out of the JSON when it is at its default (`false`, `None`, the plain
//! timer's source), so a record the TypeScript wrote reads and writes back as
//! it was, byte for byte. The values the TypeScript had and 3.16 dropped
//! (`TaskSource::Calendar`, `CalendarEvent`, `SyncState`) stay, marked Legacy
//! with what replaces each, because moving in from the artifact still makes
//! them.

use super::focus::Phase;
use super::plan::Draft;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use super::zone::DayKey;

fn is_false(b: &bool) -> bool {
    !*b
}

pub type Id = String;

/// Serializers that write a number the way `JSON.stringify` does.
pub mod ser {
    use serde::Serializer;

    /// A whole number as an integer (`45`), anything else as a float.
    pub fn num<S: Serializer>(x: &f64, s: S) -> Result<S::Ok, S::Error> {
        if x.is_finite() && x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
            s.serialize_i64(*x as i64)
        } else {
            s.serialize_f64(*x)
        }
    }

    /// An `Option<f64>`: `null` for none.
    pub fn opt_num<S: Serializer>(x: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
        match x {
            Some(v) => num(v, s),
            None => s.serialize_none(),
        }
    }
}

/// Where Heat can be: the view switcher's three views (the spec has no fourth).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Room {
    #[default]
    Heat,
    Space,
    Console,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupKind {
    Course,
    Milestone,
    Free,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: Id,
    pub name: String,
    #[serde(serialize_with = "ser::num")]
    pub hue: f64,
    pub group_kind: GroupKind,
    pub group_label: String,
    pub types: Vec<String>,
    pub persona: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Session,
    System,
    Work,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub kind: LinkKind,
    pub id: Id,
}

/// Where a task came from; Get Info names it ("Brightspace calendar").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskSource {
    #[default]
    You,
    /// Legacy: the artifact's Google Calendar path. 3.16 reads Brightspace and other calendars as iCal feeds, so a new task says `Ical`.
    Calendar,
    Ical,
    /// Legacy: the artifact's Gmail path. In 3.16 Claude reads the mail and adds the task, so it says `Claude`, with the message id as its `sourceId`.
    Mail,
    /// Legacy: an inbox capture made into a task. 3.16 has no source for it; the task is `You`'s.
    Capture,
    Claude,
}

/// Who made a task's estimate: you typed it, Claude scored it, or it is the
/// estimate chain's default (the type's average, else difficulty × 20).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EstBy {
    You,
    Claude,
    Default,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: Id,
    pub space_id: Id,
    pub title: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone_id: Option<Id>,
    /// The group of a space whose group kind is "free" (Personal's Area). 3.15 has no field for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<Id>,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub due: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_date: Option<DayKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rrule: Option<String>,
    #[serde(serialize_with = "ser::num")]
    pub difficulty: f64,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub est_min: Option<f64>,
    /// Who made `est_min`. None for a task from before 3.16, whose estimate says nothing of its maker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub est_by: Option<EstBy>,
    /// Claude's one-sentence reason for its estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub est_reason: Option<String>,
    #[serde(serialize_with = "ser::num")]
    pub adjust_min: f64,
    pub notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    pub done: bool,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub done_at: Option<f64>,
    pub source: TaskSource,
    /// What the source calls the task: a Gmail message id, a feed's VEVENT UID, the artifact's `em-` and `gp-` hashes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Whether the person has switched this task onto their public Heat view (3.15).
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

/// A finished occurrence of a recurring task. Its presence is the tick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOccurrence {
    pub id: Id,
    pub task_id: Id,
    pub date: DayKey,
    #[serde(serialize_with = "ser::num")]
    pub done_at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    You,
    Plan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeBlock {
    pub id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub habit_id: Option<Id>,
    pub date: DayKey,
    #[serde(serialize_with = "ser::num")]
    pub start: f64,
    #[serde(serialize_with = "ser::num")]
    pub minutes: f64,
    pub origin: Origin,
}

/// Who logged a focus record: the timer, or Claude through `log_focus` (3.13).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FocusSource {
    #[default]
    Timer,
    Claude,
}

impl FocusSource {
    fn is_timer(&self) -> bool {
        *self == FocusSource::Timer
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSession {
    pub id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub habit_id: Option<Id>,
    #[serde(serialize_with = "ser::num")]
    pub started_at: f64,
    #[serde(serialize_with = "ser::num")]
    pub ended_at: f64,
    #[serde(serialize_with = "ser::num")]
    pub focus_min: f64,
    #[serde(serialize_with = "ser::num")]
    pub interruptions: f64,
    /// The view the timer ran in. 3.16 calls it `view`; the TypeScript, which
    /// this has to agree with until the app is rewired, writes `room`. Either
    /// name is read, and `room` is written.
    #[serde(rename = "room", alias = "view")]
    pub view: Room,
    #[serde(default, skip_serializing_if = "FocusSource::is_timer")]
    pub source: FocusSource,
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    #[default]
    Active,
    OnHold,
    Someday,
    Archived,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: Id,
    pub space_id: Id,
    pub title: String,
    pub status: ProjectStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_date: Option<DayKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Milestone {
    pub id: Id,
    pub space_id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Id>,
    pub title: String,
    pub date: DayKey,
    pub done: bool,
    #[serde(serialize_with = "ser::num")]
    pub order: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Habit {
    pub id: Id,
    pub title: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser::opt_num"
    )]
    pub minutes: Option<f64>,
    /// The days it was done. Never pruned.
    pub log: BTreeMap<DayKey, bool>,
    pub show_counter: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Term {
    pub id: Id,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradeCategory {
    pub id: Id,
    pub name: String,
    #[serde(serialize_with = "ser::num")]
    pub weight: f64,
    pub keywords: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LetterStep {
    pub letter: String,
    #[serde(serialize_with = "ser::num")]
    pub min: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub id: Id,
    pub term_id: Id,
    pub code: String,
    pub name: String,
    pub categories: Vec<GradeCategory>,
    /// The course's own scale, highest first; the default scale when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<Vec<LetterStep>>,
    pub notes: String,
    /// Shows only the course's code and name on the public Heat view, never its grades or percentage (3.15).
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GradeSource {
    #[default]
    You,
    /// Legacy: a grade notice from the artifact's mail path. In 3.16 Claude adds the pending grade, so it says `Claude`.
    Mail,
    Valence,
    Claude,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grade {
    pub id: Id,
    pub course_id: Id,
    pub category_id: Option<Id>,
    pub title: String,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub score: Option<f64>,
    #[serde(serialize_with = "ser::num")]
    pub out_of: f64,
    pub dropped: bool,
    /// A grade notice from mail, waiting for its score ("Enter score").
    pub pending: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    pub source: GradeSource,
    /// Each grade has its own switch, off by default; it shows the course, the item, the score and what it was out of (3.15).
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultType {
    Task,
    Note,
    Project,
    Upload,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    pub id: Id,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser::opt_num"
    )]
    pub triaged_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_type: Option<ResultType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_id: Option<Id>,
}

/// Legacy, TypeScript only: a Google Calendar event, read-only and drawn grey behind blocks (3.5). 3.16 stores no events; the time column reads them from each `Calendar`'s iCal feed (3.11).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: Id,
    pub title: String,
    #[serde(serialize_with = "ser::num")]
    pub start: f64,
    #[serde(serialize_with = "ser::num")]
    pub end: f64,
    pub all_day: bool,
}

/// Legacy, TypeScript only: what the artifact's syncs remember between runs. 3.16 has no such record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    /// Legacy: the last 400 processed Gmail ids. In 3.16 `Task.sourceId` and `MailThread` replace them, so Claude can read the same mail twice and leave nothing doubled (3.13).
    pub processed_mail_ids: Vec<String>,
    /// Legacy: the last sync's time. In 3.16 each `Calendar` keeps its own `lastSyncedAt`.
    #[serde(default, serialize_with = "ser::opt_num")]
    pub last_sync_at: Option<f64>,
}

/// What Claude decided about a mail thread, kept by `record_mail_thread` (3.13).
/// It holds no body, and it is never public.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MailState {
    Grade,
    Task,
    Nothing,
}

/// Who made a mail thread's row. Only Claude does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordedBy {
    #[default]
    Claude,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailThread {
    pub id: Id,
    /// Gmail's id for the thread: the same one again updates the row rather than adding one.
    pub gmail_thread_id: String,
    pub subject: String,
    pub from: String,
    #[serde(serialize_with = "ser::num")]
    pub received_at: f64,
    /// The course code the mail is about, such as `JPN 201`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course: Option<String>,
    pub state: MailState,
    /// Claude's one-sentence reason.
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<Id>,
    #[serde(default)]
    pub recorded_by: RecordedBy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalendarKind {
    Brightspace,
    Ical,
}

/// A calendar Heat reads (3.11). The address itself is in the Keychain, under `keychain_ref`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calendar {
    pub id: Id,
    pub name: String,
    pub kind: CalendarKind,
    pub keychain_ref: String,
    /// None until the first sync; written as `null`, as the spec's `lastSyncedAt` has no `?`.
    #[serde(default, serialize_with = "ser::opt_num")]
    pub last_synced_at: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DailyNote {
    pub date: DayKey,
    pub markdown: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

/// The note that can go public (3.15's table); a daily note stays a `DailyNote`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub markdown: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShareKind {
    Now,
    Timeline,
}

/// The two default public items themselves: the Now making line and a project's
/// timeline. A row exists only once the person presses Show (3.15).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileShare {
    pub id: Id,
    pub kind: ShareKind,
    /// The task a Now making line came from, or the project a timeline shows.
    pub source_id: Id,
    /// The line the person approved, for a Now making share.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// What it hangs on: your sun, or the project's solar system.
    pub target_id: Id,
    /// When a Now making line clears by itself: 7 days after it was set, or when its task is done.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser::opt_num"
    )]
    pub clears_at: Option<f64>,
}

/// The timer as it is kept between launches: its phase, round and when it ends.
/// 3.16's shape, which holds no paused round's time left (docs/QUESTIONS.md #88).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatTimer {
    pub phase: Phase,
    #[serde(serialize_with = "ser::num")]
    pub round: f64,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub ends_at: Option<f64>,
}

impl Default for HeatTimer {
    fn default() -> Self {
        HeatTimer {
            phase: Phase::Idle,
            round: 1.0,
            ends_at: None,
        }
    }
}

/// What Heat holds between launches that is not a record: the current task, the
/// timer, and the drafts Plan my day has made and nobody has accepted yet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_task_id: Option<Id>,
    #[serde(default)]
    pub timer: HeatTimer,
    #[serde(default)]
    pub plan_drafts: Vec<Draft>,
}
