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
