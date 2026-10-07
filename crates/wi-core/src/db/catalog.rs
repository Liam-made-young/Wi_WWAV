//! What the Database tab's tables are (docs/ASK.md): one per kind of record
//! Learn keeps, generated from the schema's field lists
//! (`wi_heat_store::tables`), with a type for each field, the fields a
//! person can't type over marked locked, and the values Learn works out
//! (heat, a course's percentage) added as locked columns. A kind the schema
//! gains later becomes a table with no change here: its columns are named
//! from its fields and typed from what they hold.

use wi_heat_store::tables::{kinds, KindFields};

/// The kinds the tab itself keeps in the journal: a person's own tables,
/// their rows, the columns they added (to any table) and their saved views.
pub(crate) mod kind {
    pub const TABLE: &str = "dbTable";
    pub const ROW: &str = "dbRow";
    pub const COLUMN: &str = "dbColumn";
    pub const VIEW: &str = "dbView";
    /// A table's last layout (widths, order, what is hidden). Outside the
    /// journal: resizing a column is not a change ⌘Z should reach.
    pub const LAYOUT: &str = "dbLayout";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColType {
    Text,
    Number,
    Bool,
    /// A day, `YYYY-MM-DD`.
    Date,
    /// An instant, kept as epoch milliseconds and shown in the person's zone.
    DateTime,
    /// A link to a row of another table.
    Relation,
    /// A list or an object: shown as text, never typed over.
    Json,
    Formula,
}

impl ColType {
    pub fn as_str(self) -> &'static str {
        match self {
            ColType::Text => "text",
            ColType::Number => "number",
            ColType::Bool => "bool",
            ColType::Date => "date",
            ColType::DateTime => "datetime",
            ColType::Relation => "relation",
            ColType::Json => "json",
            ColType::Formula => "formula",
        }
    }

    /// The types a person may give a column of their own.
    pub fn parse_user(s: &str) -> Option<ColType> {
        match s {
            "text" => Some(ColType::Text),
            "number" => Some(ColType::Number),
            "bool" | "checkbox" => Some(ColType::Bool),
            "date" => Some(ColType::Date),
            "formula" => Some(ColType::Formula),
            _ => None,
        }
    }
}

/// Where a column's values come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    /// A field of the record.
    Field,
    /// Worked out by Learn; the name says which value.
    Derived(&'static str),
    /// A column the person added: a `dbColumn` record.
    Added,
}

#[derive(Clone, Debug)]
pub(crate) struct Column {
    /// The field's name, a derived column's `~name`, or a `dbColumn` id.
    pub id: String,
    pub name: String,
    pub ty: ColType,
    /// Why it can't be typed over; None when it can.
    pub locked: Option<String>,
    /// The table a relation points into.
    pub relation: Option<String>,
    /// The values a text column offers.
    pub options: Vec<String>,
    pub formula: Option<String>,
    pub source: Source,
}

pub(crate) const COMPUTED: &str = "Learn works this out, so it can't be typed over.";
pub(crate) const KEPT: &str = "Learn keeps this itself.";
const LIST: &str = "This holds a list. Change it where Learn shows it.";

/// A table's name and place in the sidebar, for the kinds this build knows.
/// Any other kind follows, named from its own word.
const NAMES: &[(&str, &str)] = &[
    ("task", "Tasks"),
    ("course", "Courses"),
    ("grade", "Grades"),
    ("gradeCategory", "Grade categories"),
    ("habit", "Habits"),
    ("habitLog", "Habit log"),
    ("focusSession", "Focus sessions"),
    ("timeBlock", "Time blocks"),
    ("mailThread", "Mail"),
    ("project", "Projects"),
    ("milestone", "Milestones"),
    ("space", "Spaces"),
    ("note", "Notes"),
    ("dailyNote", "Daily notes"),
    ("capture", "Inbox"),
    ("taskOccurrence", "Completions"),
    ("term", "Terms"),
    ("calendar", "Calendars"),
    ("calendarEvent", "Calendar events"),
    ("profileShare", "Shares"),
];

/// Tables made from part of a record: a habit's days, a course's categories.
pub(crate) const DERIVED_TABLES: &[&str] = &["habitLog", "gradeCategory"];

