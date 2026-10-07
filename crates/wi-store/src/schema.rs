//! The schema, as numbered migrations. `PRAGMA user_version` says how many
//! have run; opening runs the rest, each in its own transaction.
//!
//! Every journaled table has a TEXT `id` the journal names rows by. No foreign
//! key cascades: a cascade is a change SQLite makes behind the journal's back,
//! and "an unjournaled row is an unrestorable one" (docs/SPEC.md 9.6).

use rusqlite::{Connection, TransactionBehavior};

use crate::{refused, Result};

const MIGRATIONS: &[&str] = &[V1];

/// How many migrations this version knows.
pub const SCHEMA_VERSION: usize = MIGRATIONS.len();

const V1: &str = r#"
CREATE TABLE sequences (
  id      TEXT PRIMARY KEY,          -- ULID
  title   TEXT NOT NULL DEFAULT '',
  package TEXT NOT NULL              -- sessions/<id>.wwavsession: the edit list lives there (6.5)
);

CREATE TABLE clips (
  n             INTEGER PRIMARY KEY AUTOINCREMENT, -- the search index's key; never reused, so
                                                   -- undo puts a row back under its old number
  id            TEXT NOT NULL UNIQUE,              -- ULID, and the file's name
  kind          TEXT NOT NULL CHECK (kind IN ('wwav', 'swav', 'audio', 'video', 'image', 'text')),
  file          TEXT NOT NULL,       -- media/<id>.<ext>, purchases/<id>.<ext>, or an absolute path left in place
  sha256        TEXT NOT NULL,
  bytes         INTEGER NOT NULL,
  title         TEXT NOT NULL DEFAULT '',
  artist        TEXT NOT NULL DEFAULT '',
  bpm           REAL,
  key           TEXT,                -- "A minor"
  duration_ms   INTEGER NOT NULL DEFAULT 0,
  verdict       TEXT NOT NULL DEFAULT '', -- the reader's sentence, word for word
  text          TEXT,                -- a text clip's words, for search
  colour        TEXT CHECK (colour IN ('red', 'orange', 'yellow', 'green', 'blue', 'purple')),
  tag_count     INTEGER NOT NULL DEFAULT 0 CHECK (tag_count BETWEEN 0 AND 12),
  published_at  INTEGER,             -- ms; set by the drop
  remote_id     TEXT,                -- set when the server has it
  from_sequence TEXT REFERENCES sequences(id)
);
CREATE INDEX clips_queue ON clips(published_at) WHERE published_at IS NOT NULL AND remote_id IS NULL;
CREATE INDEX clips_from_sequence ON clips(from_sequence) WHERE from_sequence IS NOT NULL;

CREATE TABLE tags (
  id   TEXT PRIMARY KEY CHECK (id = kind || ':' || name), -- one name of one kind is one row
  name TEXT NOT NULL CHECK (name <> '' AND name = lower(name)),
  kind TEXT NOT NULL CHECK (kind IN ('user', 'card', 'system'))
);
CREATE INDEX tags_name ON tags(name);

CREATE TABLE clip_tags (
  id      TEXT PRIMARY KEY CHECK (id = clip_id || '/' || tag_id),
  clip_id TEXT NOT NULL REFERENCES clips(id),
  tag_id  TEXT NOT NULL REFERENCES tags(id)
);
CREATE INDEX clip_tags_clip ON clip_tags(clip_id);
CREATE INDEX clip_tags_tag ON clip_tags(tag_id);

-- One row holding the four slots, so every pin change touches the same value
-- and pins undo in the order they were made, whichever room made them.
CREATE TABLE pins (
  id    TEXT PRIMARY KEY CHECK (id = 'pins'),
  slots TEXT NOT NULL CHECK (json_array_length(slots) = 4) -- "clip:<id>", "folder:<id>" or null
);
INSERT INTO pins (id, slots) VALUES ('pins', '[null,null,null,null]');

CREATE TABLE smart_folders (
  id   TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  rule TEXT NOT NULL                 -- JSON: tags, colour, bpm_min, bpm_max, key, kind, all ANDed
);

-- The rooms' records, such as Heat's Space, Task and TimeBlock (3.15).
CREATE TABLE docs (
  n    INTEGER PRIMARY KEY AUTOINCREMENT,
  id   TEXT NOT NULL UNIQUE CHECK (id = kind || '/' || key),
  kind TEXT NOT NULL CHECK (kind <> '' AND instr(kind, '/') = 0),
  key  TEXT NOT NULL,                -- the record's own id, kept as it came
  json TEXT NOT NULL,
  text TEXT NOT NULL DEFAULT ''      -- what search reads
);
CREATE INDEX docs_kind ON docs(kind, key);

CREATE VIRTUAL TABLE clips_fts USING fts5(
  title, artist, text, content = 'clips', content_rowid = 'n',
  tokenize = 'unicode61 remove_diacritics 2', prefix = '1 2'
);
CREATE TRIGGER clips_fts_insert AFTER INSERT ON clips BEGIN
  INSERT INTO clips_fts (rowid, title, artist, text) VALUES (new.n, new.title, new.artist, new.text);
