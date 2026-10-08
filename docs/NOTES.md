# Notes in the code

Notes are markdown files in a folder, with links between them, and the
shortest path Learn could find from a notebook page to a filed note: take a
photo, share it, done. This file says where a note lives, how the folder and
the library stay the same, how a captured page is read and filed, and the
commands. `docs/HEAT.md` says how Learn's parts talk; this follows it.

## Where things live

| Part | Does |
|---|---|
| `crates/wi-heat/src/notes.rs` | The rules, with no I/O: a note's links, tags and checkboxes, the text of its file, search, what a captured page's note looks like, what Claude is asked about one |
| `crates/wi-heat-store/src/notes.rs` | The records: making, renaming and filing a note, a checkbox made a task, a captured page, suggestions, the quiet notices, and `notes` in the snapshot |
| `crates/wi-core/src/notes_cmd.rs` | The commands, and keeping the folder and the library the same |
| `crates/wi-core/src/capture.rs` | The capture inbox: watching it, reading a page, filing it |
| `crates/wi-core/src/ocr.rs`, `ocr/wi-ocr.swift` | Reading text on this Mac with Apple's Vision |
| `app/ui/src/heat/notes/` | The Notes tab |
| `tools/shortcut/` | The "Send to Wi-WWAV" Shortcut |

## A note is a file and a record

The notes folder is `Notes/` inside the library folder
(`~/Music/Wi_WWAV/Notes`). Each note is `<Title>.md`; images and PDFs are in
`Notes/attachments/`. Anything that reads markdown can open it.

```markdown
---
id: 01JABCDEF…
course: JPN 101
captured: 2026-10-07T10:22:00-04:00
---

![](attachments/2026-10-07-1022-jpn-101.jpg)

Te-form: 食べる becomes 食べて.
```

The front matter is what Learn needs to know the note again: its id, the
course or space it is filed to, when a page was captured, and `inbox: true`
while it waits to be filed. A line of front matter Learn didn't write is
kept word for word.

The same note is a `note` record (`title`, `markdown`, and beside what it
had: `courseId`, `spaceId`, `capturedAt`, `inbox`, `attachments`,
`createdAt`, `updatedAt`), so ⌘K, the MCP tools and undo work on notes as
they do on anything else. **The record is what the app writes; the file is
what the folder holds; the core makes them agree** (`notes_cmd::reconcile`):

