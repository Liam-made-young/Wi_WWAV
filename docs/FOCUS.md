# The Focus layout

Learn's shell since 7 Oct 2026. The screen shows only what needs you now, and
gets calmer as things come into order. Every tool is still there; it is
summoned, not shown. The look is a prism instrument: at rest the app is white
light, and work splits that light into colour only where there is energy.

It ships behind a setting, Settings → Appearance → Layout: **Focus layout**
(the default) or **Classic layout**. Classic is the toolbar, tabs, sidebar and
right column as they were, and stays until every branch in `COORDINATION.md`
has merged.

## Where things live

| | |
|---|---|
| `app/ui/src/focus/` | everything the layout draws, the registry, and the model the core ports |
| `app/ui/src/focus/model.ts` | the rules, in TypeScript: the reference, and what the fake core answers with |
| `crates/wi-core/src/focus.rs` | the same rules in the core, on the snapshot's JSON; no I/O |
| `crates/wi-core/src/focus_cmd.rs` | `snapshot.focus`, the commands, the `entropy` event, what is remembered |
| `design/tokens.json`, group `prism` | every colour, size and duration of the look |
| `app/ui/src/focus/prism.css` | the look, given to every view through the desk's tokens |
| `app/ui/src/focus/focus.css` | the layout's own parts |

## The three layers

**Focus**, the default. The readout; the Now task, large and centred, with its
course or space, when it is due, and its space's colour as one thin band of
light; the focus timer under it (Start, 25 / 50, Pulled away, and Stop while a
round is on); the first step, if the task has steps (its first open subtask,
else the first unticked `- [ ]` in its notes); Done; and at most one interrupt
line. One button is filled: Start focus while the timer is idle, Done once a
round is on.

**Summon.** A tool opens full width under its name and "Esc to focus". The
left sidebar shows only in the tools that use it.

| How | What |
|---|---|
| ⌘K | the prompt box (see "⌘K" below) |
| 1 to 9 | the view with that number, when no text field has the keyboard |
| the left 8 px | rest the cursor there for `prism.motion.edgeRest` and the tool list slides out |
| hold ⌘ | after `prism.motion.mapHold`, a faint map of every tool and key; gone on release |
| Esc | closes what is open (a popover, a sheet, drafts), then returns to Focus |
| the top edge, or ⌥1 ⌥2 ⌥3 | Learn, Space, Console (⌘1 ⌘2 ⌘3 still work) |

**Interrupt.** One quiet line under the Now task, with one action and a
Dismiss. Never a panel, a modal or a count. See "Interrupts".

### What the Now task is

1. The current task, if one is set and still open.
2. Else, if entropy is high (`highAt`) and there is a fix: the fix.
3. Else the top open task by heat.
4. Else the fix for what is left (mail, grades).
5. Else nothing: "All clear".

A fix answered with "Not now", or with its own button, is not offered again
that day.

Step 2 is a reading of two lines of the brief that pull apart: "the current
task; otherwise the top task by heat; otherwise the entropy fix" and "High:
Focus shows the fix, not the mess". As written first, a fix about unplanned
tasks could never show, because unplanned tasks are open tasks. So the fix
goes ahead of the top task only when entropy is high, and never ahead of a
task the person chose.

## The view registry

`app/ui/src/focus/registry.ts`. A tab is a registered view. The number keys,
the edge reveal, the ⌘ map and ⌘K's "Go to …" all read the registry, so a
view that registers is in every one of them with nothing else to edit. Classic's
tab bar reads it too.

```ts
import { registerView } from '../../focus/registry';

const remove = registerView({
  id: 'notes',            // unique; it is also the frame's tab id
  title: 'Notes',
  shortcut: 9,            // 1 to 9. Left out, or taken, it gets the next free one; past 9 it has no key
  icon: <path d="…" />,   // what goes inside a 20 × 20 svg, in currentColor. Left out: a plain square
  component: Notes,       // mounted inside Learn's frame, so useTabActs, useTabKeys, useSelection, useSidebarSlot work
  sidebar: false,         // true if it uses the left sidebar (the spaces filter, its own sections)
  plus: 'New note',       // what its "+" is called (N runs it), or null
  secondary: null,        // its one secondary act, on ⇧Return, or null
  interrupts: ['notes'],  // the interrupt sources whose line opens this view when it names no other
});
```

