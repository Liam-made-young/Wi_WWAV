# Commitments in the code

Learn schedules tasks. A commitment is what it schedules around: a class, a
work shift, a commute. This file says what is stored, how a schedule gets in,
what a plan reads, and the commands. `docs/HEAT.md` says how Learn's parts
talk; this follows it.

## Where things live

| Part | Does |
|---|---|
| `crates/wi-heat/src/commitments.rs` | The rules, with no I/O: the days a commitment really happens on, a day's busy and free time, what is next, a schedule checked out of Claude's JSON or an `.ics` file, mail that cancels a class |
| `crates/wi-heat-store/src/commit.rs` | The records and the writes: one commitment, an exception, a schedule waiting to be applied, an exception waiting for a tap, a subscribed calendar, and `commitments` in the snapshot |
| `crates/wi-core/src/commit_cmd.rs` | The commands, the worker that asks Claude to read pasted text or a photo, the calendar fetch, "time to leave", and the mail check |
| `app/ui/src/heat/commitments/` | The sheets, the layer Calendar and Today draw, the Schedule list |

## What is stored

| kind | Notes |
|---|---|
| `commitment` | journaled; never synced |
| `termBreak` | `{id, title, from, to, source}`: days classes don't meet. Journaled; never synced |
| `commitmentDraft` | a schedule read and waiting: `reading`, `ready` or `failed`. Outside the journal |
| `pendingException` | what a mail asked for, keyed `mail-<thread id>`: `pending`, `confirmed` or `dismissed`. Outside the journal |
| `commitmentFeed` | a subscribed calendar: `{id, name, keychainRef, lastSyncedAt}`. Its address is in the Keychain |
| `heatSetting` `sleep` | `{from, to}`, minutes after midnight: to bed, and up. 11 PM and 7 AM until set |

```jsonc
// commitment
{ "id": "…", "title": "JPN 101", "kind": "class",        // class | work | commute | other
  "location": "Swan Hall 201",
  "start": 600, "end": 650,                               // minutes after local midnight
  "rrule": "FREQ=WEEKLY;BYDAY=MO,WE,FR",                  // absent: once, on `from`
  "from": "2026-09-09", "until": "2026-12-11",            // until absent: a class ends with the term
  "exceptions": [ { "date": "2026-10-08", "kind": "skip", "note": "…", "source": "mail" },
                  { "date": "2026-10-09", "kind": "move", "toDate": "2026-10-08", "start": 840, "end": 890 } ],
  "bufferBefore": 25, "bufferAfter": 10,                  // travel, 0 to 240
  "hardness": "fixed",                                    // fixed | flexible
  "courseId": "…", "spaceId": "…",
  "source": "you",                                        // you | paste | photo | ics | claude
  "sourceId": "uid@calendar", "weekOf": "2026-10-05", "feedId": "…" }
```

**Rules.**

- The repeat rule is the subset tasks use (`FREQ`, `INTERVAL`, `BYDAY`,
  `BYMONTHDAY`, `COUNT`, `UNTIL`). A calendar's `WKST` is dropped; a rule
  with anything else is refused, never guessed at.
- A commitment happens on the days its rule gives from `from` to `until`. A
  **class** also skips every day inside a `termBreak`, and with no `until`
  ends on the term's last day (Settings → Learn → School, `termEnd`).
- An exception names the day it would have happened. `skip` takes the day
  away; `move` puts it at `start`/`end` on `toDate` (the same day when
  absent), keeping its length when only `start` is given.
- A class names its course. One made with a course code Learn doesn't hold
  makes a stub course in the same entry, as a sync does. With no space named
  it sits in the space grouped by course, and takes its colour from there.
- It never runs past midnight: one that does is two.

## Getting commitments in

Every way but the sheet ends in a **draft**: read, checked, shown, and
written only when it is accepted. Accepting is one journal entry, so one ⌘Z
takes all of it back.

| Way | Command | How it is read |
|---|---|---|
| The sheet | `heat.commitment.create` | by hand |
| Pasted text | `heat.commitment.importText {text, mode}` | one run of Claude (`haiku`, no tools, a JSON schema) |
| A photo or screenshot | `heat.commitment.importImage {path, mode}` | one run of Claude (`sonnet`) that may read that one file and nothing else |
| An `.ics` file or address | `heat.commitment.importIcs {path \| url \| text}` | in Rust; `subscribe: true` keeps the address in the Keychain and reads it again every hour |

`mode` is `schedule` (what repeats every week), `week` (this week's shifts,
each on its own day) or `breaks` (an academic calendar: whole days off).

