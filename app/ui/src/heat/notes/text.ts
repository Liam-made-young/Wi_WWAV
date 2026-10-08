// What a note's text holds, read without drawing it (docs/NOTES.md): its
// blocks, its links, its tags and its checkboxes. Nothing inside backticks
// or a fenced block is a link, a tag or a checkbox. The editor splits a note
// into blocks with this; the fake core fills its index with it. The real
// core has its own rules (crates/wi-heat/src/notes.rs), and the views read
// its index, never this, for anything the snapshot already says.
//
// A note is blocks split on blank lines; a fenced block is one block, blank
// lines and all. A block remembers how many blank lines stood before it, so
// a note that is split and joined again is the same text.

/** One block of a note: its text, and how many blank lines stood before it. */
export interface Block {
  text: string;
  gap: number;
}

const FENCE = /^\s{0,3}(`{3,}|~{3,})/;

/** Whether `line` closes a fence opened with `fence`. */
function closes(line: string, fence: string): boolean {
  const m = /^\s{0,3}(`{3,}|~{3,})\s*$/.exec(line);
  return !!m && m[1][0] === fence[0] && m[1].length >= fence.length;
}

/** The note's blocks, each with the line it starts on. */
export function splitBlocks(markdown: string): (Block & { line: number })[] {
  const lines = markdown.split('\n');
  const blocks: (Block & { line: number })[] = [];
  let start = -1;
  let gap = 0;
  let blank = 0;
  let fence: string | null = null;
  const close = (end: number) => {
    if (start >= 0) blocks.push({ text: lines.slice(start, end).join('\n'), gap, line: start });
    start = -1;
  };
  lines.forEach((line, i) => {
    if (fence) {
      if (closes(line, fence)) fence = null;
      return;
    }
    if (line.trim() === '') {
      close(i);
      blank += 1;
      return;
    }
    if (start < 0) {
      start = i;
      gap = blank;
      blank = 0;
    }
    const opened = FENCE.exec(line);
    if (opened) fence = opened[1];
  });
  close(lines.length);
  return blocks;
}

/** The newlines after the last block, kept so a note ends as it did. */
export function tailOf(markdown: string): string {
  const blocks = splitBlocks(markdown);
  const last = blocks.at(-1);
  if (!last) return '';
  const end = last.line + last.text.split('\n').length;
  return '\n'.repeat(Math.max(0, markdown.split('\n').length - end));
}

const body = (text: string) => text.replace(/\n+$/, '');
const isEmpty = (block: Block) => block.text.trim() === '';

/**
 * The blocks as one text again. A block with nothing in it leaves no trace, and the blank lines the note
 * starts with are the first block's, whether or not it still has words.
 */
export function joinBlocks(blocks: Block[], tail = ''): string {
  let out = '';
  let first = true;
  for (const b of blocks) {
    if (isEmpty(b)) continue;
    out += first ? '\n'.repeat(blocks[0].gap) : '\n'.repeat(Math.max(1, b.gap) + 1);
    out += body(b.text);
    first = false;
  }
  return first ? '' : out + tail;
}

/** The line of the whole note each block starts on, from 0. */
export function firstLines(blocks: Block[]): number[] {
  const lines: number[] = [];
  let next = 0;
  let first = true;
  for (const b of blocks) {
    const at = next + (first ? blocks[0].gap : Math.max(1, b.gap));
    lines.push(at);
    if (isEmpty(b)) continue;
    next = at + body(b.text).split('\n').length;
    first = false;
  }
  return lines;
}

/** A note's text as splitting and joining leaves it: what the editor compares against. */
export function canon(markdown: string): string {
  return joinBlocks(splitBlocks(markdown), tailOf(markdown));
}

/** Whether the text ends inside a fence nothing has closed yet. */
export function inOpenFence(text: string): boolean {
  let fence: string | null = null;
  for (const line of text.split('\n')) {
    if (fence) {
      if (closes(line, fence)) fence = null;
    } else {
      const opened = FENCE.exec(line);
      if (opened) fence = opened[1];
    }
  }
  return fence !== null;
}

