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
- `Core::open` returns at once; the engine starts in the background and
  `engine.status` says "starting" until it runs.

## Beyond docs/COMMANDS.md

| cmd | args | result |
|---|---|---|
| `library.rename` | `{id, title, label}` | `{clips}`: Get Info's title; the file keeps its name and bytes |
| `assist.call` | `{task, body}` | `{result}`, or `consent_needed` (with the consent sentence), `not_granted`, `rate_limited`, `daily_limit`, `offline`, `unavailable`, `signed_out`. Tasks are the mock server's: `score`, `score-batch`, `read-mail`, `syllabus`, `review-note`, `release-plan`, `feedback`, `clerk` |

Rust-only, for the Console and tests: `Core::pause_player(PausedFor::Console
| Film)` (2.3: pressing play in the Console), `Core::sync_heat()` (a Heat sync
round now), and `Core::engine()` (load a session, call an op, read the clock
and meters, the session hash).

Events beyond COMMANDS.md: `engine` (the status on every change), `library`
(`{ids}` after a library change), `account`, `settings` and `export`
(progress). The engine's own `key` events (keys a plugin window didn't use,
docs/ENGINE.md 3.7) aren't forwarded yet: mock-engine has no plugin windows
to send one, so nothing could test it.

## For the Tauri app

`app/ui/src/bridge` expects two commands: `core` taking `{cmd, args}` and
answering `invoke`'s result or rejecting with the `CoreError` (`{code,
message}`), and `core_listen` taking `{events, meters}`, two `Channel`s, once
per window: send each `Event` on `events` and each meters frame on `meters`
as raw bytes (`InvokeResponseBody::Raw`), never JSON.

## Tests

`cargo test -p wi-core` builds the workspace's mock-engine and runs the core
against it, and against `tools/mock-server` (Node 22) for the account, the
upload queue and Heat sync. `tests/core/mock_counts.mjs` starts the mock with
a side door that counts each upload part's sends.
