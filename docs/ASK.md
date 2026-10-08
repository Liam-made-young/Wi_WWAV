# The prompt box, the Database tab and the Wiki tab

Three parts of Learn, and how they are built: the Claude prompt box on ⌘K,
the Database tab, and the Wiki tab. `docs/HEAT.md` says how the rest of Learn
fits together; this file follows its shape. Where they disagree about Learn's
own records, HEAT.md wins.

One rule runs through all three. **Every action is a command of the core.**
The views call it, and Claude in the prompt box calls the same command as a
tool. Nothing is written a second way, so a rule in `schema.rs` holds for a
cell typed in the grid, a row Claude adds, and a task made in Tasks alike.

## Where things live

| Part | Does | Never |
|---|---|---|
| `crates/wi-formula` | The formula language: reads a formula, works it out for a row, lists the columns it reads | I/O, the clock, a dependency |
| `crates/wi-wiki` | Wikipedia's article HTML as blocks of text; a formula's TeX as a line of text | the network |
| `crates/wi-store/src/merge.rs` | `Store::merge_entries`: journal entries made one after another become one | change what an entry did |
| `crates/wi-heat-store/src/tables.rs` | Reads the schema's field lists out for the Database tab | a write |
| `crates/wi-core/src/batch.rs` | `one_step`: runs commands, then joins the entries they made on this thread | write anything itself |
| `crates/wi-core/src/db/` | The `db.*` commands: `catalog` (tables and columns), `sheet` (cells and formula columns), `query` (views, pivots, charts), `write` (edits, rows, columns, tables, views, CSV) | reach Learn's records except through `heat.*` |
| `crates/wi-core/src/wiki.rs` | The `wiki.*` commands: fetch, cache, wait its turn | parse HTML |
| `crates/wi-core/src/ask.rs`, `ask_index.rs`, `ask_tools.rs`, `ask_mcp.rs` | The `ask.*` commands: search, one run of Claude Code with the core's tools, staged changes | hold a key, name a model beyond its size, make a change Claude asked for |
| `app/ui/src/ask/` | The prompt box, its Markdown, where a link goes (`nav.ts`), what is on screen (`context.ts`) | set HTML |
| `app/ui/src/heat/database/` | The Database tab: the grid, the forms, the pivot, the chart | arithmetic: the core totals, sorts and works formulas out |
| `app/ui/src/heat/wiki/` | The Wiki tab | the network: the web view can reach only the app |

## The prompt box

⌘K opens it where Search was (`app/ui/src/ask/AskBox.tsx`). The shell's
overlay is still called `palette`, and its keys still come through the one
router.

**As a search.** While something is typed, matches show at once: actions,
Learn's records, settings, clips. Learn's records come from `ask.search`,
which reads the library's own full-text index (`docs_fts`): tasks, courses,
notes, habits, mail notes, grades, projects, milestones, captures, spaces,
and the rows, tables and views of the Database tab. Every word must be
there; each matches as a prefix. No Claude, no network. ↓ picks a result and
Return opens it; ⌘Return opens it in its other view.

**Return with nothing picked asks Claude.** That is the one change to the
keys: Search opened its first result on Return, and the box asks. The first
row says so ("Ask Claude: …") and is the one selected.

**The answer** draws under the field as Markdown, made into elements one by
one (`markdown.tsx`), never set as HTML. A link in it is one of three kinds
and nothing else is a link:

| Address | Is | Opens |
|---|---|---|
| `learn://<table id>/<row id>` | a Learn record | a task as it always has (Tasks, selected); any other as its row in the Database tab |
| `wiki://<Title>` | a Wikipedia article | in the Wiki tab; ⌘click opens it behind what you are reading |
| `https://…` | a web page | in the system browser |

The articles an answer links, and any it read, are also listed under it as
chips.

**What is on screen** goes with each request (`context.ts`, `screen.ts`):
the tab, the day, the space, what is selected, the items in view by title
and id, and from the Database tab the table, view, columns, filters and
selected rows, and from the Wiki tab the article and the section being read.
So "move these to Friday" and "explain this section simpler" mean something.
The last four turns go too.

### Changes are staged