END;
CREATE TRIGGER clips_fts_delete AFTER DELETE ON clips BEGIN
  INSERT INTO clips_fts (clips_fts, rowid, title, artist, text) VALUES ('delete', old.n, old.title, old.artist, old.text);
END;
CREATE TRIGGER clips_fts_update AFTER UPDATE OF title, artist, text ON clips BEGIN
  INSERT INTO clips_fts (clips_fts, rowid, title, artist, text) VALUES ('delete', old.n, old.title, old.artist, old.text);
  INSERT INTO clips_fts (rowid, title, artist, text) VALUES (new.n, new.title, new.artist, new.text);
END;

CREATE VIRTUAL TABLE docs_fts USING fts5(
  text, content = 'docs', content_rowid = 'n',
  tokenize = 'unicode61 remove_diacritics 2', prefix = '1 2'
);
CREATE TRIGGER docs_fts_insert AFTER INSERT ON docs BEGIN
  INSERT INTO docs_fts (rowid, text) VALUES (new.n, new.text);
END;
CREATE TRIGGER docs_fts_delete AFTER DELETE ON docs BEGIN
  INSERT INTO docs_fts (docs_fts, rowid, text) VALUES ('delete', old.n, old.text);
END;
CREATE TRIGGER docs_fts_update AFTER UPDATE OF text ON docs BEGIN
  INSERT INTO docs_fts (docs_fts, rowid, text) VALUES ('delete', old.n, old.text);
  INSERT INTO docs_fts (rowid, text) VALUES (new.n, new.text);
END;

-- The undo journal, as docs/SPEC.md 9.6 gives it.
CREATE TABLE txn     (id    TEXT PRIMARY KEY,   -- ULID, so order is time
                      label TEXT NOT NULL,      -- "move clip" -> "Undo move clip"
                      room  TEXT NOT NULL,      -- heat | space | console | unquantized | library
                      state TEXT NOT NULL);     -- done | undone
CREATE TABLE txn_row (txn_id TEXT NOT NULL REFERENCES txn(id),
                      seq INTEGER NOT NULL, tbl TEXT NOT NULL, row_id TEXT NOT NULL,
                      before TEXT, after TEXT,  -- JSON; NULL means the row didn't exist
                      PRIMARY KEY (txn_id, seq));
CREATE INDEX txn_room ON txn(room, state, id);
CREATE INDEX txn_row_target ON txn_row(tbl, row_id);

-- Work that left the machine: never undone, but the Edit menu says so.
CREATE TABLE outward (
  id        TEXT PRIMARY KEY,          -- ULID, ordered with the journal's
  room      TEXT NOT NULL,
  what      TEXT NOT NULL,             -- "a purchase" -> "Can't undo a purchase."
  after_txn TEXT                       -- the room's newest done entry when it happened (NULL: none),
                                       -- so a redo made after it still undoes
);

-- Not journaled: what the server has, what was bought, what the scanner found.
CREATE TABLE upload_part (
  clip_id TEXT NOT NULL,               -- no REFERENCES: kept only while the clip is queued
  n       INTEGER NOT NULL,
  etag    TEXT NOT NULL,
  PRIMARY KEY (clip_id, n)
);

-- Files on their way into media/ (an import being copied, a render being
-- written) that no clip names yet: Clean up media… leaves them alone.
CREATE TABLE media_pending (
  file    TEXT PRIMARY KEY,            -- media/<id>.<ext>
  made_ms INTEGER NOT NULL             -- a reservation older than a day is a crash's leftover
);

CREATE TABLE purchases (
  id        TEXT PRIMARY KEY,          -- ULID
  clip_id   TEXT NOT NULL,             -- no REFERENCES: the receipt outlives a deleted clip
  remote_id TEXT NOT NULL,
  file_name TEXT NOT NULL,             -- as delivered
  bytes     INTEGER NOT NULL,
  sha256    TEXT NOT NULL,
  receipt   TEXT NOT NULL              -- the server's receipt, as JSON
);

CREATE TABLE plugin_scans (
  path        TEXT NOT NULL,
  uid         TEXT NOT NULL,           -- '' for a plugin that never said
  format      TEXT NOT NULL,
  name        TEXT NOT NULL,
  vendor      TEXT NOT NULL,
  version     TEXT NOT NULL,
  modified_ms INTEGER NOT NULL,        -- the file's date: a change means check it again
  status      TEXT NOT NULL CHECK (status IN ('ok', 'crashed', 'hung')),
  reason      TEXT NOT NULL,
  checked_ms  INTEGER NOT NULL,
  PRIMARY KEY (path, uid)
);
"#;

/// Runs the migrations this library hasn't had. Each runs in a write
/// transaction that reads `user_version` again first, so two handles opening
/// a new library at once (the app and its uploader) never run one twice.
pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    loop {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let have: usize = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if have > SCHEMA_VERSION {
            return refused("This library was made by a newer Wi_WWAV. Update the app to open it.");
        }
        let Some(sql) = MIGRATIONS.get(have) else {
            return Ok(()); // up to date; dropping `tx` ends it
        };
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", have + 1)?;
        tx.commit()?;
    }
}