| | |
|---|---|
| `registerView(def)` | adds or replaces; answers a function that takes it out |
| `allViews()` | every view, in key order |
| `viewById(id)`, `viewByShortcut(n)`, `viewForInterrupt(source)` | lookups |
| `useViews()` | the list as React state: redraws when a view registers |

The old way still works. Every id in `TAB_IDS` with a row in `TAB_TABLE` and a
component in `HEAT_TABS` is registered at start (`focus/builtin.tsx`), in that
order, on keys 1, 2, 3… So Database and Wiki, added to those three tables,
are on 7 and 8 in every summon path. `builtin.tsx` already holds a glyph for
`database`, `wiki` and `notes`.

## ⌘K

⌘K is the prompt-box agent's (`claude/ask-database-wiki`). The shell keeps the
`palette` overlay and the props it hands it. What this layout adds is in the
`actions` list: "Focus", one "Go to …" for each registered view with its key,
"Sync calendars" and "Show the player". Until `AskBox` merges, the existing
`CommandPalette.tsx` is the temporary palette on the same key: tools and tasks
by name. `AskBox` replaces it with the one import swap that branch already
makes in `Shell.tsx`; nothing here changes when it does.

In the Focus layout there is no search pill: ⌘K is the door.

## Entropy

How out of order things are, 0 to 1, worked out in the core from the snapshot
(`focus::entropy`):

| Part | Counts | Weight |
|---|---|---|
| `overdue` | open tasks past their due date | 3 |
| `unplanned` | open tasks due within `horizonDays` (7) with no time block from today on and no day scheduled | 2 |
| `mail` | threads not archived that made no task yet, or are unread and urgent or high | 1.5 |
| `grades` | grades waiting for a score | 0.5 |

`score = min(1, weighted sum / full)`, with `full` = 12. Levels: `clear` at 0,
`calm` below `busyAt` (0.2), `busy` below `highAt` (0.5), `high` from there.

It is exposed three ways: in every snapshot as `snapshot.focus.entropy`; as the
command `heat.entropy`; and as the event `entropy` `{score, level, parts}`,
sent whenever it is no longer what the views were last told. The shell puts
the score on the page's root as the CSS variable `--entropy`, the one variable
colour intensity reads: a band shows `prism.band.rest` of its colour at 0 and
all of it at 1, times the task's own heat. The risk colour (`prism.risk`) is
only for what is overdue or at risk.

Every threshold is in one place: `focus::Config` in the core, sent to the UI
as `snapshot.focus.config`. `CONFIG` in `model.ts` is the fake core's copy,
and the hint's two numbers are read from it; a test on each side pins both to
the same keys and values, so changing one without the other fails.

## Interrupts

One function decides: `wi_core::focus::should_interrupt(event, ctx)`. The
rule: interrupt only if it changes what the person should do next. It answers
the line to show, or nothing.

| Source | Raised when | Shows if | Action | Priority |
|---|---|---|---|---|
| `dueChanged` | someone other than the person moves an open task's due date (Claude, reading mail) | it is the Now task, or the new date is within 7 days, or the old one was within 48 hours | Show it | normal; high within 48 hours |
| `atRisk` | work left is more than the time that can still be planned before it is due | it is not already the Now task | Do it now | high |
| `mailUrgent` | mail makes a task | it is due within 48 hours, or its thread is urgent | Do it now | high |
| `focusEnded` | a focus round ends and its break waits | always | Start break | high |
| `leaveFor` | the next commitment starts within `leaveLeadMin` (15) | it has not started | Open Calendar | high |
| `gradeWaiting` | a grade waits for its score | always | Open Grades | low |
| custom | a view calls `heat.interrupt.raise` | `changesNext` is true | the view's own | the view's own |

