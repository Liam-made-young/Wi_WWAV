# Console

Phase 0 supplies the four-tool shell and the shared local Library; Phase 1
adds Write as a standalone writing tool. Write,
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
Write has a CodeMirror Markdown/Fountain editor, a section binder and outline,
screenplay and lyrics modes, notes/research, exports, and scoped Claude edits.
The other native documents still have empty canvases. Imported text, images,
audio and video have read-only readers. Brushes, a timeline, modeling,
rendering and media export belong to later phases.

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
placeholders, not finished export formats. Write now has a Markdown
section/manifest bundle; Console owns the future Image JSON/PNG/SVG layer bundle, with
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
The legacy generic entry proposes title/text saves and unchanged-content
variations. Write's entry additionally supports section-scoped proposals
(see below). Unsupported requests receive an answer. The user previews and applies an
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
Write readers render all saved sections as Markdown or screenplay formatting,
without revealing private notes/research. Read-only text above Write's editing
limit is shown as source. Space packages contain the finished text plus the
edit ledger, never manuscript research snapshots or private notes assets.
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

Phase 0 added `lucide-react` 1.52.0. Its ISC license (with
Feather-derived MIT notices) was checked before installation and retained
in `app/ui/public/licenses/lucide.txt`, copied into production output by Vite.
Write's dependencies and their retained notices are listed below.

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

## Write (Phase 1)

### Working Surface

The editable page is Markdown source with live syntax styling and a toggle
for Rust-rendered preview. Preview follows unsaved changes, escapes embedded
HTML, allows HTTP(S)/mailto links, and never fetches embedded images. Focus
mode removes the binder, research and Library and dims non-current lines.
Typewriter scrolling centers the cursor during typing. View preferences are
saved independently for each document. Drafts and the selected section remain
in memory across tool and room switches; only saved versions survive a crash.

A manuscript contains 1-256 sections in preorder. Drag a section before a
peer, or use the up/down buttons. Nest/unnest and a parent selector move the
entire subtree. There are at most eight levels. A section has a title, type,
synopsis and body. Outline mode shows synopses and clickable Markdown headings.
Word counts come from Rust for each section and the whole manuscript.

Screenplay mode edits Fountain source and renders scene headings, action,
character cues, dialogue, parentheticals and transitions. The element selector
formats the current line through Rust. Fountain imports retain source bytes;
exports preserve that source, joined in binder order. The existing `fountain`
parser handles the grammar; a narrow adapter resumes consecutive lines and
supports forced scene/action/character markers and continued dialogue.
This is the standard six-element subset, not full Fountain conformance:
dual-dialogue columns, revision marks, boneyards and scene-number management
are not implemented.

Lyrics use verse, hook, bridge, intro and outro section types. Rust counts
nonblank lines and displays per-line words and **estimated English syllables**.
These are vowel-group estimates, not pronunciation-dictionary counts; names,
dialect and non-English lyrics need the writer's judgment.

Notes and manual research sources belong to one manuscript. Learn notes are
searched through the existing `heat.note.search` API. Linking records a note
ID and a short snapshot; clicking opens the current Learn note read-only.
Deleting or changing the Learn note does not delete its saved snapshot.
Console does not modify Learn notes or navigate into a private Notes editor
using an invented API. URLs open through the shared external-browser helper.

### Manuscript Bundle

`Version.write` is optional, so Phase 0 manifests remain readable. Its format
is `wi-write/1`:

```json
{
  "format": "wi-write/1",
  "mode": "prose",
  "sections": [
    { "id": "<ULID>", "parent": null, "title": "Opening",
      "kind": "section", "synopsis": "The first image.", "asset": "<Asset>" }
  ],
  "notes": "<Asset>",
  "research": [
    { "id": "<ULID>", "title": "A source", "url": null,
      "noteId": "<Learn note ULID>", "excerpt": "A saved snapshot." }
  ]
}
```

The strings `<Asset>` above stand for the same structured asset descriptor
as the outer version: name, relative file, MIME, bytes, SHA-256 and preview.
Bodies are ordinary hashed `.md` files, with section IDs as logical names.
Private notes are another Markdown asset; research metadata is in the manifest.
The outer asset is the flattened public body (`manuscript.md` or
`script.fountain`), enabling the Library preview and Space's existing package.
All old section assets remain available to version readers. Variations copy
the whole manuscript, including notes/research, and record the exact parent.
Invalid/corrupt section assets are refused before creating a variation.

Reading a legacy single-file Write version creates a virtual first section;
reading alone never migrates or rewrites it. The next Write save records the
structured bundle as a new version. `console.save {text}` still works for a
single-section manuscript; it refuses a multi-section replacement rather than
silently destroying the binder. Use `console.write.edit` or `.save` instead.

### Commands And Claude

Every command below is in `console.tools`, callable through the same Rust
implementation as `console.tool.call` with an underscore-safe tool name.
Selections use **UTF-16 offsets**, matching CodeMirror and DOM ranges; Rust
refuses surrogate splits, stale bases, invalid trees and unknown IDs.

