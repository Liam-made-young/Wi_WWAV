# Heat in the code

How Heat (`docs/SPEC.md` chapter 3) and its MCP server (3.13, 8.8) are built:
where each part lives, what is stored, the core's `heat.*` commands, and the
eight tools. The spec says what Heat does; this file says how the parts talk.
Where they disagree, the spec wins and this file is fixed.

## Where things live

| Part | Does | Never |
|---|---|---|
| `crates/wi-heat` | The rules and the maths, as plain functions over plain records: `model/` (heat, the estimate chain, grades, Plan my day, focus, habits, recurrence, the weekly review's facts, moving in), the iCal and Brightspace parsers, the mail rules, sync's clock | I/O, the clock (time comes in as `now`), global state |
| `crates/wi-heat-store` | Heat's reads and writes on `library.sqlite`, through `wi-store`'s journal: one function per operation below, each one transaction. The core and `wi-mcp` both call it, so a rule exists once | the network, the Keychain, the engine |
| `crates/wi-core` | The `heat.*` commands (thin wrappers over `wi-heat-store`), the iCal fetch (network, addresses from the Keychain), Heat sync, and the watcher that notices other processes' commits | maths of its own |
| `crates/wi-mcp` | The stdio helper: MCP over JSON-RPC, the eight tools, the switches. Calls `wi-heat-store` with `actor = claude` | the network, the Keychain, any write a tool's table doesn't name |
| `app/ui/src/heat/` | The views. Reads `heat.snapshot`, writes through `heat.*`, refetches on the `heat` event | sorting by heat, clamping, planning, grade arithmetic: the snapshot carries every derived value |

The TS model in `app/ui/src/heat/model/` is the reference the Rust was ported
from (`crates/wi-heat/tests/vectors/`). The views don't call it for anything
the snapshot carries.

## What is stored

Heat's records are rows of `wi-store`'s `docs` table: `kind`, `key` (the
record's own id) and the record as JSON, exactly as 3.16 gives it, with
camelCase fields. `text` is what ⌘K searches (titles, notes, note text).

| kind | key | Notes |
|---|---|---|
| `space`, `task`, `taskOccurrence`, `timeBlock`, `focusSession`, `project`, `milestone`, `habit`, `term`, `course`, `grade`, `mailThread`, `calendar`, `capture`, `note`, `profileShare` | the record's `id` (ULID for new records; imported ids kept) | journaled |
| `dailyNote` | the date, `YYYY-MM-DD` | journaled |
| `heatState` | `state` | the timer, the current task and `planDrafts`; **not** journaled (a draft is not a change, 8.8) |
| `heatSetting` | `claude.tools`, `school`, `dayEnds` | `claude.tools` is `{tool: bool}`, all true by default |
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

## The journal

`txn` gains three columns: `actor` (`you` or `claude`, default `you`),
`tool` (the MCP tool's name, or null) and `reason` (Claude's one sentence, or
null). The Edit menu's label is `Undo ` + `label`. Claude's labels are fixed
(3.13): "Claude's task", "Claude's estimate", "Claude's pending grade",
"Claude's focus log", "Claude's mail note".

The person's labels:

| Operation | Label |
|---|---|
| a new record of any kind | `add <kind>` ("add task", "add habit") |
| a task's title | `rename task` |
| a task's difficulty or minutes | `estimate` |
| any other field | `edit <kind>` |
| delete | `delete <kind>` |
| done, and back | `mark done`, `mark not done` |
| a block moved, resized, added, removed | `move block`, `resize block`, `add block`, `remove block` |
| Plan my day accepted, one draft accepted | `plan my day`, `accept draft` |
| a focus round logged; Get Info's "Took" | `focus session`; `change time taken` |
| a habit's day | `tick habit`, `untick habit` |
| the Public switch | `make public`, `make private` |
| Show on a Now making line or a timeline, and hiding it | `show Now making`, `show timeline`, `hide Now making`, `hide timeline` |
| a capture; triage | `capture`; `triage capture` |
| a score typed into a pending grade | `enter score` |
| moving in | `move in` |
| a calendar sync that changed anything | `calendar sync` |

`history.undoEntry {txnId}` undoes one entry out of order, for Settings →
Claude's list. It is refused, with "This changed again since. Undo the later
change first.", when a later done entry touched any of the same rows.

## The commands

All go through `core(cmd, args)` (`docs/COMMANDS.md`). Errors are
`{code, message}` with one plain sentence. Every write returns
`{..., undo: "Undo add task"}` and emits the event `heat` `{kinds: [...]}`.
Writes made by another process (`wi-mcp`) emit the same event: the core checks
`PRAGMA data_version` every 500 ms and when the window comes forward, reads the
journal rows newer than the last it saw, and names their kinds.

### Reading

`heat.snapshot {date, from?, to?}` → everything the views draw for `date`
(`from`/`to` bound events and blocks for Calendar; default: the month around
`date`, a week either side):

```jsonc
{
  "now": 1791384000000, "date": "2026-10-07", "zone": "America/New_York",
  "records": { "space": [...], "task": [...], "taskOccurrence": [...], "timeBlock": [...],
               "focusSession": [...], "project": [...], "milestone": [...], "habit": [...],
               "term": [...], "course": [...], "grade": [...], "mailThread": [...],
               "calendar": [...], "capture": [...], "dailyNote": [...], "note": [...],
               "profileShare": [...] },
  "heatState": { "currentTaskId": null, "timer": { "phase": "idle", "round": 1, "endsAt": null }, "planDrafts": [] },
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
    "averages": [ { "type": "Homework", "minutes": 75, "count": 4 } ],
    "courses": { "<courseId>": { "currentPct": 0.0, "decidedPct": 0.0, "letter": "B", "weights": "Weights add to 95%. The other 5% is unassigned." } },
    "habits":  { "<habitId>": { "today": true, "record": "Done 41 days since August 26" } },
    "status":  "Saved on this Mac · Synced 3:41 PM"
  }
}
```

Lists are ids in display order, so the views never sort. `heat.whatItWouldTake
{courseId, letter}` → `{text}` ("To finish with a B (83%), you need 78.4% on
the remaining 35%."). `heat.review.week {weekStart}` → step 2's facts and step
5's headings (3.14). `heat.publicView {}` → exactly what someone opening your
sun would see (3.15), from the local records: the same shape the server's
`/api/heat/public/:userId` returns.

### Writing

| cmd | args | result |
|---|---|---|
| `heat.put` | `{kind, record}` | `{record, undo}`. New when `record.id` is absent (a ULID is made). Checked against the kind's fields and rules: a 7th habit is refused ("Habit limit reached"), minutes are clamped 5–600, unknown fields are refused |
| `heat.patch` | `{kind, id, set}` | `{record, undo}`. Fields not in `set` are untouched. `public` is refused here: it has its own command |
| `heat.delete` | `{kind, id}` | `{undo}`. Takes the record's dependents with it, in the same entry |
| `heat.done` | `{taskId, done, date?}` | `{task, took?, undo}`. A recurring task gets or loses a `taskOccurrence` for `date`. With logged time, `took` is "Done. Took 1h 15m across 3 focus sessions." |
| `heat.estimate` | `{taskId, difficulty?, estMin?}` | `{task, clamped, undo}`. The same store function as `update_task`, with `actor = you` |
| `heat.tookTime` | `{taskId, minutes}` | `{task, undo}`. Sets `adjustMin` so the total reads `minutes` |
| `heat.plan.make` | `{date, dayEnds?}` | `{drafts, unplanned, minutesLeft}`. Writes `heatState.planDrafts` only |
| `heat.plan.accept` | `{date, taskIds?}` | `{blocks, undo}`. All drafts, or the ones named |
| `heat.plan.clear` | `{}` | `{}` |
| `heat.block.put` | `{id?, taskId?, habitId?, date, start, minutes}` | `{block, undo}`. 15-minute snap; label from what changed |
| `heat.current.set` | `{taskId}` (or null) | `{}`. Not journaled |
| `heat.focus.start` / `.pause` / `.resume` / `.interrupt` / `.stop` / `.finish` | `{taskId?, length?}` on start | `{heatState, logged?, undo?}`. `stop` and `finish` log a `focusSession` (journaled) and add its minutes; `interrupt` counts one and pauses. Nothing starts without a call (3.5) |
| `heat.capture.add` | `{text, link?}` | `{capture, inbox, undo}` (`inbox` is the count line "4 in inbox · captured ✓") |
| `heat.capture.triage` | `{id, to: "task"\|"note"\|"project"\|"upload", record?}` | `{result, undo}` |
| `heat.score` | `{gradeId, score}` | `{grade, undo}` (a pending grade gets its score) |
| `heat.public.set` | `{kind, id, public}` | `{record, undo}`. A grade's answer carries the sentence from 3.15 for the switch to show |
| `heat.share.now` | `{taskId, text}` | `{share, undo}`. `clearsAt` is 7 days on; done clears it |
| `heat.share.timeline` | `{projectId, targetId}` | `{share, undo}` |
| `heat.share.hide` | `{id}` | `{undo}` |
| `heat.review.complete` | `{weekStart, note}` | `{note, undo}` |
| `heat.import` | `{json}` | `{counts, undo}`. Moving in: every id kept (3.16) |
| `heat.calendars.add` | `{name, kind, url}` | `{calendar}`. The address goes to the Keychain |
| `heat.calendars.remove` | `{id}` | `{}` |
| `heat.calendars.sync` | `{id?}` | `{line}` ("Synced 3:41 PM: 2 new tasks, 1 date change", or the failure sentence of 3.11) |
| `heat.school.set` | `{name, host, icalUrl?, codePattern, termStart, termEnd}` | `{}` |
| `heat.claude.get` | `{}` | `{helper, desktop, code, tools: [{name, on}], recent: [{txnId, label, reason, at, undone}]}` (the lines of Settings → Claude) |
| `heat.claude.setTool` | `{name, on}` | `{}` |

## The MCP server

`wi-mcp` speaks MCP over stdio: one JSON-RPC 2.0 message per line on stdin
and stdout, logs on stderr only. It answers `initialize` (with the client's
`protocolVersion` when it supports it, else its newest; capabilities
`{tools: {}}`, server name `wi-wwav`), `notifications/initialized`, `ping`,
`tools/list` and `tools/call`; anything else is error -32601.

- **Library.** It finds the library the app uses from the app's settings, with
  `--library <dir>` as an override for tests. No library, or a newer layout
  than it knows: one sentence, nothing written: "No Wi_WWAV library yet. Open
  the app once."
- **Tools.** The eight in 3.13, with those arguments, results and labels. Each
  `inputSchema` is a JSON Schema with `additionalProperties: false`, so no
  call can carry `done`, a score or `public`. Read tools carry
  `annotations.readOnlyHint: true`. Every write tool's description opens with
  the two lines of 3.13. A result is `content: [{type: "text", text: <the JSON>}]`
  plus `structuredContent` with the same object.
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