/// "estMin" as "Est min".
pub(crate) fn words(field: &str) -> String {
    let mut out = String::new();
    for (i, c) in field.chars().enumerate() {
        if c.is_ascii_uppercase() {
            out.push(' ');
            out.push(c.to_ascii_lowercase());
        } else if i == 0 {
            out.extend(c.to_uppercase());
        } else if c == '_' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn table_name(kind: &str) -> String {
    match NAMES.iter().find(|(k, _)| *k == kind) {
        Some((_, name)) => name.to_string(),
        None => format!("{}s", words(kind)),
    }
}

/// Where a kind sorts in the sidebar.
pub(crate) fn table_rank(kind: &str) -> usize {
    NAMES
        .iter()
        .position(|(k, _)| *k == kind)
        .unwrap_or(NAMES.len())
}

fn field_name(kind: &str, field: &str) -> String {
    let named = match (kind, field) {
        (_, "id") => "ID",
        (_, "estMin") => "Estimate (min)",
        (_, "adjustMin") => "Adjust (min)",
        (_, "estBy") => "Estimated by",
        (_, "estReason") => "Estimate reason",
        (_, "focusMin") => "Minutes",
        (_, "outOf") => "Out of",
        (_, "rrule") => "Repeats",
        (_, "scheduledDate") => "Scheduled",
        (_, "doneAt") => "Done at",
        (_, "startedAt") => "Started",
        (_, "endedAt") => "Ended",
        (_, "targetDate") => "Target date",
        (_, "parentTaskId") => "Parent task",
        (_, "sourceId") => "Source ID",
        (_, "claudeReason") => "Claude's reason",
        (_, "gmailThreadId") => "Gmail thread",
        (_, "showCounter") => "Show counter",
        ("timeBlock", "start") => "Start (min)",
        ("course", "code") => "Code",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_string();
    }
    // "courseId" reads "Course": the cell shows the course, not its id.
    match field.strip_suffix("Id") {
        Some(base) if !base.is_empty() => words(base),
        _ => words(field),
    }
}

/// The table a field points into, by the field's name.
fn relation_of(kind: &str, field: &str) -> Option<&'static str> {
    match field {
        "spaceId" => Some("space"),
        "courseId" => Some("course"),
        "projectId" => Some("project"),
        "milestoneId" => Some("milestone"),
        "parentTaskId" | "taskId" => Some("task"),
        "habitId" => Some("habit"),
        "termId" => Some("term"),
        "calendarId" => Some("calendar"),
        "categoryId" if kind == "grade" => Some("gradeCategory"),
        _ => None,
    }
}

fn type_of(kind: &str, field: &str) -> Option<ColType> {
    if relation_of(kind, field).is_some() {
        return Some(ColType::Relation);
    }
    Some(match field {
        "due" | "doneAt" | "startedAt" | "endedAt" | "triagedAt" | "postedAt" | "clearsAt"
        | "createdAt" | "updatedAt" | "recordedAt" | "receivedAt" | "lastSyncedAt" | "syncedAt"
        | "savedAt" | "lastMessageAt" => ColType::DateTime,
        "start" | "end" if kind == "calendarEvent" => ColType::DateTime,
        "scheduledDate" | "date" | "targetDate" => ColType::Date,
        "done" | "public" | "dropped" | "pending" | "showCounter" | "allDay" | "unread"
        | "archived" | "needsReply" => ColType::Bool,
        "difficulty" | "estMin" | "adjustMin" | "minutes" | "start" | "focusMin"
        | "interruptions" | "hue" | "order" | "score" | "outOf" | "weight" => ColType::Number,
        "types" | "categories" | "scale" | "log" | "keywords" | "messages" | "labels" => {
            ColType::Json
        }
        "link" if kind != "grade" && kind != "capture" => ColType::Json,
        "id" | "title" | "name" | "notes" | "markdown" | "text" | "code" | "type" | "group"
        | "rrule" | "status" | "source" | "sourceId" | "claudeReason" | "estBy" | "estReason"
        | "origin" | "room" | "view" | "persona" | "groupKind" | "groupLabel" | "subject"
        | "from" | "account" | "priority" | "category" | "summary" | "resultType" | "resultId"
        | "tag" | "kind" | "link" => ColType::Text,
        _ => return None,
    })
}

/// Fields Learn fills in itself: who estimated, where a record came from,
/// when it was done. They are shown, and locked.
fn kept_by_learn(kind: &str, field: &str) -> bool {
    matches!(
        field,
        "source"
            | "sourceId"
            | "claudeReason"
            | "estBy"
            | "estReason"
            | "doneAt"
            | "triagedAt"
            | "resultType"
            | "resultId"
            | "postedAt"
            | "origin"
            | "tag"
    ) || (kind == "focusSession")
        || (kind == "taskOccurrence")
}

fn options_of(kind: &str, field: &str) -> Vec<String> {
    let list: &[&str] = match (kind, field) {
        ("project", "status") => &["active", "on_hold", "someday", "archived"],
        ("space", "groupKind") => &["course", "milestone", "free"],
        _ => &[],
    };
    list.iter().map(|s| s.to_string()).collect()
}

/// The columns Learn works out for a kind, after its own fields.
fn derived_columns(kind: &str) -> Vec<Column> {
    let col = |id: &'static str, name: &str, ty: ColType, relation: Option<&str>| Column {
        id: format!("~{id}"),
        name: name.to_string(),
        ty,
        locked: Some(COMPUTED.to_string()),
        relation: relation.map(String::from),
        options: Vec::new(),
        formula: None,
        source: Source::Derived(id),
    };
    match kind {
        "task" => vec![
            col("heat", "Heat", ColType::Number, None),
            col("level", "Heat level", ColType::Text, None),
            col("planned", "Planned (min)", ColType::Number, None),
            col("logged", "Logged (min)", ColType::Number, None),
            col("next", "Next", ColType::Date, None),
        ],
        "course" => vec![
            col("pct", "Current %", ColType::Number, None),
            col("letter", "Letter", ColType::Text, None),
        ],
        "grade" => vec![col("pct", "Percent", ColType::Number, None)],
        "habit" => vec![
            col("today", "Done today", ColType::Bool, None),
            col("days", "Days done", ColType::Number, None),
        ],
        "focusSession" => vec![
            col("date", "Date", ColType::Date, None),
            col("course", "Course", ColType::Relation, Some("course")),
        ],
        _ => Vec::new(),
    }
}