| Command | Arguments / result |
| --- | --- |
| `console.write.read` | `{id, version?}`; manuscript, base, per-section/total stats, view |
| `console.write.save` | `{id, base, title?, manuscript}`; new version |
| `console.write.edit` | `{id, base, action}`; new version and optional new section ID |
| `console.write.render` | `{text, mode}`; safe HTML and stats |
| `console.write.transform` | `{text, from, to, style}`; formatted source without saving |
| `console.write.view` | `{id, view}`; persisted view settings |
| `console.write.export` | `{id, version?, format}`; disk path, download name, MIME, base64, version |
| `console.write.assist` | `{id, base?, section?, selection?, intent, prompt?}`; answer/proposal |
| `console.write.research.search` | `{query?}`; existing Learn note hits |
| `console.write.research.read` | `{note}`; current local Learn note |

`action.type` is one of `add`, `update`, `move`, `delete`, `reorder`, `mode`,
`notes`, `researchAdd`, `researchRemove`, `linkNote`, `replace` or `format`.
The `Edit` type in `console/write/mod.rs` is the complete field contract.
Save records granular title/body/binder/notes/research changes. Claude tool
calls cannot forge a hand actor. Workspace view changes emit `console` events
with workspace state, avoiding Library rescans on every caret move.
The shared context entry now includes Write's mode and section IDs/hierarchy.

Command-T offers rewrite/tighten, rhymes, alternatives, summarize, reorder and
**explicit** continuation. Rewrites replace only the selection (or the chosen
section if empty). Suggestions and summaries cannot mutate a document, even
if Claude returns replacement text. Reordering is validated before presenting
it. Only the explicit continuation intent inserts new writing at the cursor.
The person previews and applies proposals; apply records a Claude version,
refuses a stale or already-consumed proposal, and is undoable. Discard does
not change the manuscript. Drafts must be saved first. Requests send the
selected text, bounded section/piece excerpts and binder context; they do not
send private document notes or research. This feature requires the existing
configured/signed-in Claude CLI; automated tests use a deterministic CLI double.

### Exports And Limits

Markdown adds section headings matching nesting; plain text removes Markdown
syntax; Fountain is offered in screenplay mode. PDF runs entirely in Rust
using `lopdf` and `rustybuzz`, with embedded IBM Plex Serif/Mono fonts,
Unicode shaping/copyable text, Letter pages, pagination, and screenplay
title page, 12pt monospaced text and standard element indents. PDF preserves
block hierarchy and content, not full rich HTML fidelity: inline emphasis,
tables, images and hyperlinks are flattened to readable text. Fonts cover
Latin and additional supported scripts, not every Unicode character. An
unsupported glyph returns an explicit error; Markdown/text remain lossless.
Screenplay dialogue crossing a page is not annotated with `(MORE)`/`(CONT'D)`.
Each export is pinned to the saved version, including when another agent edits.
It is written as an ordinary file under the Library's `exports/` directory
and offered as a browser download. Notes and research never enter finished exports.

The serialized manuscript, including notes and research, is capped at 1 MiB;
there are at most 256 sections, eight levels, 100 sources, 4,000 bytes per
synopsis and 16,000 bytes per research excerpt. Larger imported source files
(up to the shell's 24 MiB cap) remain read-only. Selection edits are capped
at 32,000 bytes; summary/context excerpts are bounded and marked truncated.
PDF is capped at 2,000 pages. History manifests retain the 16 MiB cap.
Typing undo is per section's current editor session; persisted document undo
restores whole saved versions. There is no autosave or crash-recovery log yet.

A development sample on this Mac with **20,001 words** measured 2,865 ms to
fill/render/count, 63 ms for four keystrokes and two animation frames,
451 ms to save and 385 ms to export PDF through the real bridge. This is one
sample, not a performance guarantee. Search and version manifest writes still
scale linearly. The production bundle has the existing large-chunk warning,
now approximately 1.78 MB JS (534 KB gzip), including CodeMirror.

Libraries were checked before use: CodeMirror/Lezer and their added npm
dependencies (MIT), pulldown-cmark (MIT), fountain (Unlicense OR MIT; using
Unlicense), nom (MIT), lopdf (MIT, already transitive), rustybuzz (MIT with its
retained upstream notices), and IBM Plex fonts (OFL-1.1). New Rust transitive
licenses are permissive MIT/Apache-2.0 or BSL-1.0. No GPL dependency or ffmpeg
is added. Full notices ship at `app/ui/public/licenses/write.txt`; font OFL
is also beside the embedded font files. Public notices are copied by Vite.

Phase 1 verification: 26 Console Rust tests; 948 UI tests passing and one
existing skip; typecheck/production build; seven real-core Playwright tests
covering the Phase 0 workflows plus multi-section authoring, drag/nesting,
outline, Learn research, screenplay/Fountain, lyrics, exports, Claude editing,
reorder, explicit continuation and the 20,001-word sample. Desktop/mobile
screenshots and an exported PDF raster were inspected. Scoped strict clippy
passes with only the pre-existing `homes_cmd.rs` redundant-closure warning
suppressed; dependency-wide strict clippy still finds Learn's existing
`homes.rs` type-complexity warnings. No native Tauri installation or live
Claude-service call was part of these tests.

Phase 1 is complete. Liam authorized Image (Phase 2) to begin immediately
after this phase's commit and coordination report; it remains a separate phase.