// --- links, tags and checkboxes ----------------------------------------------

/** The id a checkbox line carries once it is a task: `^t-<id>` at its end, which no view shows. */
export const MARKER = /\s*\^t-([A-Za-z0-9_-]+)\s*$/;

export const LIST_ITEM = /^(\s*)([-*+]|\d+[.)])(\s+)(?:\[([ xX])\](?:\s+|$))?(.*)$/;

/** Each line with what is code blanked out, so a rule that reads it finds nothing there. */
export function plainLines(markdown: string): string[] {
  let fence: string | null = null;
  return markdown.split('\n').map((line) => {
    if (fence) {
      if (closes(line, fence)) fence = null;
      return '';
    }
    const opened = FENCE.exec(line);
    if (opened) {
      fence = opened[1];
      return '';
    }
    return line.replace(/(`+)[^`]*?\1/g, (code) => ' '.repeat(code.length));
  });
}

const WIKILINK = /\[\[([^[\]|]+)(?:\|([^[\]]+))?\]\]/g;

/** `Title#heading` names the note `Title`. */
export const targetOf = (written: string) => written.split('#')[0].trim();

export function linksIn(markdown: string): { target: string; shown: string | null; line: number }[] {
  return plainLines(markdown).flatMap((text, line) =>
    [...text.matchAll(WIKILINK)]
      .map((m) => ({ target: targetOf(m[1]), shown: m[2]?.trim() ?? null, line }))
      .filter((l) => l.target !== ''),
  );
}

/** A tag is `#` and a word with a letter in it, not hard against a word before it. */
export const TAG = /(^|[^\p{L}\p{N}_#&/])#([\p{L}\p{N}_/-]*\p{L}[\p{L}\p{N}_/-]*)/gu;

export function tagsIn(markdown: string): string[] {
  const tags = new Set<string>();
  for (const text of plainLines(markdown)) {
    // A link's `#heading` and an address's `#part` are not tags.
    const bare = text.replace(WIKILINK, ' ').replace(/\]\([^)]*\)/g, ' ').replace(/https?:\/\/\S+/g, ' ');
    for (const m of bare.matchAll(TAG)) tags.add(m[2]);
  }
  return [...tags];
}

export function boxesIn(markdown: string): { line: number; text: string; done: boolean; taskId: string | null }[] {
  const raw = markdown.split('\n');
  return plainLines(markdown).flatMap((text, line) => {
    if (!LIST_ITEM.exec(text)?.[4]) return [];
    const m = LIST_ITEM.exec(raw[line])!;
    const marker = MARKER.exec(m[5]);
    return [{ line, text: m[5].replace(MARKER, ''), done: m[4] !== ' ', taskId: marker?.[1] ?? null }];
  });
}

/** A line as plain words: no markers, no brackets, no stars. */
export function plainText(line: string): string {
  return line
    .replace(MARKER, '')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
    .replace(/\[\[([^[\]|]+)\|([^[\]]+)\]\]/g, '$2')
    .replace(/\[\[([^[\]]+)\]\]/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/^\s*(#{1,6}\s+|>\s?|(?:[-*+]|\d+[.)])\s+(?:\[[ xX]\]\s+)?)/, '')
    .replace(/(\*\*|__|~~|`)/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

/** The first words of a note, for its row in the list. */
export function excerptOf(markdown: string, most = 140): string {
  let out = '';
  for (const line of plainLines(markdown)) {
    // A table's row reads as its cells, and its rule as nothing.
    const words = plainText(line).replace(/\s*\|\s*/g, ' ').trim();
    if (!words || /^[-*_:\s]+$/.test(words)) continue;
    out = out ? `${out} ${words}` : words;
    if (out.length >= most) break;
  }
  return out.length > most ? `${out.slice(0, most - 1).trimEnd()}…` : out;
}
