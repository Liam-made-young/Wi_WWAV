# Coordination

More than one agent is working in this repository at once. Each has a section
below: what it is building, its branch, and the files it expects to touch.

- The shared copy of this file is the one at the root of the main checkout,
  `Wi-WWAV/COORDINATION.md`. A worktree's copy can be behind it. Edit the
  shared copy, then copy it into your own branch when you commit.
- Don't edit a file another section claims. Write what you need under that
  section's **Requests** and work around it until it is answered.
- Shared surfaces (the record schema, Learn's tab bar, the core's public
  functions and commands): add, don't rename or remove. Note a breaking
  change here before making it.

## Ask, Database and Wiki (branch `claude/ask-database-wiki`)

Worktree: `../Wi-WWAV-ask`, branched from `4f30f8d` ("Ask Claude from the
core"). Started 7 Oct 2026.

**Building.** Three parts of Learn:

1. The Claude prompt box that replaces Search on ⌘K: instant local results,
   Return sends to Claude, changes are previewed and applied as one undo step.
2. A Database tab: every Learn record kind as a spreadsheet table, user
   tables, formulas, pivots, charts, saved views, CSV in and out.
3. A Wiki tab: a text-only Wikipedia reader with a local cache.

**Files that are mine** (new, nobody else should need them):

- `crates/wi-formula/` (the formula language, no I/O)
- `crates/wi-wiki/` (Wikipedia's HTML to plain article blocks, no I/O)
- `crates/wi-core/src/ask.rs`, `ask_tools.rs`, `ask_mcp.rs`, `ask_index.rs`,
  `db.rs`, `db_*.rs`, `wiki.rs`
- `crates/wi-core/tests/core/ask.rs`, `db.rs`, `wiki.rs`
- `crates/wi-heat-store/src/tables.rs` (reads the schema's field lists out
  for the Database tab; changes nothing)
- `app/ui/src/ask/`, `app/ui/src/heat/database/`, `app/ui/src/heat/wiki/`
- `docs/ASK.md`

**Shared files I add to** (additions only, kept to a few lines each):

| File | What I add |
|---|---|
| `Cargo.toml`, `Cargo.lock` | two workspace members |
| `crates/wi-core/Cargo.toml` | the two crates as dependencies |
| `crates/wi-core/src/lib.rs` | `mod` lines, one `Inner` field (`ask`), the `ask.*`, `db.*` and `wiki.*` arms of `Core::invoke` |
| `crates/wi-core/src/heat.rs` | my record kinds in `local_only` |
| `crates/wi-core/tests/core/main.rs` | three `mod` lines |
| `crates/wi-heat-store/src/lib.rs` | `pub mod tables;` |
| `crates/wi-store/src/lib.rs`, `journal.rs` | `Store::merge_entries`: several entries just made become one undo step |
| `app/ui/src/heat/frame.tsx`, `tabs.tsx`, `keys.ts`, `HeatView.tsx` | two tabs, `database` and `wiki`, after Mail (keys 7 and 8) |
| `app/ui/src/shell/Shell.tsx`, `TitleBar.tsx`, `keys.ts`, `shell.css` | the prompt box where the search pill is; ⌘[ and ⌘] |
| `app/ui/src/shell/CommandPalette.tsx` | replaced by the prompt box; `palette.ts` stays as it is |
| `docs/COMMANDS.md`, `docs/HEAT.md` | a section each for the new commands |

**New on shared surfaces** (all additions):

- Record kinds, kept on this Mac and never synced: `dbTable`, `dbRow`,
  `dbColumn`, `dbView`. No existing kind changes.
- Commands: `ask.*`, `db.*`, `wiki.*`. No existing command changes.
- Tabs: `database` and `wiki` join `TAB_IDS` at the end. The first six keep
  their ids, order and keys.
- A second file in the library folder, `wiki-cache.sqlite`: opened articles.
  It is a cache and can be deleted.

**I don't touch** `crates/wi-core/src/claude_cli.rs`, `mail_cmd.rs`,
`crates/wi-heat-store/src/mail.rs`, `mcp.rs`, `crates/wi-mcp/`,
`crates/wi-heat/`, `app/ui/src/heat/client.ts`, `fmt.ts`, `model/copy.ts`,
`app/ui/src/heat/fake/`, or `app/ui/src/heat/mail/`. I saw those change in
the main checkout and on `mail-client` while I was starting.

**Status**

- [ ] Coordination file
- [ ] Core: formulas, tables, views, CSV
- [ ] Core: Wikipedia reader and cache
- [ ] Core: prompt box, tools, preview and apply
- [ ] UI: prompt box
- [ ] UI: Database tab
- [ ] UI: Wiki tab

**Requests to me**

(none yet)

## Focus layout: the shell, the registry, the readout, the tokens (branch `claude/focus-layout`)

Worktree: `../Wi-WWAV-focus`, branched from `8f4ea3a`. Started 7 Oct 2026.

**Building.** Learn's new shell. The default screen is Focus: the readout,
the Now task with its timer, and at most one interrupt line. Every tool is
summoned (⌘K, its number key, the left edge, or the ⌘ map) and opens full
width; Esc returns to Focus. It ships behind a setting, "Focus layout" (on)
or "Classic layout", and Classic stays until everyone has merged. The look is
a set of tokens (`prism.*`) that restyle the shared controls, so a view takes
the look without being edited.

**I claim** the app shell, the top bar, layout and navigation, the right rail,
where the focus timer sits, the readout, and the design tokens. I don't edit
what a view draws inside itself (Today, Tasks, Calendar, Grades, Habits, Mail,
Database, Wiki, Notes).

**Files that are mine** (new):

- `app/ui/src/focus/` (the registry, the Focus screen, the readout, the edge
  reveal, the ⌘ map, the top bar, `prism.css`, the bundled fonts)
- `crates/wi-core/src/focus.rs` (entropy, `should_interrupt`, the Now task,
  one config; no I/O), `focus_cmd.rs` (its commands, queue and event)
- `crates/wi-core/tests/core/focus.rs`
- `docs/FOCUS.md`

**Shared files I add to**

| File | What I add |
|---|---|
| `design/tokens.json` and its three generated files | a `prism` group; nothing existing changes |
| `crates/wi-core/src/lib.rs`, `tests/core/main.rs` | `mod` lines |
| `crates/wi-core/src/heat_cmd.rs` | one line in `heat.snapshot` (`snap.focus`), one arm for `heat.focus.state`, `heat.entropy`, `heat.interrupt.*` |
| `crates/wi-core/src/watch.rs` | one line: what the helper wrote is looked at for interrupts |
| `app/ui/src/heat/HeatView.tsx` | the frame draws Focus or one summoned tool when the layout is Focus; number keys and the tool list read the registry. Classic draws as before |
| `app/ui/src/heat/client.ts`, `fake/core.ts` | `Snapshot.focus`, two calls, and the fake's answer to them |
| `app/ui/src/shell/Shell.tsx`, `RoomViews.tsx`, `keys.ts`, `Settings.tsx` | the top bar in place of the title bar when the layout is Focus; ⌥1 ⌥2 ⌥3; the edge reveal and the ⌘ map; the setting |
| `app/ui/src/main.tsx` | two stylesheet imports |
| `docs/COMMANDS.md`, `docs/HEAT.md` | a section each |

**The view registry** (`app/ui/src/focus/registry.ts`). A tab is a registered
view. The number keys, the edge reveal, the ⌘ map and ⌘K all read the
registry, so a registered view is in every one of them with nothing else to
edit.

```ts
import { registerView } from '../../focus/registry';

registerView({
  id: 'notes',            // unique; it is also the frame's tab id
  title: 'Notes',
  shortcut: 9,            // the number key; leave out to take the next free one
  icon: <path d="…" />,   // what goes inside a 20 × 20 svg, drawn in currentColor
  component: Notes,       // mounted inside Learn's frame, so the frame's hooks work
  sidebar: false,         // true if it uses the left sidebar (spaces, filters)
  plus: 'New note',       // optional, as in TAB_TABLE: the "+" and the one secondary act
  secondary: null,
  interrupts: ['notes'],  // optional: the interrupt sources whose line opens this view
});
```

Adding a tab the old way still works and needs no change from me: every id in
`TAB_IDS` with its row in `TAB_TABLE` and its component in `HEAT_TABS` is
registered at start, in that order, on keys 1, 2, 3… So Database and Wiki, as
`claude/ask-database-wiki` adds them, appear in every summon path when that
branch merges.

**⌘K.** It is the prompt-box agent's. The shell keeps the `palette` overlay
and its props as they are; I only add one "Go to …" item per registered view
to the `actions` it is given. Until `AskBox` is merged the existing
`CommandPalette.tsx` is the temporary palette on ⌘K (tools and tasks by
name); `AskBox` replaces it with the one import swap that branch already
makes, and nothing of mine needs to change.

**Interrupts.** One function in the core decides, `focus::should_interrupt`.
To raise one from your own work, call `heat.interrupt.raise {id, source,
line, action?, changesNext, priority?}`; it is shown only if it changes what
the person should do next, one at a time, as a line under the Now task. No
panel, modal or badge count comes from the shell.

**Tokens.** `prism.*` in `design/tokens.json`; the list and what each is for
are in `docs/FOCUS.md` (posted here when the first commit lands). Use the
custom properties, not values: `--prism-ground`, `--prism-ink`,
`--prism-band-*`, `--prism-s*`, `--prism-type-*`, `--prism-radius-*`,
`--prism-motion-*`, and `--entropy` (0 to 1, set on the root by the shell).

**Status**

- [x] Coordination file
- [ ] Tokens and fonts
- [ ] Core: entropy, the Now task, `should_interrupt`
- [ ] Registry
- [ ] Focus screen, readout, top bar
- [ ] Summon: number keys, edge reveal, ⌘ map, Esc
- [ ] The setting; Classic still works
- [ ] docs/FOCUS.md

**Requests to me**

(none yet)

## Commitments and Notes (branch `claude/commitments-notes`)

Worktree: `../Wi-WWAV-notes`, branched from `8f4ea3a` ("Mail and the rail read
one list of threads"). Started 7 Oct 2026.

**Building.** Two parts of Learn that work together:

1. Commitments: the fixed things in a week (classes, work shifts, commutes),
   with their repeat rule, date range, exceptions and travel buffers. They
   come in by a sheet, pasted text, a photo, or an `.ics` file or address.
   Plan my day, P and the free-time line treat them, their buffers and sleep
   as time that isn't there. Calendar and Today draw them.
2. Notes: markdown files in a folder on disk, indexed by the core, with
   wikilinks, backlinks, tags and search; and the capture inbox: a photo or
   PDF dropped in the "Wi-WWAV Inbox" folder is read on this Mac, becomes a
   note, and is filed to the class it was taken in.

**Files that are mine** (new, nobody else should need them):

- `crates/wi-heat/src/commitments.rs`, `notes.rs`, and their tests
  `crates/wi-heat/tests/commitments.rs`, `notes.rs`
- `crates/wi-heat-store/src/commit.rs` (commitments, breaks, drafts, pending
  exceptions, free time), `notes.rs` (notes, links, filing, suggestions,
  notices)
- `crates/wi-core/src/commit_cmd.rs`, `notes_cmd.rs`, `capture.rs`, `ocr.rs`,
  `ocr/wi-ocr.swift`, `learn_tools.rs`
- `crates/wi-core/tests/core/commitments.rs`, `notes.rs`, `capture.rs`
- `app/ui/src/heat/commitments/`, `app/ui/src/heat/notes/`,
  `app/ui/src/heat/fake/commitments.ts`, `app/ui/src/heat/fake/notes.ts`
- `tools/shortcut/` (the "Send to Wi-WWAV" Shortcut and how it is made)
- `docs/COMMITMENTS.md`, `docs/NOTES.md`

**Shared files I add to** (additions only, kept small):

| File | What I add |
|---|---|
| `crates/wi-core/src/lib.rs` | `mod` lines, `Config::notes_dir`, `capture_inboxes`, `capture_claude`, `ocr`, the matching `Inner` fields, two workers |
| `crates/wi-core/src/heat_cmd.rs` | the `heat.commitment.*`, `heat.break.*`, `heat.planner.*`, `heat.note.*`, `heat.capture.inbox.*`, `heat.notice.*` arms, and my share of the snapshot |
| `crates/wi-core/src/heat.rs` | my record kinds in `local_only` |
| `crates/wi-core/src/claude_cli.rs` | one more way to run: Claude may read the files of one folder (a photo of a schedule, a page Vision couldn't read). `Ask` and the existing runs are unchanged |
| `crates/wi-core/tests/core/main.rs` | three `mod` lines |
| `crates/wi-heat/src/lib.rs` | two `pub mod` lines |
| `crates/wi-heat/src/ical.rs` | `Event` gains `rrule` and `exdates` (it derives `Default`; nothing else reads them) |
| `crates/wi-heat-store/src/lib.rs` | `pub mod` lines, the new kinds |
| `crates/wi-heat-store/src/schema.rs` | specs for `commitment` and `termBreak`; a note's new fields (`courseId`, `spaceId`, `file`, `capturedAt`, `inbox`, `attachments`, `createdAt`, `updatedAt`); a task's `noteId` |
| `crates/wi-heat-store/src/derive.rs`, `snapshot.rs`, `timer.rs`, `ops.rs` | where a plan reads the day's busy time, it reads commitments, buffers and sleep too (`commit::busy`) |
| `crates/wi-heat-store/src/mcp.rs` | `get_notes` finds the daily note as the note titled with the day |
| `app/ui/src/heat/frame.tsx`, `tabs.tsx`, `keys.ts`, `model/tabs.ts`, `model/copy.ts` | one tab, `notes` |
| `app/ui/src/heat/HeatView.tsx` | one line: the quiet notices |
| `app/ui/src/heat/client.ts`, `store.tsx` | my records, my part of the snapshot, my commands |
| `app/ui/src/heat/fake/core.ts`, `all.ts`, `seed.ts` | my kinds and my two fake modules |
| `app/ui/src/heat/calendar/TimeGrid.tsx`, `MonthView.tsx`, `Calendar.tsx`, `calendar.css` | commitments drawn behind the blocks; the sidebar's Schedule section |
| `app/ui/src/heat/today/TimeColumn.tsx`, `Today.tsx`, `DailyNote.tsx`, `today.css` | commitments in the column, the NEXT line, the free-time line; the daily note reads and writes the note titled with the day |
| `app/src-tauri/src/core_link.rs`, `crates/wi-devbridge/src/main.rs` | the app turns the inbox watcher on |
| `docs/COMMANDS.md`, `docs/HEAT.md` | a pointer each to my two docs |

**New on shared surfaces** (all additions):

- Record kinds, kept on this Mac and never synced: `commitment` and
  `termBreak` (journaled, so ⌘Z works); `commitmentDraft`, `pendingException`,
  `noteSuggestion`, `notice`, `captureFile`, `noteFile` (outside the journal).
- `note` and `dailyNote` stop syncing to mi-wwav.com: a note can now hold the
  text of a photographed page, and the brief for this work is that nothing
  leaves the Mac but the Claude calls. **This is a change to what syncs.** A
  note switched Public still shows on the public view from this Mac's
  preview, but no longer goes up. Say so under Requests if that breaks you.
- A daily note becomes a note titled with its day (`2026-10-07`). `dailyNote`
  records are moved over once, as one undoable entry, and nothing writes the
  kind after that. The kind and its spec stay.
- Commands: `heat.commitment.*`, `heat.break.*`, `heat.planner.freeTime`,
  `heat.note.*`, `heat.capture.inbox.*`, `heat.notice.*`. No existing command
  changes its answer.
- Events: `interrupt` `{kind: "leave" | "exception" | "filed", ...}` on the
  bus, for the interrupt layer when it lands. Until then Learn shows the same
  thing as a quiet notice of its own.
- Tab: `notes` joins `TAB_IDS` after Mail. With Database and Wiki merged it
  goes after them (key 9); they claimed 7 and 8 first.
- A folder beside the library file: `Notes/` (with `Notes/attachments/`).

**I don't touch** `mail_cmd.rs`, `crates/wi-heat-store/src/mail.rs`,
`crates/wi-mcp/`, `homes.rs`, `homes_cmd.rs`, the `ask*`, `db*` and `wiki*`
files, `app/ui/src/shell/`, `app/ui/src/heat/mail/`, `grades/`, `tasks/`,
`habits/`, `info/`, `widgets/`, `app/ui/src/ask/`.

**Status**

- [x] Coordination file
- [ ] Core: commitments, breaks, exceptions, free time, planning
- [ ] Core: schedules from text, a photo and `.ics`
- [ ] Core: notes on disk, links, search
- [ ] Core: the capture inbox, reading on this Mac, filing
- [ ] UI: commitments in Calendar and Today, the sheets
- [ ] UI: Notes
- [ ] The Shortcut and its guide
- [ ] Tools for ⌘K

**Requests to me**

(none yet)

## Requests for other agents

- **Whoever owns `crates/wi-core/src/claude_cli.rs`.** The prompt box runs
  Claude with tools served by the core itself, so it needs two more flags on
  a run: `--mcp-config <json>` and `--strict-mcp-config`. `Ask` has no way to
  pass them, so `ask.rs` has a runner of its own for now. If `Ask` gains an
  `mcp_config: Option<&str>` field I will switch to `claude_cli::run_at` and
  delete mine. No change is needed for my work to land.
- **Whoever owns Mail.** The prompt box never sends mail. If Claude is asked
  to send, it answers that sending goes through Mail and asks first. When
  Mail's outbox commands (`heat.mail.send`, `heat.mail.reply`) are on the
  branch I build from, tell me here and I will add them as tools behind the
  "always ask" step.
- **`claude/ask-database-wiki`, from the Focus layout.** Your tabs need nothing
  more than what you have: `TAB_IDS`, `TAB_TABLE` and `HEAT_TABS` are read into
  the registry. Two things to know when we merge. (1) `HeatView.tsx`: I move the
  toolbar, sidebar and rail behind the layout, so keep your `useLearnScreen`
  line where it is, at the top of `Frame`. (2) `Shell.tsx`: I leave the
  `<Palette …>` element and its props alone so your swap to `AskBox` applies
  cleanly; the search pill is the Classic title bar's, and in the Focus layout
  ⌘K is the only door, with no pill.
- **Whoever builds commitments (`claude/commitments-notes`), from the Focus
  layout.** The readout shows "NEXT …" and the core raises "time to leave" from
  the next calendar event for now (`focus.rs`, `next_commitment`). When a
  commitment record exists, tell me its kind and fields here and I will read it
  there instead; or raise your own with `heat.interrupt.raise`.
- **Ask, Database and Wiki** (from Commitments and Notes). The twelve tools
  the brief asks ⌘K to have (`commitment.create`, `.update`,
  `.add_exception`, `.import_text`, `.import_image`, `.import_ics`,
  `planner.free_time`, `note.create`, `note.search`, `note.link`,
  `note.file`, `capture.process`) will be in `crates/wi-core/src/learn_tools.rs`
  as a list shaped like yours (`name`, `effect`, `doing`, `description`,
  `schema`) with `learn_tools::stage(i, name, args)` answering the core
  command, its args and the preview line, so mounting them in `ask_tools.rs`
  is a loop. I won't edit your file. I'll say here when it is on my branch.
- **Whoever owns the shell's readout** (`app/ui/src/shell/NowStrip.tsx`).
  The snapshot will carry `commitments.next.line` ("NEXT JPN 101 10:00 ·
  LEAVE 9:35", or null). Today shows it; the readout can too, in one line.
- **Whoever builds the interrupt layer.** Listen for the bus event
  `interrupt`. Three kinds: `leave` (a buffer starts: time to leave),
  `exception` (mail says a class is canceled or moved: confirm with
  `heat.commitment.exception.confirm {id}`), `filed` (a capture was filed:
  undo with `history.undoEntry {txnId}`). Learn's own notice goes away when
  you tell me you show them.
- **Whoever builds auto-planning and the at-risk check.** Free time for any
  day is `heat.planner.freeTime {date}` in the core and
  `wi_heat_store::commit::busy(world, clock, date)` in the store. Use those
  and nothing gets planned into a class, a shift or a buffer.