| What happened | What the core does |
|---|---|
| The record changed (an edit, an undo, Claude's `add_note`) | writes the file; renames it when the title changed |
| The file changed outside the app | writes it into the record, as one entry, "notes from disk" |
| Both changed | the file wins; the app's version is one ⌘Z away |
| A file appeared | makes a note for it, titled by its name |
| A file was deleted | deletes the note ("notes from disk"; undo puts both back) |
| The record was deleted | moves the file to `Notes/.trash/` |

It runs after every note command, when another process or an undo changes a
note, and every few seconds for the folder. So a note made any other way
(`heat.put {kind: "note"}`, the Database tab, Claude's `add_note`) has its
file a moment later, with nothing more to call.

`Config::notes_dir` names the folder; the app and the dev bridge name
`Notes` in the library folder. With none, notes are records only: a test's
core writes no files unless it asks to.

**What syncs is as it was.** A `note` record syncs to the person's account
when they are signed in, as it did before Notes, because a public note has
to go up to be shown. A captured page's text is in its note, so it syncs
with it; its image never does. `noteFile` records (outside the
journal) hold what was last seen of each file, so an unchanged file is never
read twice.

**A title is a note's name everywhere**: its file's name, and what
`[[Title]]` finds. No two notes share one. Renaming a note rewrites every
link to it in the same entry.

**The daily note** is the note titled with its day, `2026-10-07`. `dailyNote`
records from before are moved over once, as one entry ("move daily notes
into Notes"); nothing writes that kind after.

## Links, tags, checkboxes

- `[[Title]]`, `[[Title|shown]]`, `[[Title#heading]]`. A link finds a note
  by its title, else a course by its code (`[[JPN 101]]`), else a task by
  its title; else it is `missing`, and the Notes tab makes the note when the
  link is clicked or picked.
- `#tag`, anywhere but in code. A heading isn't one.
- `- [ ] text` is a checkbox. `heat.note.taskFromLine` makes it a task in
  the note's space and course, and writes the task's id at the end of the
  line as `^t-<id>`, which the views hide. The task carries `noteId`. From
  then on the box shows the task's own state, and ticking it marks the task.

Nothing inside backticks or a fenced block is a link, a tag or a checkbox.

## The capture inbox

The person's side: take a photo of a notebook page, Share, "Send to
Wi-WWAV". On the Mac: drop a file on the Notes tab, paste an image, or put a
file in the folder.

**The folders.** `Config::capture_inboxes`; the app watches two:

- `iCloud Drive/Wi-WWAV Inbox`, for the Mac: drag and drop, AirDrop,
  Continuity Camera;
- `iCloud Drive/Shortcuts/Wi-WWAV Inbox`, where the Shortcut saves. A
  Shortcut that is shared as a file can only save inside the Shortcuts
  folder without asking where each time.

**The Shortcut** (`tools/shortcut/`) shows in the share sheet for images and
PDFs and saves each one as `<date> <time> <name>`, so the time it was shared
is in the name. The time the photo was *taken* is in the file itself.

**For each new image or PDF**, once it has stopped changing:

1. **When.** The photo's own date (EXIF), else the date in its name, else
   the file's.
2. **Read.** `wi-ocr` reads it with Vision, accurate level, English and
   Japanese; a PDF's pages are drawn and read one by one, and each page
   becomes a JPEG.
3. **Where.** If it was taken during a class or within 30 minutes after, it
   is that class's (`commitments::class_at`), and it is titled
   `JPN 101 · Oct 7`.
4. **Claude, once, and only if allowed** (`Config::capture_claude`, and the
   switch in the guide). When Vision read it poorly (confidence under 0.5,
   under four words, or Japanese) Claude reads the page itself; when no class
   placed it Claude picks a course, a space or a note to link from the
   person's own lists; and either way it lists tasks that sound due and key
   terms. One run does all three.
5. **Write.** The note is made in the Notes inbox, one entry, "capture"; if
   it was placed, a second entry files it, "file note". The original moves
   from the inbox to `attachments/`.
6. **Say so.** One quiet notice, "Filed to JPN 101 · Oct 7", with Undo. Undo
   takes back the filing: the page is still there, in the Notes inbox.

With nowhere to put it, it stays in the Notes inbox, to be filed with one
click. What Claude suggested (`noteSuggestion`) is shown under the note and
never applied: a suggested task becomes a task only when it is clicked.

**The image is the page. The text is what makes it searchable.** Reading
and filing happen on this Mac; the one thing that goes out is the run of
Claude in step 4, which can be switched off. (The note itself syncs as any
note does: see "What syncs is as it was" above.)

A class places a page only when it names a course: a class commitment with
no course has nowhere to file to.

### Reading on this Mac

`wi-ocr <file> <folder>` prints one JSON object:

```jsonc
{ "taken": "2026-10-07T10:22:31", "offset": "-04:00",   // from EXIF, when the file has it
  "pages": [ { "text": "…", "confidence": 0.93, "image": "/…/page-1.jpg" } ] }
```

It is a small Swift program (`crates/wi-core/src/ocr/wi-ocr.swift`). The core
uses `Config::ocr`, else `WI_WWAV_OCR`, else `wi-ocr` in the app's
`Contents/Helpers` or beside its binary, else it builds it once with `swiftc`
into the library's `cache/` folder. With none of those, and Claude off, a
captured page is still filed: image on top, no text yet.

## What the snapshot adds

`records.note` as before, whole, and:

```jsonc
"notes": {
  "folder": "/Users/…/Wi_WWAV/Notes",
  "index": { "<noteId>": {
      "title": "JPN 101 · Oct 7", "excerpt": "Te-form: 食べる becomes 食べて.", "tags": ["grammar"],
      "links": [ { "target": "Verb groups", "shown": null, "kind": "note|course|task|missing", "id": "…", "line": 2 } ],
      "backlinks": [ { "id": "…", "title": "Te-form drills", "line": "See JPN 101 · Oct 7 for the rule." } ],
      "boxes": [ { "line": 4, "text": "Do worksheet 4", "done": false, "taskId": null } ],
      "updatedAt": 0, "label": "JPN 101 · Beginning Japanese I", "hue": 210 } },
  "order": ["<noteId>"],                                  // newest first
  "tags": [ { "tag": "grammar", "count": 3 } ],
  "inbox": ["<noteId>"],                                  // captured, waiting to be filed
  "suggestions": { "<noteId>": { "tasks": [ { "title": "Study for the te-form quiz", "due": "2026-10-09", "taskId": null } ],
                                 "terms": ["te-form", "group one verbs"] } },
  "daily": "<noteId>",                                    // today's daily note, or null
  "capture": { "folders": ["/…/Wi-WWAV Inbox"], "watching": true, "waiting": 0,
               "doing": "Reading IMG_2211.jpg", "claude": true, "reader": "ready|building|missing" }
},
"notices": [ { "id": "filed:…", "kind": "filed|leave|exception|capture", "text": "Filed to JPN 101 · Oct 7",
               "at": 0, "noteId": "…", "undo": { "txnId": "…" },
               "act": { "label": "Skip it", "cmd": "heat.commitment.exception.confirm", "args": { "id": "…" } } } ]
```

A box that is a task carries `taskId`, and its `done` is the task's.

## The commands

| cmd | args | result |
|---|---|---|
| `heat.note.create` | `{title?, markdown?, courseId? \| course?, spaceId? \| space?, ifMissing?}` | `{note, undo}`. A title another note has is refused ("A note is already called …"), unless `ifMissing`, which answers that note with `existed: true`. "add note" |
| `heat.note.save` | `{id, markdown?, title?}` | `{note, renamed, undo}` (`renamed`: how many notes had links rewritten). "edit note", "rename note" |
| `heat.note.daily` | `{date, markdown?}` | `{note}`; with `markdown` it is written, and made if it isn't there |
| `heat.note.search` | `{q, limit?}` | `{hits: [{id, title, snippet, score}]}`: notes that hold every word, a title match first |
| `heat.note.link` | `{id, to}` | `{note, created, line, undo}`. Adds `[[to]]`; a name nothing has becomes a new, empty note. "link note" |
| `heat.note.file` | `{id, courseId? \| course? \| spaceId? \| space? \| none?}` | `{note, line, undo}` ("Filed to EGR 101"). Takes it out of the Notes inbox. "file note" |
| `heat.note.taskFromLine` | `{id, line}` | `{task, note, undo}`. "task from note" |
| `heat.note.toggleBox` | `{id, line, done}` | `{note, task, undo}`. "check box", "uncheck box" |
| `heat.note.suggestion.accept` | `{noteId, index}` | `{task, undo}`. "add task" |
| `heat.note.suggestion.dismiss` | `{noteId}` | `{}` |
| `heat.note.attachment` | `{path}` (as the note writes it: `attachments/…`) | `{dataUrl}`: the image, for a view that can't open files |
| `heat.note.reveal` | `{id?}` | `{path}`: the note's file, or the folder |
| `heat.note.sync` | `{}` | `{changed}`: makes the folder and the library agree now |
| `heat.delete` | `{kind: "note", id}` | as any record; its file goes to `Notes/.trash/` |
| `heat.capture.process` | `{path?, wait?}` | with `path`, that one file (left where it is unless it is in an inbox); without, everything waiting. `{started, waiting}`, or with `wait` `{notes: [{id, title, line}]}` once it is done |
| `heat.capture.inbox.add` | `{paths}` or `{name, base64}` | `{added}`: copies files, or a pasted image, into the inbox |
| `heat.capture.settings.set` | `{claude}` | `{}`: whether a captured page may be sent to Claude |
| `heat.capture.shortcut` | `{}` | `{path, inICloud}`: writes the "Send to Wi-WWAV" Shortcut (the app carries it) into the capture folder in iCloud Drive, or the notes folder with no iCloud, and shows it in the Finder |
| `heat.notice.dismiss` | `{id}` | `{}` |
| `heat.tools.list` | `{}` | `{tools: [{name, mcpName, effect, doing, description, inputSchema}]}` |
| `heat.tools.call` | `{name, args, stage?}` | a tool that reads or drafts: `{cmd, line, result}`. One that changes: the same, or with `stage` only `{staged: {cmd, args, line}}`, checked and not written |

## The tools for ⌘K

`crates/wi-core/src/learn_tools.rs` lists the twelve tools the prompt box
mounts, each a command above: `commitment.create`, `commitment.update`,
`commitment.add_exception`, `commitment.import_text`,
`commitment.import_image`, `commitment.import_ics`, `planner.free_time`,
`note.create`, `note.search`, `note.link`, `note.file`, `capture.process`.
`learn_tools::stage` answers the command, its args and the preview's line
without writing anything; the imports only ever make a draft.

## Tests

`crates/wi-heat/tests/notes.rs` holds the rules. `crates/wi-core/tests/core/notes.rs`
goes through the commands and the folder: a note is a file the moment it is
made, an edit outside the app reaches the library and undoes, a rename
follows every link, a checkbox becomes a task and stays linked.
`crates/wi-core/tests/core/capture.rs` drops a photo in an inbox with a class
on, against stand-ins for `wi-ocr` and the command line: it is filed to the
class as `JPN 101 · Oct 7`, image on top, text below, found by a word from
the page, and one undo puts it back in the inbox.
