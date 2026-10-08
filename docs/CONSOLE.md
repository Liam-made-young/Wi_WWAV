# Console

Phase 0 supplies the four-tool shell and the shared local Library. Write,
Image, Audiovisual and 3D retain separate open documents and selections.
Mounted editors retain drafts across tool and room switches. Saved workspace
state survives restarting. Unsaved drafts are memory-only, guarded against
closing a tab or leaving the browser; they are not crash recovery.

This branch starts from the combined Focus, Ask and Notes baseline `eaec951`.
Console's UI lives in `app/ui/src/console/`; its core lives in
`crates/wi-core/src/console/`. The only shared Rust integration is the public
module and the `console.*` dispatcher in `Core::invoke`.

## Scope

Each tool has an independent document surface and the same document actions.
Write has a plain-text field for testing saves and authorship. The other
native documents have empty canvases. Imported text, images, audio and video
have read-only readers. This phase does not implement markdown editing,
brushes, a timeline, modeling, rendering or media export.

Keys 1-4 switch tools while Console is active and focus is outside a field.
Command-T opens its Claude entry; Command-S saves. Command-Z outside text
fields restores a saved document version; text fields keep normal typing undo.
The version buttons always address the active document. Learn and shell
overlays retain their keys. The native application's Edit menu still uses
the shared journal: a shell-owned adapter is requested in COORDINATION.md.

The Focus registry mounts views inside Learn's frame, not across app rooms.
Console exposes `CONSOLE_TOOLS` in `registry.ts` for its four tools. It uses
the shared prism custom properties, with existing desk tokens as fallbacks.
No Learn, Ask or Focus-owned files are changed.

## Disk Library

The root is `<Core::library()>/Wi-WWAV Library/`. It is independent of the
existing music Library drawer and Learn's Notes folder.

```text
Wi-WWAV Library/
  .console.lock
  workspace.json
  documents/<document ULID>.wwwork/
    manifest.json
    assets/<SHA-256>.<ordinary extension>
  proposals/<proposal ULID>.json
  exports/<document ULID>-<version ULID>.space/
    <export filename>
    space.json
```

Assets are ordinary files readable outside the app. Bundles use format
`wi-console/0.1`. Their manifest contains `id`, `tool`, `createdAt`, `head`,
`parent`, an append-only `versions` list, and undo/redo stacks. Each version
records `id`, `previous`, time, actor, action, title, asset, `changes` and
optional `restoredFrom`. Assets include name, relative path, MIME type, byte
size, hash and a short text preview. Native empty assets are `draft.md`,
`image.json`, `timeline.json` and `scene.json`.

The JSON assets for the three later editors are explicitly version-0 empty
placeholders, not finished export formats. Console owns the future Write
markdown section/manifest bundle and Image JSON/PNG/SVG layer bundle, with
versioned migrations to this shared record. Existing `wwav-formats` readers,
writers and tests remain authoritative for `.wwav` and `.swav`; this phase
copies their bytes without inventing a replacement media format.

Every save appends a version, including a save with unchanged content.
A variation copies the exact current asset into a new document and records
its parent's document ID and version ID. Later edits cannot alter that link.
Undo and redo append restore versions rather than deleting history.

Writes use a process-wide OS file lock, atomic temp-file rename, and fsync.
Saves supply `base`, the version the editor read; stale writes return
`conflict`, preserving the existing file and the UI draft. Assets are hashed
on read. An external asset modification is reported and must be reimported
as a new file. Corrupt bundles are listed as issues without hiding good files.

## Authorship

UI edits use actor `hand`; Console tool calls and proposal applications force
actor `claude` in Rust. Client-supplied actor fields cannot change this.
Imports use actor `import`, since Console cannot know their earlier origin.

- **Made by hand:** all recorded creation/edits were through the hand path.
- **Claude assisted:** any recorded or inherited Claude edit, including an
  edit later undone. The history explains restores.
- **Origin unverified:** an import or inherited import without a recorded
  Claude edit. This avoids labeling imported AI work as human-made.

The marker is an honest local activity record, not an authenticity certificate
or tamper-proof signature. Each marker opens actor and change details, and
every immutable version can be read without changing the document.

## Core APIs

