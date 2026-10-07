//! The schema's own lists, read out for the Database tab (docs/ASK.md): each
//! kind Learn keeps, the fields a record of it may hold, and whether a
//! person's write may reach it. Nothing here writes; the tab's edits go
//! through `ops`, as every view's do, so the rules stay in `schema.rs`.

use crate::kind;
use crate::schema::{restricted, SPECS};

/// One kind of record as a table: its fields are its columns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindFields {
    pub kind: &'static str,
    /// The field that is the record's key in the store.
    pub key: &'static str,
    /// The fields `schema.rs` lists. Empty for a kind the schema doesn't
    /// spell out (Claude's and sync's records): its columns are whatever its
    /// records hold.
    pub fields: &'static [&'static str],
    /// Fields that may hold null.
    pub nullable: &'static [&'static str],
    /// Whether the kind has a Public switch.
    pub switch: bool,
    /// Why a person's put, patch or delete is refused for the whole kind,
    /// or None when it may be written.
    pub locked: Option<&'static str>,
}

/// Kinds that are records but come from somewhere other than the person's
/// own writes: listed with no fields, so the tab reads them from the data.
const UNSPECIFIED: &[(&str, &str)] = &[
    (kind::MAIL, "id"),
    (kind::CALENDAR, "id"),
    (kind::EVENT, "id"),
    (kind::SHARE, "id"),
];

/// Every kind of record Learn keeps, in the schema's order, then the kinds
/// the schema doesn't spell out. The timer's state, the settings and the
/// text of your mail are not records and are not here.
pub fn kinds() -> Vec<KindFields> {
    let mut out: Vec<KindFields> = SPECS
        .iter()
        .map(|s| KindFields {
            kind: s.name,
            key: s.key,
            fields: s.fields,
            nullable: s.nullable,
            switch: s.switch,
            locked: restricted(s.name),
        })
        .collect();
    for (k, key) in UNSPECIFIED {
        if out.iter().any(|have| have.kind == *k) {
            continue;
        }
        out.push(KindFields {
            kind: k,
            key,
            fields: &[],
            nullable: &[],
            switch: false,
            locked: Some(
                restricted(k)
                    .unwrap_or("A calendar's events come from its feed, so they change there."),
            ),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_the_schema_writes_is_a_table() {
        let all = kinds();
        for spec in SPECS {
            let found = all.iter().find(|k| k.kind == spec.name).expect(spec.name);
            assert_eq!(found.fields, spec.fields);
            assert!(found.fields.contains(&found.key), "{}", spec.name);
        }
        for name in [
            "task",
            "course",
            "grade",
            "habit",
            "focusSession",
            "note",
            "project",
            "milestone",
            "space",
        ] {
            assert!(
                all.iter().any(|k| k.kind == name && k.locked.is_none()),
                "{name}"
            );
        }
    }

    #[test]
    fn mail_calendars_and_shares_are_tables_nobody_edits() {
        let all = kinds();
        for name in [kind::MAIL, kind::CALENDAR, kind::EVENT, kind::SHARE] {
            let found = all.iter().find(|k| k.kind == name).expect(name);
            assert!(found.locked.is_some(), "{name}");
        }
        for not in [kind::STATE, kind::SETTING, kind::MAIL_TEXT] {
            assert!(all.iter().all(|k| k.kind != not), "{not}");
        }
    }
}
