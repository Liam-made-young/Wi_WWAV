# Learn in the code

How Learn (`docs/SPEC.md` chapter 3) and its MCP server (3.13, 8.8) are built:
where each part lives, what is stored, the core's `heat.*` commands, and the
eighteen tools. The spec says what Learn does; this file says how the parts talk.
Where they disagree, the spec wins and this file is fixed.

## Where things live

| Part | Does | Never |
|---|---|---|
| `crates/wi-heat` | The rules and the maths, as plain functions over plain records: `model/` (heat, the estimate chain, grades, Plan my day, focus, habits, recurrence, the weekly review's facts, moving in), the iCal and Brightspace parsers, the mail rules, sync's clock | I/O, the clock (time comes in as `now`), global state |
| `crates/wi-heat-store` | Learn's reads and writes on `library.sqlite`, through `wi-store`'s journal: one function per operation below, each one transaction. The core and `wi-mcp` both call it, so a rule exists once | the network, the Keychain, the engine |
| `crates/wi-core` | The `heat.*` commands (thin wrappers over `wi-heat-store`), the iCal fetch (network, addresses from the Keychain), Learn sync, and the watcher that notices other processes' commits | maths of its own |
| `crates/wi-mcp` | The stdio helper: MCP over JSON-RPC, the eighteen tools, the switches. Calls `wi-heat-store` with `actor = claude` | the network, the Keychain, any write a tool's table doesn't name |
| `app/ui/src/heat/` | The views. Reads `heat.snapshot`, writes through `heat.*`, refetches on the `heat` event | sorting by heat, clamping, planning, grade arithmetic: the snapshot carries every derived value |

The TS model in `app/ui/src/heat/model/` is the reference the Rust was ported
from (`crates/wi-heat/tests/vectors/`). The views don't call it for anything
the snapshot carries.

## What is stored

Learn's records are rows of `wi-store`'s `docs` table: `kind`, `key` (the
record's own id) and the record as JSON, exactly as 3.16 gives it, with
camelCase fields. `text` is what ⌘K searches (titles, notes, note text).

| kind | key | Notes |
|---|---|---|
| `space`, `task`, `taskOccurrence`, `timeBlock`, `focusSession`, `project`, `milestone`, `habit`, `term`, `course`, `grade`, `mailThread`, `calendar`, `capture`, `note`, `profileShare` | the record's `id` (ULID for new records; imported ids kept) | journaled |
| `dailyNote` | the date, `YYYY-MM-DD` | journaled |
| `heatState` | `state` | the timer, the current task and `planDrafts`, and under `focus` the timer machine's whole state (never shown to a view); **not** journaled (a draft is not a change, 8.8) |
| `heatSetting` | `claude.tools`, `school`, `dayEnds`, `feed.<calendarId>`, `feedDismissed`, `seeded`, `mailIds` | `claude.tools` is `{tool: bool}`, all true by default; `feed.*` holds a feed's missed-sync counts and `feedDismissed` the Brightspace items you deleted, so a sync doesn't make them again; `seeded` marks the three first spaces as made; **not** journaled |
| `calendar` | the record's `id` | a calendar's name, kind, sync times and `keychainRef`; added and removed in Settings → Learn, so **not** journaled: ⌘Z never takes a calendar away |
| `calendarEvent` | `<calendarId>/<UID>` | what other calendars' feeds hold; written by sync, outside the journal, replaced each sync |

Two fields beyond 3.16's lists: a task Claude added keeps `claudeReason`,
the sentence Get Info shows under "Claude, Oct 6 8:41 AM" (3.6), and a
pending grade from a notice keeps `postedAt`. A pending grade's `outOf` starts
at 100 until the person types the real one with the score.

Every record but `mailThread` carries `public: false` by default (3.16). A
record that belongs to another follows it: a task's blocks and occurrences, a
course's pending grades. iCal addresses are never in the library: `calendar`
holds a `keychainRef`, and the address is in the Keychain (Secret Service on
Linux).

**What syncs** (8.7). Only the app syncs, field by field, through
`/api/heat/changes`. `calendar`, `calendarEvent`, `heatState` and
`heatSetting` never go up. A private grade or course never goes up either: a
grade or course switched public goes up as a copy of the fields 3.15 lists
(a grade's `courseId`, `title`, `score`, `outOf`; a course's `code`, `name`),
with `public`, and when it is switched back, or deleted, the copy is pushed as
`deleted`. A private course whose grade is public goes up as its `code` and
`name` alone while that grade is public, because the grade names it. A grade
or course another device pushed is never written into this library. Everything
else, public or private, syncs.

## The journal

`txn` gains three columns: `actor` (`you` or `claude`, default `you`),
`tool` (the MCP tool's name, or null) and `reason` (Claude's one sentence, or
null). The Edit menu's label is `Undo ` + `label`. Claude's labels are fixed
(3.13): "Claude's task", "Claude's estimate", "Claude's pending grade",
"Claude's focus log", "Claude's mail note".

The person's labels:

| Operation | Label |
|---|---|
| a new record of any kind | `add <kind>`, the kind in words ("add task", "add habit", "add daily note") |
| a task's title | `rename task` |
| a task's difficulty or minutes | `estimate` |
| any other field | `edit <kind>` |
| a habit's day | `tick habit`, `untick habit` |
| delete | `delete <kind>` |
| done, and back | `mark done`, `mark not done` |
| a block moved, resized, added, removed | `move block`, `resize block`, `add block`, `remove block` |
| Plan my day accepted, one draft accepted | `plan my day`, `accept draft` |
| a focus round logged; Get Info's "Took" | `focus session`; `change time taken` |
| the Public switch | `make public`, `make private` |
| Show on a Now making line or a timeline, and hiding it | `show Now making`, `show timeline`, `hide Now making`, `hide timeline` |
| a capture; triage | `capture`; `triage capture` |
| a score typed into a grade | `enter score` |
| the weekly review's note | `weekly review` |
| moving in | `move in` |
| a calendar sync that changed anything | `calendar sync` |

`history.undoEntry {txnId}` undoes one entry out of order, for Settings →
Claude's list. It is refused, with "This changed again since. Undo the later
change first.", when a later done entry touched any of the same rows.

## The commands

All go through `core(cmd, args)` (`docs/COMMANDS.md`). Errors are
`{code, message}` with one plain sentence (`refused` unless a row says
otherwise). Every write returns `{..., undo: "Undo add task"}` (`undo` is null
when nothing changed) and emits the event `heat` `{kinds: [...]}` and the
`history` event the Edit menu listens to. Writes made by another process
(`wi-mcp`) emit the same events: the core checks `PRAGMA data_version` every
500 ms, reads the journal entries newer than the last it saw, skips its own,
and names the kinds each one changed. A change made outside the journal (the
timer, a setting, a calendar) emits `heat` with `heatState`, `heatSetting`,
`calendar` or `calendarEvent`.

### Reading

`heat.snapshot {date, from?, to?}` → everything the views draw for `date`,
and `school`: the School sheet for Settings → Learn (`name`, `host`,
`codePattern`, `termStart`, `termEnd`, and `icalSaved`, which says a
Brightspace link is in the Keychain and never the link).
`from` and `to` are the days shown plus today (the views widen the window to
take in today); `timeBlock`, `taskOccurrence` and `events` are bounded by them,
and `derived.occurrences` is drawn for them. Every other record comes whole.
Without them: the month around `date`, from a week before its 1st to a week
after its last day. What the core works out (`next`, heat, the lists) reads
every record, not only those in the window:

```jsonc
{
  "now": 1791384000000, "date": "2026-10-07", "zone": "America/New_York",
  "records": { "space": [...], "task": [...], "taskOccurrence": [...], "timeBlock": [...],
               "focusSession": [...], "project": [...], "milestone": [...], "habit": [...],
               "term": [...], "course": [...], "grade": [...], "mailThread": [...],
               "calendar": [...], "capture": [...], "dailyNote": [...], "note": [...],
               "profileShare": [...] },
  "heatState": { "currentTaskId": null,
                 "timer": { "phase": "idle", "round": 1, "endsAt": null, "running": false, "leftMs": 1500000,
                            "lengthMs": 1500000, "focusMin": 25, "taskId": null, "note": null },
                 "planDrafts": [] },
  "events": [ { "id": "...", "calendarId": "...", "title": "...", "start": 0, "end": 0, "allDay": false } ],
  "derived": {
    "tasks":   { "<taskId>": { "heat": { "v": 0.0, "level": "..." }, "actualMin": 0,
                               "estimate": { "min": 45, "by": "you|claude|default", "reason": null },
                               "next": "2026-10-08" } },
    "today":   { "header": "4 blocks · 3h 10m planned · 2 due today",
                 "planned": ["<blockId>"], "dueToday": ["<taskId>"],
                 "recurringToday": [{ "kind": "task|habit", "id": "..." }], "hotUnplanned": ["<taskId>"] },
    "hotTasks": ["<taskId>"],
    "lists":   { "inbox": [], "allOpen": [], "hot": [], "dueThisWeek": [], "scheduled": [], "someday": [], "done": [] },
    "averages": [ { "space": "<spaceId>", "type": "Homework", "minutes": 75, "count": 4, "line": "Homework 1h 15m (4)" } ],
    "courses": { "<courseId>": { "currentPct": 0.0, "decidedPct": 0.0, "letter": "B", "weights": "Weights add to 95%. The other 5% is unassigned.", "basedOn": "Based on 62.5% of the course so far" } },
    "habits":  { "<habitId>": { "today": true, "record": "Done 41 days since August 26" } },
    "occurrences": [ { "taskId": "<taskId>", "date": "2026-10-08", "done": false } ],
    "timer":   { "digits": "25:00", "line": "...", "note": null, "meter": 0, "paused": false, "strip": "..." },
    "status":  "Saved on this Mac · Synced 3:41 PM"
  }
}
```

Lists are ids in display order, so the views never sort (`done` reads newest
first). `heatState.timer` is the summary 3.16 gives (`phase`, `round`,
`endsAt`) and what the LCD needs to draw a paused round or a break that waits
for a press: `running`, `leftMs` (time left while paused or waiting),
`lengthMs`, `focusMin`, `taskId` (what the round is on, null for a habit's)
and `note` ("Focus done. 25m logged to Mix the second verse.", until the next
press). `derived.occurrences` holds each recurring, open series' dates inside
the window, with whether each is ticked, for Calendar's pills; without it only
`next` shows. `derived.timer` is the LCD's words at `now`, for a view that
doesn't run the model. `heat.whatItWouldTake
{courseId, letter}` → `{text}` ("To finish with a B (83%), you need 78.4% on
the remaining 35%."). `heat.review.week {weekStart}` → step 2's facts and step
5's headings (3.14). `heat.publicView {}` → exactly what someone opening your
sun would see (3.15), from the local records: the same shape the server's
`/api/heat/public/:userId` returns.

### Writing

| cmd | args | result |
|---|---|---|
| `heat.put` | `{kind, record}` | `{record, undo}`. New when `record.id` is absent (a ULID is made). Checked against the kind's fields and rules: a 7th habit is refused ("Habit limit reached"), minutes are clamped 5–600, unknown fields are refused ("A task has no field called 'colour'."). `mailThread`, `calendar` and `profileShare` are refused: Mail lists only what Claude recorded, calendars are added in Settings → Learn, and Show and Hide make the shares |
| `heat.patch` | `{kind, id, set}` | `{record, undo}`. Fields not in `set` are untouched. `public` is refused here: it has its own command |
| `heat.delete` | `{kind, id}` | `{undo}`. Takes the record's dependents with it, in the same entry; a space that still holds tasks, projects or milestones, and a term that still holds courses, are refused with a sentence. Deleting the current task, or one in a draft, clears it there |
| `heat.done` | `{taskId, done, date?}` | `{task, took?, undo}`. A recurring task gets or loses a `taskOccurrence` for `date` (the views always send it); a series never flips to done. With logged time, `took` is "Done. Took 1h 15m across 3 focus sessions." Marking the current task done makes nothing current, and clears a Now making line that shows it |
| `heat.estimate` | `{taskId, difficulty?, estMin?}` | `{task, clamped, undo}`. The same store function as `update_task`, with `actor = you` |
| `heat.tookTime` | `{taskId, minutes}` | `{task, undo}`. Sets `adjustMin` so the total reads `minutes` |
| `heat.plan.make` | `{date, dayEnds?}` | `{drafts, unplanned, minutesLeft}`. Writes `heatState.planDrafts` only. `drafts` are as `planDrafts` keeps them (`{taskId, date, start, minutes, leftMin, reason, leftLine}`); `unplanned` is the ids of open tasks that got no draft and have no block that day, in heat order (a parent is planned through its subtasks, so it isn't listed); `minutesLeft` is the 15-minute marks between now (a day ahead: 7 AM) and the day's end that no block, timed event or draft covers. `dayEnds` (minutes after midnight) is remembered |
| `heat.plan.accept` | `{date, taskIds?}` | `{blocks, undo}`. All drafts of `date` ("plan my day"), or the ones named ("accept draft"); the rest wait |
| `heat.plan.clear` | `{}` | `{}` |
| `heat.block.put` | `{id?, taskId?, habitId?, date, start, minutes}` | `{block, undo}`. 15-minute snap, kept inside 7 AM to midnight; other fields of a block (`origin`) are left as they are; label from what changed |
| `heat.current.set` | `{taskId}` (or null) | `{}`. Not journaled. Any task there is; else "No task has that id." Changing it while a round runs closes that task's part of the round, logs it, and runs on for the new one |
| `heat.focus.start` / `.pause` / `.resume` / `.interrupt` / `.stop` / `.finish` | `{taskId?, length?}` on start | `{heatState, logged?, undo}`. `start` takes the task named, else the current one ("Pick a task and press C first." with none) and makes it current; it starts a waiting break too, and never pauses a round that runs. `length` is 25, 50 or a whole 10–90. `stop` logs the minutes so far, `finish` is the window saying time is up, and each logs a `focusSession` (journaled, "focus session") only if it comes to a minute; `interrupt` counts one and pauses. **`finish` is idempotent:** the round ends at its own `endsAt`, whenever the call comes, logs once, and leaves the break waiting; asked early, late or twice, it changes nothing more, writes nothing and tells no one. The views ask every 500 ms, up to 8 times, once `endsAt` has passed. Nothing starts on its own (3.5) |
| `heat.capture.add` | `{text, link?}` | `{capture, inbox, undo}` (`inbox` is the count line "4 in inbox · captured ✓"). Empty text: "Type something to capture." |
| `heat.capture.triage` | `{id, to: "task"\|"note"\|"project"\|"upload", record?}` | `{result: {type, id, kind?, record?}, undo}`. With no `record` the core builds it from the capture's text: a task titled by the first line with the rest as notes, in the first space; a note holding the text; a project titled by the first line. `upload` only marks the capture dealt with (`id` null). A capture triaged already is refused |
| `heat.score` | `{gradeId, score, outOf?}` | `{grade, undo}` (a pending grade gets its score) |
| `heat.public.set` | `{kind, id, public}` | `{record, undo, sentence?}`. A grade's answer carries the sentence from 3.15 for the switch to show. Mail is never public ("Mail is never public."); kinds with no switch are refused |
| `heat.share.now` | `{taskId, text}` | `{share, undo}`. `clearsAt` is 7 days on; done clears it |
| `heat.share.timeline` | `{projectId, targetId}` | `{share, undo}` |
| `heat.share.hide` | `{id}` | `{undo}` |
| `heat.review.complete` | `{weekStart, note}` | `{note, undo}` |
| `heat.import` | `{json}` | `{counts, undo}`. Moving in: every id kept (3.16) |
| `heat.calendars.add` | `{name, kind, url}` | `{calendar}`. The address goes to the Keychain (`webcal://` is read as `https://`; error `bad_address` for one that isn't an address) and is never logged, stored, exported or put in an error. The new calendar is read a few seconds later |
| `heat.calendars.remove` | `{id}` | `{}`. Its events go; the tasks it made stay |
| `heat.calendars.sync` | `{id?}` | `{line}` ("Synced 3:41 PM: 2 new tasks, 1 date change", or the failure sentence of 3.11 naming the calendar). Calendars are also read on open if the last read was over 15 minutes ago, then hourly; Brightspace items become tasks with the UID as the id, and one missing from two syncs in a row is tagged "No longer in Brightspace", never deleted; one sync is one entry, "calendar sync" |
| `heat.school.set` | `{name, host, icalUrl?, codePattern, termStart, termEnd}` | `{}`. `icalUrl` goes to the Keychain |
| `heat.claude.get` | `{}` | `{helper, desktop, code, tools: [{name, on}], recent: [{txnId, label, reason, at, undone}], note?}` (the lines of Settings → Claude). `helper` is `WI_WWAV_MCP`, else `Contents/Helpers/wi-mcp` inside the app bundle, else `wi-mcp` beside the app's binary; when that file isn't there `note` is "Build the helper first: cargo build -p wi-mcp". `recent` is Claude's latest 50 entries, newest first, an undone one marked `undone` |
| `heat.claude.setTool` | `{name, on}` | `{}`. Not journaled; the helper reads it on every list and call |
| `history.undoEntry` | `{txnId}` | `{label}`, or `cant_undo` ("This changed again since. Undo the later change first."). Also emits `heat` and `history` |

## The MCP server

`wi-mcp` speaks MCP over stdio: one JSON-RPC 2.0 message per line on stdin
and stdout, logs on stderr only. It answers `initialize` (with the client's
`protocolVersion` when it supports it, else its newest; capabilities
`{tools: {}}`, server name `wi-wwav`), `notifications/initialized`, `ping`,
`tools/list`, `tools/call`, `prompts/list` and `prompts/get`; anything else
is error -32601.

- **Library.** It finds the library the app uses from the app's settings, with
  `--library <dir>` as an override for tests. No library, or a newer layout
  than it knows: one sentence, nothing written: "No Wi_WWAV library yet. Open
  the app once."
- **Tools.** The eighteen in 3.13, with those arguments, results and labels:
  the first eight for mail and planning, then `get_schedule`, `draft_block`,
  `list_habits`, `list_projects`, `add_project`, `add_milestone`, `get_notes`,
  `add_note`, `list_inbox` and `add_capture`. The four that add a record go
  through `schema::finish`, as a person's `heat.put` does, and mark the record
  `source: "claude"` with `claudeReason`. `draft_block` writes
  `heatState.planDrafts` only, as `plan_day` does. Each
  `inputSchema` is a JSON Schema with `additionalProperties: false`, so no
  call can carry `done`, a score or `public`. Read tools carry
  `annotations.readOnlyHint: true`. Every write tool's description opens with
  the two lines of 3.13. A result is `content: [{type: "text", text: <the JSON>}]`
  plus `structuredContent` with the same object.
- **Prompts.** One, `school_mail {days?}` (1–60, default 7): the job of 3.12's
  mail rows written out for Claude, naming the school and its Brightspace
  host from Settings → Learn in a Gmail query. A prompt calls no tool and
  writes nothing; it is how the person starts the job in one step
  (`/mcp__wi-wwav__school_mail` in Claude Code).
- **Failures** set `isError: true` with one sentence: "No open task has that
  id." A tool switched off is missing from `tools/list` and answers "This tool
  is switched off in Wi_WWAV."
- **One call, one transaction** (8.8): switch, arguments, then one
  `wi-heat-store` call inside one SQLite transaction with a busy timeout, so a
  kill leaves the change and its entry, or neither.
- **Settings → Claude** shows, with the helper's real path:

```json
{ "mcpServers": { "wi-wwav": { "command": "/Applications/Wi_WWAV.app/Contents/Helpers/wi-mcp" } } }
```

```sh
claude mcp add --scope user wi-wwav -- /Applications/Wi_WWAV.app/Contents/Helpers/wi-mcp
```

## Tests

Fail criteria first, in `docs/PLAN.md` (S2.1–S2.11). The MCP suite is 8.12's:
every tool called over stdio; one journal entry per write with `actor`
`claude`, its label and `undo_label`; undo returns the library byte for byte;
reads and `plan_day` write no entry; a repeated `source_id` or `thread_id`
makes no second row; a switched-off tool is missing and refused; 200 kills at
random moments; 1,000 interleaved writes with the core writing too, none lost.
The views are checked in Playwright through the dev bridge against a real core.

The core's side of S2.2–S2.6 and S2.9–S2.11 is `crates/wi-core/tests/core/heat_*.rs`:
every check goes through the `heat.*` commands on a core standing at a fixed
time in New York (`Core::set_now`); sync and the public view run against
`tools/mock-server`; calendars are read from a feed server in the test; and the
watcher is checked against a real `wi-mcp` process writing to the same
library.