All document mutations run in Rust. `console::tool_definitions()` returns
JSON tool definitions, and `console::call_tool(core, name, args)` invokes the
same implementation with Claude's actor. The tool names are underscore-safe
for MCP. Their current registry includes local import as well as all saved
document operations.

| Command | Arguments / result |
| --- | --- |
| `console.workspace` | persisted tool/open documents/selection and disk root |
| `console.selectTool` | `{tool}` |
| `console.selection` | `{tool, selection}` |
| `console.library` | `{tool?, query?}`; documents and unreadable-bundle issues |
| `console.create` | `{tool, title}`; document summary |
| `console.import` | `{path, title?}` or `{name, base64, title?}` |
| `console.open`, `console.close` | `{id}`; workspace tab operation |
| `console.read` | `{id, version?}`; text or base64, path, `readOnly: true` |
| `console.history` | `{id}`; immutable versions, parent and record explanation |
| `console.save` | `{id, base, title?, text?}`; a new version |
| `console.variation` | `{id, base, title}`; new document with exact parent |
| `console.undo`, `console.redo` | `{id, base}`; a new restore version |
| `console.post.prepare` | `{id, base}`; local package path, `published: false` |
| `console.tools`, `console.tool.call` | definitions; `{name, args}` |
| `console.claude.context` | `{tool, id?, selection?}`; saved document context |
| `console.claude.ask` | `{tool, id?, prompt}`; answer and optional proposal |
| `console.claude.apply` | `{proposalId}`; new Claude-authored version |

Tool IDs are `write`, `image`, `audiovisual`, `three`. The `console` event
signals successful mutations; selection events include the updated workspace
so the UI does not rescan the Library on each caret move. The UI also emits
`wi-console-context` with `{tool, documentId, selection}` for a shell adapter.

Command-T uses the existing Claude CLI with a structured proposal schema.
Only title/text saves and unchanged-content variations are proposed in Phase
0. Unsupported requests receive an answer. The user previews and applies an
edit; asking alone never writes a document. Apply uses the proposal's original
base, refuses stale/double applications, forces Claude authorship and supports
undo. Selection and up to 16,000 characters of the saved text are sent with
the current tool/document; larger or non-text replacements are refused.
Drafts must be saved before asking. Actual Claude service access is not part
of the automated test; a fixture CLI tests the full proposal/apply path.

## Space And Readers

Prepare Space post copies the current finished asset into a local `.space`
folder with lineage and provenance in `space.json` (`wi-console-post/0.1`).
It explicitly returns `published: false`. Native placeholder JSON cannot be
posted as a finished work. Publication, shared-drawer indexing and a prompt
box mount remain integration requests, not silent fake success.

Readers use core-supplied bytes and browser-native image/audio/video playback.
Codec support follows the installed webview; 3D imports can be read as source.
Large-file processing, video scrubbing, stems, recording and glTF rendering
belong to their later phases. No audio/video/image processing library or
ffmpeg build is added here.

## Limits And Verification

Phase 0 caps imported/read assets at 24 MiB, text saves at 1 MiB, selection
JSON at 16 KiB and manifests at 16 MiB. Library search scans manifests and
the first 240 text characters, not complete documents. Histories grow
linearly and are rewritten atomically; there is no SQLite index, streaming
transport, thumbnail cache or history compaction yet. The browser receives
base64 copies for bounded previews. Multiple open documents occupy memory;
hidden audio/video readers are unmounted to stop playback.

`lucide-react` 1.52.0 is the only added dependency. Its ISC license (with
Feather-derived MIT notices) was checked before installation and retained
in `app/ui/public/licenses/lucide.txt`, copied into production output by Vite.
Rust reuses existing dependencies.

Verification commands from this worktree:

```sh
cargo test --offline -p wi-core --test console
cd app/ui
npm test
npm run build
CARGO_TARGET_DIR=/private/tmp/wi-console-target npx playwright test -c playwright.console.config.ts
```

The browser suite starts a real Rust devbridge with a disposable Library and
tests saved versions, variation lineage, hand/Claude markers, undo/redo,
read-only previews, local post packaging, draft/tool/restart persistence,
bitmap decoding, mobile framing, and the combined Focus shell. Screenshots
are written under `/private/tmp/console-*.png`. The standalone development
entry is `/console.html`; production still uses the normal app entry.
