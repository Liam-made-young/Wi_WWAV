# wi-core

The app's Rust core, with no Tauri in it (docs/SPEC.md 9.1). The Tauri app
and `wi-devbridge` are two hosts of the same `Core`.

```rust
let core = Core::open(&library_dir, Config::new(engine_path, server_url, secrets, opener))?;
let state = core.invoke("player.play", json!({}))?;   // every command in docs/COMMANDS.md
let events = core.events();                            // {event, payload}
let meters = core.meters();                            // raw bytes, at most once a frame
```

- `Config.secrets` is a `SecretStore`: the operating system's keychain in the
  app, `MemorySecrets` in tests and the dev bridge. The account's tokens are
  the only secret the core keeps, under one item, "mi-wwav.com account".
- `Config.opener` opens a URL in the system browser (sign-in).
- `Config.device` is the engine's `--device` ("null" for the timer);
  `Config.engine_test` its `--test`; `Config.tmp_dir` where its private
  socket folder goes. wwav-wire names that folder by the app's pid, so two
  cores in one process need two `tmp_dir`s.
- `Config.helper` is where Settings → Claude says the MCP helper is
  (`wi-mcp`); unset, it is `WI_WWAV_MCP`, else `Contents/Helpers/wi-mcp` in an
  app bundle, else `wi-mcp` beside the app's binary. `Config.now` and
  `Core::set_now` fix "now" (milliseconds) for tests of what the time decides.
- `Core::open` returns at once; the engine starts in the background and
  `engine.status` says "starting" until it runs. Learn doesn't wait for it: a
  missing engine binary leaves the engine "stopped", with its sentence, and
  every `heat.*` command works.

## Beyond docs/COMMANDS.md

| cmd | args | result |
|---|---|---|
| `library.rename` | `{id, title, label}` | `{clips}`: Get Info's title; the file keeps its name and bytes |

Rust-only, for the Console and tests: `Core::pause_player(PausedFor::Console
| Film)` (2.3: pressing play in the Console), `Core::sync_heat()` (a Learn sync
round now), and `Core::engine()` (load a session, call an op, read the clock
and meters, the session hash).

Learn's commands (`heat.*`, `history.undoEntry`) are `docs/HEAT.md`'s. Each is
a thin call into `wi-heat-store`, which `wi-mcp` shares, so a rule exists
once. Three workers sit behind them: the watcher (`PRAGMA data_version` every
500 ms; what another process, the MCP helper, committed is read from the
journal and announced as `heat` and `history`, and goes up with the next
sync), the calendars (read on open if the last read was over 15 minutes ago,
then hourly; addresses from the Keychain through `Config.secrets`) and Heat
sync (`Core::sync_heat()`).

Events beyond COMMANDS.md: `engine` (the status on every change), `library`
(`{ids}` after a library change), `heat` (`{kinds}`), `account`, `settings`
and `export` (progress). The engine's own `key` events (keys a plugin window didn't use,
docs/ENGINE.md 3.7) aren't forwarded yet: mock-engine has no plugin windows
to send one, so nothing could test it.

## For the Tauri app

`app/src-tauri` hosts the core and gives the web UI one command, `core`,
taking `{cmd, args}` and answering the result or rejecting with the
`CoreError` (`{code, message}`). It sends each `Event` to the main window as
the Tauri event `core` `{event, payload}`, and each meters frame to the
`Channel` the UI names with `core` `meters.listen {channel}`, as raw bytes
(`InvokeResponseBody::Raw`), never JSON. `app/src-tauri/README.md` has the
whole contract, and `app/ui/src/bridge` is its other end.

The app calls no model. Claude reaches Learn through an MCP server that
reuses wi-heat's rules (docs/SPEC.md 2.11, 8.8), so the core has no
`assist.call` and no Anthropic key; Settings → Claude's switches for the
helper's tools are `heat.claude.setTool`.

## Tests

`cargo test -p wi-core` builds the workspace's mock-engine and runs the core
against it, and against `tools/mock-server` (Node 22) for the account, the
upload queue and Learn sync. `tests/core/mock_counts.mjs` starts the mock with
a side door that counts each upload part's sends.

Learn's tests (`heat_commands`, `heat_calendars`, `heat_public`, `heat_watch`)
stand a core at a fixed time in New York (`Core::set_now`), read calendars from
a feed server in the test, and build `wi-mcp` once and run it as a second
process on the same library, to check that the app notices what Claude wrote.