While a focus round runs, only `high` gets through. At most one shows, the most
pressing first and then the oldest; the rest wait and are counted in
`snapshot.focus.queued`, which no screen draws.

The person's own edits never raise anything: an event is made of what another
process wrote (`watch.rs`), not of what the app's own commands write. A line
that is no longer true (the task is done, the date moved again) drops itself.
A dismissal is remembered for 30 days; none of this is journaled, so ⌘Z never
touches it.

"Time that can be planned" is, for each day up to the due date, the stretch
between `dayStartsMin` (7 AM) and `dayEndsMin` (10 PM) that is after now and
before the due date, less calendar events and other tasks' blocks, and never
more than `plannablePerDayMin` (6 hours).

A commitment is the next calendar event until commitments have a record of
their own (`focus::next_commitment`). When the snapshot carries
`commitments.next.line`, the readout says that line in place of its own NEXT.

## Commands

All are `heat.*`, and none is journaled.

| cmd | args | result |
|---|---|---|
| `heat.focus.state` | `{date?}` | `snapshot.focus`: `{entropy, now, next, interrupt, queued, config}` |
| `heat.entropy` | `{date?}` | `{score, level, parts: {unplanned, overdue, mail, grades}}` |
| `heat.interrupt.dismiss` | `{id}` | `{}` |
| `heat.interrupt.raise` | `{id, source, line, action?, changesNext, priority?}` | `{queued: true}` |
| `heat.focus.snooze` | | `{}`: the fix is put off until tomorrow |

`action` is `{label, do, taskId?, view?, cmd?, args?}` with `do` one of
`current` (make the task the Now task), `task` (show it in Tasks), `view`
(open a registered view), `break`, `plan`, and `command` (run `cmd` with
`args`; only a `heat.*` command is taken). A line raised with no action gets
"Open …" for the view that registered its `source`. After its action runs, a
line is dismissed. The last three commands send `heat` `{kinds: ["focus"]}`.

## The readout

The one ornament, and the link between Wi-WWAV and the Mi-WWAV device. The
device's screen is a 1602A character LCD, so the readout is that screen drawn
dot by dot on a canvas (`focus/Readout.tsx`): a dark field in both
appearances, a row of 5 × 8 cells a dot apart, every unlit dot faintly there.
The characters are the 5 × 7 patterns a character LCD holds in its ROM
(`focus/dots.ts`), not a font; a character the ROM lacks, such as 第, is set
small in the system's face and read back as dots, two cells wide.

It says, in capitals: the Now task and when it is due; then, from the right,
the next commitment and what is playing. When the row is short it drops from
the right and keeps the main line. With every loop closed it says
`ALL CLEAR · NEXT JPN 101 10:00 AM`. VoiceOver hears it as a sentence
("Readout. Now: Grammar quiz 4, Tomorrow 11:59 PM. Next JPN 101 10:00 AM."),
not as dots. Its colours and its dot size are `prism.readout.*`.

## The tokens

`design/tokens.json`, group `prism`; compiled with the rest to
`app/ui/src/styles/tokens.css`, `crates/wwav-tokens` and the engine's header.
Use the custom properties, never a value: `tools/tokens/compile.mjs --check`
fails a colour or a duration written anywhere else.

