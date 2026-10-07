//! Heat's records: a port of `app/ui/src/heat/model/records.ts`, the shapes
//! `docs/SPEC.md` 3.16 gives. Instants are epoch ms. Days are `"YYYY-MM-DD"`
//! in the person's time zone. A block's `start` is minutes after that day's
//! local midnight, as the time column reads it.
//!
//! The serde names are the TypeScript JSON field names, so a record read from
//! the store or written for the app round-trips as it is. A number is an `f64`
//! because the TypeScript's is one number type, and the model does its sums on
//! it as JavaScript does; a whole number is written as `45`, not `45.0`, as
//! `JSON.stringify` writes it. An optional field that is absent in TypeScript
//! is `None` here and left out of the JSON; a field that is `null` there is an
//! `Option` that is written as `null`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use super::zone::DayKey;

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
    Calendar,
    Ical,
    Mail,
    Capture,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    #[serde(serialize_with = "ser::num")]
    pub adjust_min: f64,
    pub notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    pub done: bool,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub done_at: Option<f64>,
    pub source: TaskSource,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub room: Room,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Active,
    OnHold,
    Someday,
    Archived,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GradeSource {
    You,
    Mail,
    Valence,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

/// A Google Calendar event: read-only, drawn grey behind blocks (3.5).
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

/// What the syncs remember between runs. 3.15 has no record for it; the artifact keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub processed_mail_ids: Vec<String>,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub last_sync_at: Option<f64>,
}