A tool that changes data does not change it. It checks the change as far as
it can without making it (the row is there, the column can be typed in, the
value reads, the formula works on the table's first rows), describes it in a
line, and keeps it on the run. A mistake comes back to Claude as a sentence
naming the row and the column, so it can put it right before the person sees
anything.

The answer then carries `change`: a summary ("4 tasks moved, 1 task
created") and a line for each. Nothing has been written. **Apply** (or
Return on the empty box) calls `ask.apply`, which runs every staged command
inside `batch::one_step` and joins the journal entries they made into one,
labelled `Claude: 4 tasks moved, 1 task created`. One ⌘Z takes all of it
back, and one redo puts it back. A staged change that fails when it is made
(another edit got there first) is listed as not made; the rest are made.
**Discard** lets the run go.

A row made and used in the same answer (a new task, then a block for it) is
named by a stand-in id, `new:1`, swapped for the real id as the changes are
made.

### What leaves this Mac

Four tools have an effect outside this Mac: `mail_send`, `mail_reply`,
`mail_file` (archive, unarchive, mark read or unread: the mailbox is not on
this Mac) and `make_public`. They are never part of Apply. Each is shown on
its own, whole (a mail with its To, its Subject and its body word for word),
with Yes and No, and `ask.applyOutward {id, index}` makes that one. A yes
given twice does it once. One never answered never happens. There is no
setting that turns the asking off.

### The run

`ask.send` is one run of the person's own Claude Code (`claude -p`), found
as `claude_cli::binary` finds it. The app holds no key. The run is told of
exactly one MCP server, the core's, and has no other tool:

```
claude -p --output-format stream-json --verbose --no-session-persistence
       --disable-slash-commands --tools "" --strict-mcp-config
       --mcp-config '{"mcpServers":{"learn":{"type":"http","url":"http://127.0.0.1:<port>/mcp",
                      "headers":{"Authorization":"Bearer <token>"}}}}'
       --allowedTools mcp__learn__search,mcp__learn__list_rows,…
       --system-prompt <what Learn is, today, the tables, the rules> --model sonnet
```

`--strict-mcp-config` means the connectors the person has given Claude Code
(Gmail, Calendar) are not loaded, which is also why a run is cheap: the
answer's `cost` is what Claude Code reports for it. The model is `sonnet`
unless `ask.send` is given `model`, or `WI_WWAV_ASK_MODEL` is set. The
request goes on stdin: what is on screen, the earlier turns, the request.
The output is read as a stream, so words Claude wrote before calling a tool
are part of the answer.

**The server** (`ask_mcp.rs`) is MCP's streamable HTTP in its plainest form:
one JSON-RPC message per POST to `/mcp`, answered as JSON. It listens on
127.0.0.1, on a port the system picks, from the first question until the app
closes. A request is answered only if it carries the token of a run that is
still asking; a token stops working when its answer is in. A request with an
`Origin` (a web page) or a `Host` that isn't this address is refused before
it is read.

**Failures** are one sentence, code `claude`: Claude Code isn't installed,
is signed out, can't be reached, didn't answer in four minutes, or was
stopped (`ask.cancel`). The box shows the sentence and goes on searching.

### The tools

Twenty-four (`ask_tools.rs`; `ask.tools` lists them). Tables and columns are
named as the Database tab names them.

| Tool | Effect | Is |
|---|---|---|
| `search` | reads | `ask.search` |
| `list_rows`, `get_row` | reads | `db.query`; a mail note's saved text from `heat.mail.text` |
| `today` | reads | `heat.snapshot` for a day, as Today shows it |
| `summarize` | reads | `db.pivot` |
| `grade_needed` | reads | `heat.whatItWouldTake` |
| `wiki_search`, `wiki_get_article` | reads | `wiki.search`, `wiki.text` |
| `open` | reads | shows an article, a row, a table or a tab when the answer is shown |
| `update_rows`, `complete_tasks` | changes | `db.cells.set`, which is `heat.patch`, `heat.done` |
| `create_rows` | changes | `db.rows.add`, which is `heat.put` |
| `delete_rows` | changes | `db.rows.delete`, which is `heat.delete` |
| `schedule_block` | changes | `heat.block.put` |
| `plan_day` | changes | `heat.plan.make` at once (drafts, as Plan my day makes them); `heat.plan.accept` staged |
| `tick_habit` | changes | `heat.patch` on the habit's log |
| `capture` | changes | `heat.capture.add` |
| `add_formula_column` | changes | `db.column.add`, checked with `db.formula.check` |
| `save_view`, `create_table` | changes | `db.view.save`; `db.table.create` and `db.rows.add` |
| `mail_send`, `mail_reply`, `mail_file` | leaves this Mac | `heat.mail.send`, `.reply`, `.archive`, `.mark` |
| `make_public` | leaves this Mac | `heat.public.set` |

Because `list_rows`, `update_rows` and `create_rows` take any table, a kind
of record another part of Learn adds to the schema is readable and writable
by Claude the day it is added, with no new tool.

**Tools from another part of Learn** are mounted, not written here:
`ask_tools::EXTRA` takes a list of tools and one function that turns a call
into the core command that does it. The box then treats each by its effect
as it treats its own: a read runs at once, a change waits for the preview,
and what leaves this Mac waits for its own yes.

Flashcards and study notes are rows of Notes: a note titled "Flashcards:
<topic>" whose Markdown holds a `Q:` line and an `A:` line per card.

These tools are the prompt box's own. `wi-mcp`, the helper Claude Desktop
and Claude Code start, is unchanged and keeps its rule: it drafts and
estimates and never marks done, deletes or makes public.

### Commands

| cmd | args | result |
|---|---|---|
| `ask.search` | `{q, limit?, kinds?}` | `{results: [{kind, id, title, hint, word, table}]}` |
| `ask.status` | `{}` | `{available, reason?}`: whether Claude Code was found |
| `ask.tools` | `{}` | `{tools: [{name, effect, description}]}` |
| `ask.send` | `{prompt, context?, history?, model?}` | `{id, answer, wiki, opens, change, outward, tools, cost, seconds}`. `change` is `{summary, lines, count}` or null; `outward` is `[{index, line}]`. Sends `ask` events `{id, doing, tool?}` while it runs |
| `ask.apply` | `{id}` | `{applied, failed: [{line, message}], summary, undo, steps}` |
| `ask.applyOutward` | `{id, index}` | `{done, line, undo?, sentence?}` |
| `ask.discard` | `{id}` | `{}` |
| `ask.cancel` | `{id}` | `{}`: stops a run that is still asking |

## The Database tab

A spreadsheet over Learn's own data, and tables of the person's own.

### Tables and columns

There is a table for every kind of record `schema.rs` lists, read through
`wi_heat_store::tables::kinds()`, and for the kinds it doesn't spell out
(`mailThread`, `calendar`, `calendarEvent`, `profileShare`). Two more are
made from part of a record: Habit log (a habit's days) and Grade categories
(a course's). A kind added to the schema is a table with no change here: its
columns are named from its fields (`estMin` reads "Est min"), and a field
`catalog.rs` has no type for is typed from what its records hold.

| Type | Holds | In a formula |
|---|---|---|
| `text`, `number`, `bool` | as said | text, a number, TRUE or FALSE |
| `date` | a day, `YYYY-MM-DD` | a day count |
| `datetime` | an instant (epoch ms), shown in the person's zone as `2026-10-09 23:59` | a day count with the time as its fraction |
| `relation` | another row's id; the cell is `{id, label}` and shows as a link that jumps to that row | the label (a course's code, a task's title) |
| `json` | a list or an object, shown as text | text |
| `formula` | whatever its formula gives | that |

**Locked** cells are drawn quieter and say why when tried: a record's own
id; what Learn fills in itself (who estimated, where a record came from,
when it was done); a list; a column Learn works out (a task's Heat, Planned
and Logged minutes, a course's Current % and Letter, a grade's Percent, a
focus session's Date and Course); every cell of a table nobody edits (Mail
is Claude's, calendars are set in Settings, a habit's days are ticked in
Habits); and a formula column.

### Edits

`db.cells.set {table, edits: [{row, column, value}], label?}` takes what was
typed or pasted and reads it as the column's type: `1,234.5` and `45%` are
numbers; `Oct 9, 2026`, `10/9/2026` and `2026-10-09 17:00` are dates; a due
date with no time is due at 11:59 PM that day; a relation takes the other
row's name or id; empty clears. For one of Learn's kinds each row's edits
are one `heat.patch` (a task's Done is `heat.done`, a Public switch is
`heat.public.set`), so the real record changes by the real rules. An edit
that can't be made is left out and said in `failed`; the others are made.
The whole call is one undo step, labelled `paste`, `fill down`, `clear
cells`, or by the one change it made ("rename task").

### Formulas

A formula column can be added to any table (`db.column.add`); it is the only
kind of column that can be added to one of Learn's own. The language is
`wi-formula`'s, written for this app: the spreadsheet engines with this
coverage are GPL or carry a commercial licence.

- `[Name]`, or a bare `Name` when it is one word, is this row's value.
- `Table[Name]` is a whole column, of this table or another:
  `SUMIFS(Focus sessions[Minutes], Focus sessions[Course], [Code])`.
- `+ - * / ^ &`, comparisons, `%`, text in quotes, `TRUE` and `FALSE`.
- Dates are day counts in the person's zone: `[Due] - TODAY()` is days.
- Ninety functions: totals (`SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNT`,
  `MEDIAN`), the IF family (`COUNTIF`, `SUMIF`, `SUMIFS`, `AVERAGEIFS`,
  `MAXIFS`), logic (`IF`, `IFS`, `AND`, `OR`, `SWITCH`, `IFERROR`), numbers
  (`ROUND`, `FLOOR`, `MOD`), dates (`TODAY`, `DAYS`, `WEEKDAY`,
  `STARTOFWEEK`, `DATEDIF`, `EDATE`), text (`LEFT`, `MID`, `SUBSTITUTE`,
  `TEXTJOIN`, `TEXT`) and lookups (`LOOKUP`, `MATCH`, `INDEX`, `FILTER`,
  `UNIQUE`). `db.tables` answers the whole list.

A formula that can't be read is refused with why and where. One that fails
on a row is a value in that cell (`#DIV/0!`, `#REF!`, `#N/A`) with its
sentence. Two that read each other are `#CYCLE!`.

In plain English: ask the prompt box ("add a column for hours logged per
course this week"). Claude writes the formula, `add_formula_column` checks
it against the table, and the column is in the preview.

### Views

A view's spec is what `db.query` reads and what a saved view keeps:

```jsonc
{ "filters": [{"column": "due", "op": "lt", "value": "2026-10-12"}], "match": "all",
  "search": "kanji", "sorts": [{"column": "due", "dir": "asc"}], "group": "courseId",
  // what only the grid reads:
  "hidden": [], "order": [], "widths": {}, "frozen": 1, "summary": {"estMin": "sum"},
  "pivot": {"rows": ["courseId", "type"], "values": [{"column": "estMin", "agg": "sum"}], "across": false},
  "charts": [{"id": "c1", "type": "bar", "x": "courseId", "y": ["estMin"], "agg": "sum"}] }
```

A column is named by its id or its name. Blanks sort last either way. A
date filter with no time matches any time that day. A saved view is a
`dbView` record, journaled; a table's last layout is a `dbLayout` record,
outside the journal, so resizing a column is not something ⌘Z reaches.

### What is stored

| kind | Holds | Journaled |
|---|---|---|
| `dbTable` | a table of the person's own: `{id, name}` | yes |
| `dbColumn` | an added column, on any table: `{id, table, name, type, order, formula?}` | yes |
| `dbRow` | a row of a person's table: `{id, tableId, order, cells}`; its text is in the search index | yes |
| `dbView` | a saved view: `{id, table, name, spec}` | yes |
| `dbLayout` | a table's last layout, keyed by the table | no |

None of them leaves this Mac (`heat.rs`'s `local_only`). Export everything
includes them, as it does every kind.

### Commands

| cmd | args | result |
|---|---|---|
| `db.tables` | `{}` | `{tables: [{id, name, origin, count}], views, layouts, functions}` |
| `db.query` | `{table, spec?, view?, limit?, offset?}` | `{table, columns, rows: [{id, cells}], total, all, summary, groups}` |
| `db.pivot` | `{table, rows: [1 or 2 columns], values: [{column, agg}], spec?}` | `{by, values, groups: [{keys, count, values}], total}` |
| `db.chart` | `{table, x, y: [columns], agg?, spec?}` | `{x: {name, kind}, labels, xs, series: [{name, values}]}` |
| `db.formula.check` | `{table, formula}` | `{ok, sample}` or `{ok: false, message, at}` |
| `db.cells.set` | `{table, edits, label?}` | `{changed, failed, undo}` |
| `db.rows.add`, `db.rows.delete` | `{table, rows}` | `{ids \| deleted, failed, undo}` |
| `db.table.create`, `.rename`, `.delete` | `{name}`, `{table, name}`, `{table}` | `{table?, undo}`; a delete takes the rows, columns and views with it, as one entry |
| `db.column.add`, `.update`, `.delete` | `{table, name, type, formula?}` … | `{column?, undo}` |
| `db.view.save`, `db.view.delete` | `{table, name, spec, id?}`, `{id}` | `{view?, undo}` |
| `db.layout.set` | `{table, spec}` | `{}` |
| `db.csv.import` | `{name, csv}` or `{name, path}` | `{table, rows, columns, undo}`: a new table; each column's type is read from what it holds |
| `db.csv.export` | `{table, spec?, to?}` | `{csv, name, rows}`, or with `to` a file: a path, or `"downloads"` |

### The grid

Rows are all one height, so the rows in view come from the scroll position
and a table of thousands costs a screenful. Keys reach it through the
frame's router (`useTabKeys`): arrows, ⇧arrows, ⌘arrows, Tab, Return or F2
to edit, a letter or a digit to type over, ⌫ to clear, ⌘A, ⌘D to fill down.
⌘C, ⌘X and ⌘V are the clipboard's own events, as tab-separated text. A
header's menu sorts, filters, groups, hides and freezes; its edge resizes;
dragging it reorders. Charts are bars, a line or a scatter on one axis, with
a legend for two or more series, a hover readout, and the same numbers as a
table one click away; four series at most, in four colours checked together
for colour-blind separation and contrast on both surfaces.

The Database tab has no secondary act, and neither has the Wiki tab: the
frame gives ⇧click to a tab's secondary act, and in both ⇧click selects.

## The Wiki tab

Wikipedia as a reader. Text only, read-only.

**Source.** Live, through the REST API (`/api/rest_v1/page/html/`,
`/page/summary/`) and the Action API (`prefixsearch` for suggestions,
`search` for a query, `morelike:` for Related). Every request carries a
User-Agent that names the app and where to reach its maker
(`Wi_WWAV/<version> (https://www.wi-wwav.com; Learn's Wikipedia reader)`),
as Wikimedia asks. Requests go one at a time, 100 ms apart, and a 429 or 503
with `Retry-After` is obeyed: nothing is sent until that time passes.

**What an article becomes** (`wi-wiki`): headings, paragraphs, lists,
tables, quotes and preformatted text as JSON blocks, with every link sorted
into one of three kinds: another article (opens in the tab), a place in this
article (scrolls), the web (system browser). A link to a file, a template, a
talk page, a category or a page that doesn't exist is plain words. Left out:
pictures and their captions, navigation boxes, maintenance banners, edit
links. The infobox is kept as a folded table of its facts, after the first
paragraph, without its picture rows. References are gathered into one list,
folded at the foot. A formula's TeX becomes a line of text (`\hat{f}(\xi)`
reads f̂(ξ)). The licence and a link to the article on Wikipedia are under
every article.

**Kept on this Mac.** An article opened once is in `wiki-cache.sqlite` in
the library folder, with its summary and its related list. It reopens from
there at once, is read again when it is over a week old, and is what you get
with the network off (the page says so, with the date of the copy). The
search box then finds what is saved. The file is a cache: deleting it loses
nothing else.

**In the tab.** A click on a link opens the article; resting on one for
350 ms shows its title and first paragraph; ⌘click reads it into this Mac
and lists it under Opened for later, leaving what you are reading in front.
⌘[ and ⌘] go back and forward. The trail across the top is the path read
this session, and any step of it can be gone back to. The sidebar holds the
contents, with the section being read marked, then up to eight related
articles.

### Commands

| cmd | args | result |
|---|---|---|
| `wiki.suggest` | `{q}` | `{results: [{title, description}], offline?}` |
| `wiki.search` | `{q, limit?}` | `{results: [{title, snippet}], offline?}` |
| `wiki.article` | `{title, refresh?}` | `{article, fetchedAt, cached, offline?}`, or `not_found`, `offline`, `slow_down` |
| `wiki.summary` | `{title}` | `{title, description, extract, disambiguation}` |
| `wiki.related` | `{title}` | `{results: [{title, description}]}` |
| `wiki.text` | `{title, section?, maxChars?}` | `{title, text, sections, url, license}`: what Claude reads |
| `wiki.saved` | `{}` | `{articles: [{title, fetchedAt}]}` |
| `wiki.open` | `{url}` | `{}`: a web address in the system browser; anything else is refused |

## Tests

- `crates/wi-formula/tests/formulas.rs`: every family of function against
  two small tables; a formula that can't be read says why and where.
- `crates/wi-wiki/tests/article.rs`: a page shaped like the REST API's;
  nothing that runs or loads comes through.
- `crates/wi-store/tests/merge.rs`: a batch is one undo and one redo, and
  undoing everything after a merge returns the first state.
- `crates/wi-core/tests/core/db.rs`, `wiki.rs`, `ask.rs`: through the core's
  commands. `wiki.rs` runs against a stand-in for Wikipedia; `ask.rs`
  against `fake_claude.mjs`, a stand-in for the command line that calls the
  core's tools over HTTP as the real one does.
- `app/ui/src/ask/*.test.tsx`, `heat/database/*.test.tsx`,
  `heat/wiki/Wiki.test.tsx`: the views on a stand-in for the core
  (`ask/testkit.ts`).

## What isn't here

- Pictures in articles, and formulas drawn as formulas.
- Sending mail without being asked, by any setting.
- A grid key for Space: the shell's router gives Space to play and pause.
- Flashcards as records of their own: they are notes.