/// The columns of a kind the schema spells out, or of one it doesn't, from
/// the fields its records hold (`seen`, in the order first met).
pub(crate) fn columns_of(k: &KindFields, seen: &[String]) -> Vec<Column> {
    let fields: Vec<String> = if k.fields.is_empty() {
        let mut all = vec![k.key.to_string()];
        all.extend(seen.iter().filter(|f| f.as_str() != k.key).cloned());
        all
    } else {
        k.fields.iter().map(|f| f.to_string()).collect()
    };
    let mut out: Vec<Column> = fields
        .iter()
        .map(|field| {
            let ty = type_of(k.kind, field);
            let locked = if let Some(why) = k.locked {
                Some(why.to_string())
            } else if field == k.key {
                Some("A record's own id never changes.".to_string())
            } else if ty == Some(ColType::Json) {
                Some(LIST.to_string())
            } else if kept_by_learn(k.kind, field) {
                Some(KEPT.to_string())
            } else {
                None
            };
            Column {
                id: field.clone(),
                name: field_name(k.kind, field),
                // A field this build doesn't know is typed from its values when the table loads.
                ty: ty.unwrap_or(ColType::Text),
                locked,
                relation: relation_of(k.kind, field).map(String::from),
                options: options_of(k.kind, field),
                formula: None,
                source: Source::Field,
            }
        })
        .collect();
    out.extend(derived_columns(k.kind));
    // Two fields may read the same once named ("taskId" and "task"): the later one says which.
    for i in 0..out.len() {
        let clash = out[..i]
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(&out[i].name));
        if clash {
            out[i].name = format!("{} ({})", out[i].name, out[i].id.trim_start_matches('~'));
        }
    }
    out
}

/// Whether a field's type was a guess, to be settled from the data.
pub(crate) fn type_is_known(kind: &str, field: &str) -> bool {
    type_of(kind, field).is_some()
}

/// The columns of the tables made from part of a record.
pub(crate) fn derived_table_columns(table: &str) -> Vec<Column> {
    let col = |id: &str, name: &str, ty: ColType, relation: Option<&str>| Column {
        id: id.to_string(),
        name: name.to_string(),
        ty,
        locked: Some(match table {
            "habitLog" => "A habit's days are ticked in Habits.".to_string(),
            _ => "A course's categories are set in Grades.".to_string(),
        }),
        relation: relation.map(String::from),
        options: Vec::new(),
        formula: None,
        source: Source::Field,
    };
    match table {
        "habitLog" => vec![
            col("id", "ID", ColType::Text, None),
            col("habitId", "Habit", ColType::Relation, Some("habit")),
            col("date", "Date", ColType::Date, None),
            col("done", "Done", ColType::Bool, None),
        ],
        _ => vec![
            col("id", "ID", ColType::Text, None),
            col("courseId", "Course", ColType::Relation, Some("course")),
            col("name", "Name", ColType::Text, None),
            col("weight", "Weight", ColType::Number, None),
        ],
    }
}