| Custom property | For |
|---|---|
| `--prism-ground`, `--prism-raised`, `--prism-sunk` | the page; what sits over it (a sheet, a sidebar); a well, a hover, a selection |
| `--prism-ink`, `--prism-ink2`, `--prism-ink3` | text: 7:1, 7:1 and 4.5:1 on all three grounds, in light and dark |
| `--prism-line` | the only border, where a control would be ambiguous without one |
| `--prism-fill`, `--prism-fill-ink` | the one filled button a screen has |
| `--prism-focus`, `--prism-focus-width` | the keyboard's ring |
| `--prism-backdrop` | behind a sheet: flat, no blur |
| `--prism-risk` | overdue and at risk, and nothing else; always beside words that say so |
| `--prism-band-blue`, `-green`, `-orange`, `-red` | the spectrum's four anchors: royal blue, xanadu green, blood orange, red |
| `--prism-band-width`, `--prism-band-rest` | a band's thickness; how much colour it keeps at entropy 0 |
| `--prism-readout-field`, `-unlit`, `-lit`, `-dim`, `-dot`, `-pitch`, `-height` | the readout |
| `--prism-s1` … `--prism-s8` | the 8 px scale: 8, 16, 24, 32, 48, 64, 96, 128 |
| `--prism-radius-control`, `--prism-radius-sheet` | 4 and 6 px: a machined edge, not a pill |
| `--prism-type-family`, `--prism-type-mono` | Instrument Sans for words, IBM Plex Mono for numbers, times and the readout; Hiragino Sans follows both for Japanese |
| `--prism-type-small`, `-body`, `-large`, `-title`, `-display`, `-line-height`, `-tight`, `-tracking` | 12, 14, 17, 22, 40 px |
| `--prism-motion-quick`, `-detent`, `-settle`, `-curve` | 150, 180, 240 ms; no bounce |
| `--prism-motion-edge-rest`, `--prism-motion-map-hold` | 150 and 600 ms |
| `--entropy`, `--spectrum` | set by the shell: entropy 0 to 1, and how much of a band's colour shows at it |

Two classes go with them: `.prism-fill`, the one filled button, and
`.prism-plain`, every other button.

**How a view takes the look.** With the layout on, the root carries
`data-layout="focus"`, and `prism.css` gives the desk's tokens the prism's
values: `--desk-well` is `--prism-ground`, `--desk-gel` is two equal stops,
`--gloss` is transparent, and so on. Every shared control already reads the
desk's tokens, so a view written with `.gel`, `.segment`, `.sheet`, `.check`
and the `--desk-*` properties is flat, monochrome and in the new faces with no
change, and a form's submit button is its filled one. A view that writes its
own gradient or shadow keeps it; use a token and it goes.

A space's band is its hue placed on the spectrum (`focus/band.ts`): on the
anchor it is nearest, or between two as a mix. Classes is blue, Personal is
green, and WWAV sits between red and orange.

Both faces are bundled (`app/ui/src/focus/fonts`, SIL Open Font License 1.1,
licences beside them), Latin only; nothing is fetched.

## Discoverability

For the first 7 days or 20 launches, whichever ends first, one faint line
stands under the task: "⌘K for anything · 1–6 for tools · hold ⌘ for the map".
Then it is gone for good. Settings → Appearance → "Show the shortcut hint
again" brings it back for as long again. The same pane has the click on Done
and on starting the timer, off until switched on.

## Accessibility

- Nothing hidden is unreachable. The tool list and the view switch are always
  in the page, only moved out of sight; tabbing into either brings it out.
- The edge reveal is `navigation` "Tools", the switch `navigation` "Views",
  the map `note` "Map of tools and shortcuts", the readout `img` "Readout. …",
  and the interrupt line `status` "Interrupt", so it is read when it arrives.
- Focus order: the view switch, the Now task's controls, the interrupt line,
  then the tool list.
- Colour is never the only signal: a band stands beside the space's name, the
  risk colour beside "overdue" or "At risk", a chosen length has a line under it.
- Contrast is checked by `crates/wwav-tokens/tests/contrast.rs` for every
  prism text pair on every ground, in light and dark. Entropy changes only the
  bands, which carry no text.
- Under Reduce Motion (the Mac's or the app's) nothing in the layout moves.

## Tests

- `app/ui/src/focus/*.test.ts(x)`: the model on the fake core; the registry;
  the readout's words and dots; and the layout on Learn's frame (launch, number
  keys, Esc, Done, the fix, All clear, a due date Claude moved, a view that
  registers, Classic).
- `crates/wi-core/tests/core/focus.rs` and the unit tests in `focus.rs`: the
  same rules through the commands, with the helper writing from another
  process.
- `app/ui/e2e-heat/focus.spec.ts`: a browser on the fake core: what a launch
  shows, the three ways to a tool, Esc, the ⌘ map, the top edge, Classic, and
  that no gradient is left in the shell.