- **A week's shifts** carry `weekOf`, the week's Monday. Accepting a `week`
  draft deletes the work commitments of that `weekOf` and adds the new ones,
  in one entry. No other week is touched.
- A schedule applied twice adds nothing: an item with the same title, times
  and rule as a commitment already there is shown as "already there".
- A line Claude gave that doesn't check (no time, an end before its start)
  is counted in `unread` and left out.
- A subscribed calendar read again is one entry, "schedule sync", and only
  when something changed. What you set on one of its commitments (travel
  time, its space, an exception) is kept.

## Mail that cancels a class

After mail is read, `commit::pending_from_mail` looks at each thread from the
last 14 days that nothing was made for: its subject, Claude's reason and its
newest message. Written rules, no model: a word that cancels ("canceled", "no
class", "will not meet") or moves ("moved to", "rescheduled", "postponed"), a
commitment it names (the thread's course, else a title or code in the text),
and a day it meets on ("Thursday", "tomorrow", "Oct 9", or with none named
the next time it meets). "Not canceled" finds nothing.

What it finds is a `pendingException`, and a line: "EGR 101 is canceled
Thursday, Oct 8. Skip it?" It is offered through `heat.interrupt.raise` when
the Focus layout is there, and as a quiet notice otherwise. One tap is
`heat.commitment.exception.confirm {id}`, which writes the exception as one
entry ("skip class"). Nothing is ever applied without the tap.

## Planning

`World::busy(clock, date)` (`derive.rs`) is a day's busy time for any plan:
other calendars' timed events, every commitment from the start of its travel
to the end of it, and sleep. `derive::plan` (Plan my day, the MCP `plan_day`)
and the free-minutes count read it, and so does the UI's P (next free gap),
through `snapshot.commitments.days`. Nothing is planned into a class, a
shift, a buffer or sleep.

**Free time** for a day is the day (from now, today; from midnight on a day
ahead) less that busy time. `planned` is the minutes of blocks in the same
stretch; `over` is what is planned beyond the free time. The line is
"3h 20m free today. Planned 4h. Move 40m?", and says only what is true: with
nothing over, it ends at "Planned 4h."

A block that overlaps a commitment or its travel is listed in
`commitments.conflicts` and drawn flagged. Dragging a block there is the
person's own choice, so it is flagged, not refused.

## What the snapshot adds

`records.commitment` and `records.termBreak`, whole, and:

```jsonc
"commitments": {
  "days": { "2026-10-07": [ { "commitmentId": "…", "date": "2026-10-07", "start": 600, "end": 650,
      "title": "JPN 101", "kind": "class", "location": "Swan Hall 201",
      "bufferBefore": 25, "bufferAfter": 10, "hardness": "fixed",
      "courseId": "…", "spaceId": "…", "movedFrom": "2026-10-09",        // when an exception moved it
      "hue": 210, "label": "JPN 101 · Beginning Japanese I" } ] },       // hue null: no space
  "list": [ { "id": "…", "when": "MWF 10:00 AM to 10:50 AM", "range": "Sep 9 to Dec 11",
              "days": ["MO","WE","FR"], "hue": 210, "label": "…" } ],
  "next": { "commitmentId": "…", "title": "JPN 101", "date": "2026-10-07", "start": 600, "leaveAt": 575,
            "line": "NEXT JPN 101 10:00 · LEAVE 9:35" },                 // null when nothing is left today
  "free": { "date": "2026-10-07", "freeMin": 200, "plannedMin": 240, "overMin": 40,
            "spans": [[1180, 1380]], "line": "3h 20m free today. Planned 4h. Move 40m?" },
  "conflicts": [ { "blockId": "…", "commitmentId": "…", "date": "2026-10-07", "bufferOnly": false,
                   "line": "Overlaps JPN 101 (10:00 AM to 10:50 AM)." } ],
  "sleep": { "from": 1380, "to": 420 },
  "term": { "start": "2026-09-09", "end": "2026-12-11" },
  "drafts": [ { "id": "…", "mode": "schedule", "source": "paste|photo|ics", "fileName": "…",
      "weekOf": "2026-10-05", "createdAt": 0, "state": "reading|ready|failed", "error": "one sentence",
      // when ready:
      "items": [ { "title": "JPN 101", "kind": "class", "start": 600, "end": 650, "location": "…",
                   "from": "2026-09-09", "until": null, "days": ["MO","WE","FR"],
                   "when": "MWF 10:00 AM to 10:50 AM", "course": "JPN 101", "isNew": true, "newCourse": false } ],
      "breaks": [ { "title": "Thanksgiving recess", "from": "2026-11-25", "to": "2026-11-29", "isNew": true } ],
      "replaces": 2, "unread": 0,
      "line": "2 shifts. Replaces 2 shifts in the week of Oct 5." } ],
  "pending": [ { "id": "mail-…", "commitmentId": "…", "title": "EGR 101", "kind": "skip",
                 "exception": { "date": "2026-10-08", "kind": "skip" },
                 "line": "EGR 101 is canceled Thursday, Oct 8. Skip it?", "act": "Skip it",
                 "mailThreadId": "…", "subject": "…", "createdAt": 0 } ],
  "feeds": [ { "id": "…", "name": "Work rota", "lastSyncedAt": 0 } ]
}
```

`days` covers the snapshot's window (and its `date`), with every exception,
break and term end already applied: a view draws what it is given.

## The commands

| cmd | args | result |
|---|---|---|
| `heat.commitment.create` | `{title, kind?, start, end, days? \| date? \| rrule?, interval?, from?, until?, location?, bufferBefore?, bufferAfter?, hardness?, courseId? \| course?, spaceId? \| space?}` | `{commitment, course, line, undo}`. `start` and `end` are minutes or a time ("10:00", "5pm"); `days` are weekday codes; `date` is one day; `course` is a code (a stub course is made if Learn has none, and returned). "add commitment" |
| `heat.commitment.update` | `{id, set}` (`set` as create's args) | `{commitment, line, undo}`. `id` may be a title when only one has it. "edit commitment" |
| `heat.commitment.addException` | `{id, date, kind: "skip"\|"move", toDate?, start?, end?, location?, note?}` | `{commitment, line, undo}`. Refused for a day it doesn't meet on. "skip class", "move shift" |
| `heat.commitment.removeException` | `{id, date}` | `{commitment, undo}`. "restore class" |
| `heat.delete` | `{kind: "commitment", id}` | as any record |
| `heat.put`, `heat.patch`, `heat.delete` | `{kind: "termBreak", …}` | a break by hand |
| `heat.commitment.importText` | `{text, mode?, weekOf?}`, or `{json, mode?, weekOf?}` when the caller read it itself | `{draft}`: `reading` (the `heat` event says when it turns), or `ready` at once with `json` |
| `heat.commitment.importImage` | `{path, mode?, weekOf?}` | `{draft}`, `reading` |
| `heat.commitment.importIcs` | `{path \| url \| text, mode?, subscribe?, name?}` | `{draft}`, `ready`; with `subscribe`, `{feed, changed, undo}`: the address is kept and its commitments are written at once, as "schedule sync" |
| `heat.commitment.draft.accept` | `{draftId, skip?}` (`skip`: places in `items` to leave out) | `{commitments, breaks, replaced, courses, undo}`. "import schedule", "this week's shifts", "import breaks" |
| `heat.commitment.draft.discard` | `{draftId}` | `{}` |
| `heat.commitment.exception.confirm` | `{id}` | `{commitment, line, undo}` |
| `heat.commitment.exception.dismiss` | `{id}` | `{}`. It isn't offered again |
| `heat.commitment.feed.sync` | `{id?}` | `{changed}` |
| `heat.commitment.feed.remove` | `{id}` | `{}`. What it made stays |
| `heat.planner.freeTime` | `{date?}` or `{from, to}` | `{days: [{date, freeMin, plannedMin, overMin, spans, line, commitments}]}`. Today when nothing is named |
| `heat.sleep.set` | `{from, to}` | `{}`. Not journaled: a setting |

## Time to leave

While the app is open the core looks every twenty seconds. When a
commitment's travel time has started and it hasn't, once for that day, it
says "Time to leave for JPN 101 (10:00 AM, Swan Hall 201)." through
`heat.interrupt.raise`, or as a quiet notice until the Focus layout is merged.
`Config::leave_notices` turns it on; a test's core never says it by itself.

## Tests

`crates/wi-heat/tests/commitments.rs` holds the rules: a class through a
term with its breaks, exceptions, free time to the minute, a schedule checked
line by line, a calendar file, and the mail rules with the mails that must
find nothing. `crates/wi-core/tests/core/commitments.rs` goes through the
commands: Plan my day never lands in a class, a shift or a buffer; a pasted
and a photographed schedule against a stand-in for the command line; a week's
shifts replacing that week only; one undo for an import; a mail that cancels
Thursday's class waiting for its tap.