/// Every table made from Learn's records, in the sidebar's order: the
/// schema's kinds and the two made from part of a record.
pub(crate) fn learn_tables() -> Vec<(String, Option<KindFields>)> {
    let mut out: Vec<(String, Option<KindFields>)> = kinds()
        .into_iter()
        .map(|k| (k.kind.to_string(), Some(k)))
        .collect();
    for d in DERIVED_TABLES {
        out.push((d.to_string(), None));
    }
    out.sort_by_key(|(id, _)| table_rank(id));
    out
}

/// The field a row of a kind is called by, in a link and in search.
pub(crate) fn label_field(kind: &str) -> &'static str {
    match kind {
        "course" => "code",
        "space" | "term" | "calendar" | "gradeCategory" => "name",
        "mailThread" => "subject",
        "capture" => "text",
        "dailyNote" => "date",
        _ => "title",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_field_of_the_schema_is_a_column() {
        for (id, k) in learn_tables() {
            let Some(k) = k else { continue };
            let cols = columns_of(&k, &[]);
            for field in k.fields {
                assert!(cols.iter().any(|c| c.id == *field), "{id}.{field}");
            }
            // No two columns of a table share a name: a formula names one.
            for (i, c) in cols.iter().enumerate() {
                assert!(
                    cols[..i]
                        .iter()
                        .all(|o| !o.name.eq_ignore_ascii_case(&c.name)),
                    "{id}: two columns called {}",
                    c.name
                );
                assert!(!c.name.is_empty(), "{id}.{}", c.id);
            }
        }
    }

    #[test]
    fn every_schema_field_has_a_type_this_build_knows() {
        // A field added to the schema still becomes a column; this says to give it a type here.
        for (id, k) in learn_tables() {
            let Some(k) = k else { continue };
            for field in k.fields {
                assert!(
                    type_is_known(k.kind, field),
                    "{id}.{field} has no type in catalog.rs"
                );
            }
        }
    }

    #[test]
    fn the_entities_the_tab_promises_are_all_tables() {
        let ids: Vec<String> = learn_tables().into_iter().map(|(id, _)| id).collect();
        for want in [
            "task",
            "course",
            "grade",
            "habit",
            "habitLog",
            "focusSession",
            "mailThread",
            "project",
            "milestone",
            "space",
            "note",
        ] {
            assert!(ids.iter().any(|i| i == want), "{want}");
        }
        assert_eq!(ids[0], "task");
    }

    #[test]
    fn what_learn_keeps_is_locked_and_what_a_person_types_is_not() {
        let all = learn_tables();
        let task = all
            .iter()
            .find(|(id, _)| id == "task")
            .unwrap()
            .1
            .clone()
            .unwrap();
        let cols = columns_of(&task, &[]);
        let locked = |id: &str| cols.iter().find(|c| c.id == id).unwrap().locked.is_some();
        for free in [
            "title",
            "due",
            "scheduledDate",
            "estMin",
            "notes",
            "courseId",
            "done",
            "difficulty",
        ] {
            assert!(!locked(free), "{free}");
        }
        for kept in [
            "id", "source", "doneAt", "estBy", "link", "~heat", "~logged",
        ] {
            assert!(locked(kept), "{kept}");
        }
        let mail = all
            .iter()
            .find(|(id, _)| id == "mailThread")
            .unwrap()
            .1
            .clone()
            .unwrap();
        assert!(columns_of(&mail, &["subject".into()])
            .iter()
            .all(|c| c.locked.is_some()));
    }

    #[test]
    fn names_read_as_words() {
        assert_eq!(words("estMin"), "Est min");
        assert_eq!(field_name("task", "courseId"), "Course");
        assert_eq!(field_name("task", "estMin"), "Estimate (min)");
        assert_eq!(table_name("focusSession"), "Focus sessions");
        assert_eq!(table_name("flashCard"), "Flash cards");
    }
}
