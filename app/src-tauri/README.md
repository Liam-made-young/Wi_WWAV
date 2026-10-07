# Wi_WWAV.app: the Tauri shell

The window, the menus and the bridge to the core (`docs/SPEC.md` 9.1). The
core itself is `crates/wi-core`; this crate opens it on the library and
hosts it. What the web UI sees of the shell:

| Way | Shape | Notes |
|---|---|---|
| Command | `invoke('core', {cmd, args})` → result, or `{code, message}` | Every cmd of `docs/COMMANDS.md`, as far as the window's capability allows |
| Command | `core` `meters.listen {channel}` → `{}` | Meters arrive on that `Channel` as an `ArrayBuffer`, the newest entry, at most once a frame |
| Command | `core` `shell.room {room}` → `{}` | The room now showing (`heat`, `space`, `console`, `unquantized`, `library`), so the Edit menu names its undo and View ticks it |
| Event | `listen('core', e => …)`, `e.payload` = `{event, payload}` | Every core event, the same shape the dev bridge pushes, only to the main window |
| Event | `{event: "menu", payload: {action}}` | A menu item the UI acts on: `session.new`, `export.everything`, `disc.make`, `history.undo`, `history.redo`, `library.toggle`, `room.heat`, `room.space`, `room.console`, `room.unquantized` |
| Event | `{event: "open", payload: {clips} \| {sessions} \| {error}}` | Files opened from outside: songs and films already imported through `library.import`, sessions as paths for the Console |
| Event | `{event: "status", payload: {area: "update", sentence}}` | "Update ready · installs when you quit" |

On Linux a menu's keys never reach the page (GTK hands Ctrl+1 to the menu
first; the end-to-end check shows it), so the room keys arrive only as
`menu` events, and the UI must act on those as on its own keys. On macOS
WKWebView offers ⌘1 to the page first, and one the page handles with
`preventDefault` should not reach the menu as well; that is still to check
on a Mac. The Undo item replaces the system's, so when a text field
has focus the UI should treat `history.undo` as `document.execCommand('undo')`.

The settings window loads `index.html?window=settings`.

## Windows and what they may call

`capabilities/` gives each window its commands (`docs/SPEC.md` 9.8):

- `main.json`: every `core` cmd, listening to events, dragging the title bar,
  and the system's open and save pickers (`plugin:dialog|open`, `save`).
- `settings.json`: `app.hello`, `app.settings.*`, `account.*`, `engine.*`,
  `library.cleanup.*`, `library.trash.empty`, and the open picker.
- A window in no file, like the checkout window that shows Stripe's page,
  can call nothing. No window can drive the updater.

`build.rs` declares `core` in the app's manifest; without that, Tauri lets
every local window call it.

## Running it

```
cd app/src-tauri
cargo tauri dev                           # the UI from Vite, the core on ~/Music/Wi_WWAV
cargo tauri build --debug --no-bundle     # the UI built in, for the end-to-end check
cargo build -p mock-engine                # the engine the end-to-end check runs
node ../ui/e2e-webkit/run.mjs             # needs tauri-driver, WebKitWebDriver, xdotool, Xvfb
```

Run the Tauri CLI from this folder: the build hooks find `ui/` from here.

| Variable | Default | For |
|---|---|---|
| `WI_WWAV_LIBRARY` | `~/Music/Wi_WWAV` | a throwaway library |
| `WI_WWAV_ENGINE` | `wwav-engine` beside the app (`Contents/Helpers/wwav-engine.app` on macOS) | `mock-engine` or a fresh build |
| `WI_WWAV_SERVER` | `https://www.mi-wwav.com` | `tools/mock-server` |

`tests/keychain.rs` needs `dbus-daemon`, `gnome-keyring-daemon` and
`secret-tool` (Debian and Ubuntu: dbus, gnome-keyring, libsecret-tools); it
runs its own bus and keyring in a throwaway home. Shipping is in
`tools/release/`.
