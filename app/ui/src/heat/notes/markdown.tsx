// A note's markdown, drawn as React nodes (docs/NOTES.md). It is Learn's
// own small reader, with no HTML in or out: nothing a note holds is ever
// handed to the page as markup, so a note can't run anything.
//
//   # to ######      headings              **bold** *italic* ~~strike~~ `code`
//   ``` … ```        a fenced block        > a quote
//   - · 1.           lists, nested         - [ ] · - [x]   checkboxes
//   | a | b |        a table, with its rule under the heading row
//   ---              a rule                [text](url) and bare addresses
//   ![alt](path)     an image; ![alt](<a path with spaces>)
//   [[Title]] · [[Title|shown]] · [[Title#heading]]         #tag
//
// Nothing inside code is a link or a tag. `^t-<id>` at the end of a checkbox
// line is the task it became, and is never shown. What a link finds, what a
// click does and how an image loads come from the caller, as `Hooks`; with
// none, the text still reads.

import { Fragment, type ReactNode } from 'react';
import { openExternal } from '../external';
import { LIST_ITEM, MARKER, plainText, targetOf } from './text';

/** What a `[[link]]` finds: a note, else a course, else a task, else nothing yet. */
export interface Linked {
  kind: 'note' | 'course' | 'task' | 'missing';
  id: string | null;
}

export interface Hooks {
  /** What `[[target]]` finds. Without it every link reads as missing. */
  resolve?(target: string): Linked;
  onLink?(target: string, to: Linked): void;
  /** A click on `#tag`. Without it a tag is only a chip. */
  onTag?(tag: string): void;
  /** A checkbox on `line` of the whole note: its state when a task holds it, and what sits beside it. */
  box?(line: number, written: boolean, text: string): { done?: boolean; beside?: ReactNode } | undefined;
  onBox?(line: number, done: boolean): void;
  /** An image, drawn by the caller: one in the notes folder has to be asked for. */
  image?(src: string, alt: string): ReactNode;
}

interface Ctx {
  hooks: Hooks;
  /** The line of the whole note the text starts on. */
  base: number;
}

// --- inline ---------------------------------------------------------------------

const TOKEN = [
  /\\(?<esc>[\\`*_{}[\]()#+\-.!~|>])/,
  /(?<tick>`+)(?<code>[^`](?:.*?[^`])?)\k<tick>(?!`)/,
  /!\[(?<alt>[^\]]*)\]\((?:<(?<imgWide>[^>]+)>|(?<img>[^)\s]+))(?:\s+"[^"]*")?\)/,
  /\[\[(?<wiki>[^[\]|]+)(?:\|(?<shown>[^[\]]+))?\]\]/,
  /\[(?<text>[^\]]+)\]\((?:<(?<hrefWide>[^>]+)>|(?<href>[^)\s]+))(?:\s+"[^"]*")?\)/,
  /(?<url>https?:\/\/[^\s<>"')\]]*[^\s<>"')\].,;:!?])/,
  /\*\*\*(?<both>\S(?:.*?\S)?)\*\*\*/,
  /\*\*(?<bold>\S(?:.*?\S)?)\*\*/,
  /__(?<bold2>\S(?:.*?\S)?)__/,
  /~~(?<strike>\S(?:.*?\S)?)~~/,
  /\*(?<em>[^\s*](?:[^*]*[^\s*])?)\*/,
  /_(?<em2>[^\s_](?:[^_]*[^\s_])?)_/,
  /#(?<tag>[\p{L}\p{N}_/-]*\p{L}[\p{L}\p{N}_/-]*)/u,
]
  .map((r) => r.source)
  .join('|');

const WORD = /[\p{L}\p{N}]/u;
const isWeb = (url: string) => /^https?:\/\//i.test(url);

function Address({ url, children }: { url: string; children: ReactNode }) {
  // Only a web address opens, and in the person's own browser; any other is the words alone.
  if (!isWeb(url)) {
    return (
      <span className="notes-link" data-dead title={url}>
        {children}
      </span>
    );
  }
  return (
    <a
      className="notes-link"
      href={url}
      onClick={(e) => {
        e.preventDefault();
        openExternal(url);
      }}
    >
      {children}
    </a>
  );
}

/** One line's words, with what is marked in them drawn. */
function inline(text: string, ctx: Ctx): ReactNode[] {
  const out: ReactNode[] = [];
  const token = new RegExp(TOKEN, 'gu');
  let at = 0;
  let n = 0;
  const put = (node: ReactNode) => out.push(<Fragment key={n++}>{node}</Fragment>);
  for (let m = token.exec(text); m; m = token.exec(text)) {
    const g = m.groups!;
    const before = m.index > 0 ? text[m.index - 1] : '';
    const after = text[m.index + m[0].length] ?? '';
    // An underscore inside a word is a letter, and so is a `#` hard against one.
    const inWord = (g.em2 !== undefined || g.bold2 !== undefined) && (WORD.test(before) || WORD.test(after));
    const notTag = g.tag !== undefined && (WORD.test(before) || '#&/_'.includes(before || ' '));
    if (inWord || notTag) {
      token.lastIndex = m.index + 1;
      continue;
    }
    if (m.index > at) put(text.slice(at, m.index));
    at = m.index + m[0].length;

    if (g.esc !== undefined) put(g.esc);
    else if (g.code !== undefined) put(<code className="notes-code-inline">{g.code}</code>);
    else if (g.img !== undefined || g.imgWide !== undefined) {
      const src = g.imgWide ?? g.img;
      put(ctx.hooks.image ? ctx.hooks.image(src, g.alt) : <img className="notes-img" src={isWeb(src) || src.startsWith('data:') ? src : undefined} alt={g.alt} />);
    } else if (g.wiki !== undefined) {
      const target = targetOf(g.wiki);
      const to = ctx.hooks.resolve?.(target) ?? { kind: 'missing', id: null };
      const { onLink } = ctx.hooks;
      put(
        <button
          type="button"
          className="notes-wikilink"
          data-dense
          data-kind={to.kind}
          title={to.kind === 'missing' ? `Make the note “${target}”` : undefined}
          onClick={onLink && (() => onLink(target, to))}
        >
          {g.shown?.trim() ?? g.wiki.trim()}
        </button>,
      );
    } else if (g.text !== undefined) put(<Address url={g.hrefWide ?? g.href}>{inline(g.text, ctx)}</Address>);
    else if (g.url !== undefined) put(<Address url={g.url}>{g.url}</Address>);
    else if (g.both !== undefined) {
      put(
        <strong>
          <em>{inline(g.both, ctx)}</em>
        </strong>,
      );
    } else if (g.bold !== undefined || g.bold2 !== undefined) put(<strong>{inline(g.bold ?? g.bold2, ctx)}</strong>);
    else if (g.strike !== undefined) put(<del>{inline(g.strike, ctx)}</del>);
    else if (g.em !== undefined || g.em2 !== undefined) put(<em>{inline(g.em ?? g.em2, ctx)}</em>);
    else if (g.tag !== undefined) {
      const { onTag } = ctx.hooks;
      const tag = g.tag;
      put(
        onTag ? (
          <button type="button" className="notes-tag" data-dense onClick={() => onTag(tag)}>
            #{tag}
          </button>
        ) : (
          <span className="notes-tag">#{tag}</span>
        ),
      );
    }
  }
  if (at < text.length) put(text.slice(at));
  return out;
}

/** Lines one under another, as they were written. */
function lines(texts: string[], ctx: Ctx): ReactNode[] {
  return texts.map((t, i) => (
    <Fragment key={i}>
      {i > 0 && <br />}
      {inline(t, ctx)}
    </Fragment>
  ));
}

// --- blocks ---------------------------------------------------------------------

const FENCE = /^\s{0,3}(`{3,}|~{3,})\s*([^\s`]*)\s*$/;
const HEADING = /^\s{0,3}(#{1,6})\s+(.*?)(?:\s+#+)?\s*$/;
const RULE = /^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/;
const QUOTE = /^\s{0,3}>\s?/;
const TABLE_RULE = /^\s*\|?\s*:?-+:?\s*(?:\|\s*:?-+:?\s*)*\|?\s*$/;

const isItem = (line: string) => LIST_ITEM.test(line) && !RULE.test(line);
/** A table is a row of cells with its rule under it, a dash for each cell. */
const isTable = (rows: string[], i: number) =>
  rows[i].includes('|') &&
  i + 1 < rows.length &&
  TABLE_RULE.test(rows[i + 1]) &&
  cells(rows[i + 1]).length === cells(rows[i]).length;
/** Whether a paragraph ends before this line: something else starts on it. */
const starts = (rows: string[], i: number) =>
  FENCE.test(rows[i]) || HEADING.test(rows[i]) || RULE.test(rows[i]) || QUOTE.test(rows[i]) || isItem(rows[i]) || isTable(rows, i);

/** A table row's cells: split on the pipes that aren't escaped or inside code. */
function cells(row: string): string[] {
  const out: string[] = [];
  let cell = '';
  let code = false;
  const text = row.trim().replace(/^\|/, '').replace(/\|$/, '');
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch === '\\' && text[i + 1] === '|') {
      cell += '|';
      i += 1;
    } else if (ch === '|' && !code) {
      out.push(cell.trim());
      cell = '';
    } else {
      if (ch === '`') code = !code;
      cell += ch;
    }
  }
  out.push(cell.trim());
  return out;
}

const indentOf = (space: string) => space.replace(/\t/g, '    ').length;

interface Item {
  indent: number;
  ordered: boolean;
  start: number;
  box: boolean | null;
  texts: string[];
  /** Its line within the text being drawn. */
  line: number;
  children: Item[];
}

/** A run of list lines as items, each holding the items indented under it. */
function items(rows: string[], from: number): Item[] {
  const flat: Item[] = [];
  rows.forEach((row, i) => {
    const m = isItem(row) ? LIST_ITEM.exec(row) : null;
    if (!m) {
      // A line indented under an item goes on with its words.
      flat.at(-1)?.texts.push(row.trim());
      return;
    }
    flat.push({
      indent: indentOf(m[1]),
      ordered: /\d/.test(m[2]),
      start: Number.parseInt(m[2], 10) || 1,
      box: m[4] === undefined ? null : m[4] !== ' ',
      texts: [m[5]],
      line: from + i,
      children: [],
    });
  });
  const top: Item[] = [];
  const open: Item[] = [];
  for (const item of flat) {
    while (open.length > 0 && open.at(-1)!.indent >= item.indent) open.pop();
    (open.at(-1)?.children ?? top).push(item);
    open.push(item);
  }
  return top;
}

function list(all: Item[], ctx: Ctx): ReactNode[] {
  // A bulleted run and a numbered run under one another are two lists.
  const runs: Item[][] = [];
  for (const item of all) {
    if (runs.at(-1)?.[0].ordered === item.ordered) runs.at(-1)!.push(item);
    else runs.push([item]);
  }
  return runs.map((run) => {
    const List = run[0].ordered ? 'ol' : 'ul';
    return (
      <List
        key={run[0].line}
        className="notes-items"
        data-line={run[0].line}
        start={run[0].ordered && run[0].start !== 1 ? run[0].start : undefined}
      >
        {run.map((item) => {
          const whole = ctx.base + item.line;
          const text = item.box === null ? item.texts[0] : item.texts[0].replace(MARKER, '');
          const held = item.box === null ? undefined : ctx.hooks.box?.(whole, item.box, item.texts[0]);
          const { onBox } = ctx.hooks;
          return (
            <li key={item.line} data-line={item.line} data-box={item.box === null ? undefined : ''}>
              {item.box !== null && (
                <input
                  type="checkbox"
                  className="notes-box"
                  aria-label={plainText(text) || 'Checkbox'}
                  checked={held?.done ?? item.box}
                  disabled={!onBox}
                  onChange={(e) => onBox?.(whole, e.target.checked)}
                />
              )}
              <span className="notes-item">{lines([text, ...item.texts.slice(1)], ctx)}</span>
              {held?.beside}
              {item.children.length > 0 && list(item.children, ctx)}
            </li>
          );
        })}
      </List>
    );
  });
}

/** The lines from `from` on, as headings, paragraphs, lists and the rest. `from` is their place in the text being drawn. */
function blocks(rows: string[], from: number, ctx: Ctx): ReactNode[] {
  const out: ReactNode[] = [];
  let i = 0;
  while (i < rows.length) {
    const row = rows[i];
    const at = from + i;
    if (row.trim() === '') {
      i += 1;
      continue;
    }
    const fence = FENCE.exec(row);
    if (fence) {
      let end = i + 1;
      const closing = new RegExp(`^\\s{0,3}${fence[1][0]}{${fence[1].length},}\\s*$`);
      while (end < rows.length && !closing.test(rows[end])) end += 1;
      out.push(
        <pre key={at} className="notes-code" data-line={at}>
          <code data-lang={fence[2] || undefined}>{rows.slice(i + 1, end).join('\n')}</code>
        </pre>,
      );
      i = end + 1;
      continue;
    }
    const heading = HEADING.exec(row);
    if (heading) {
      const H = `h${heading[1].length}` as 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6';
      out.push(
        <H key={at} className="notes-h" data-line={at}>
          {inline(heading[2], ctx)}
        </H>,
      );
      i += 1;
      continue;
    }
    if (RULE.test(row)) {
      out.push(<hr key={at} className="notes-rule" data-line={at} />);
      i += 1;
      continue;
    }
    if (QUOTE.test(row)) {
      let end = i;
      while (end < rows.length && QUOTE.test(rows[end])) end += 1;
      out.push(
        <blockquote key={at} className="notes-quote" data-line={at}>
          {blocks(
            rows.slice(i, end).map((r) => r.replace(QUOTE, '')),
            at,
            ctx,
          )}
        </blockquote>,
      );
      i = end;
      continue;
    }
    if (isTable(rows, i)) {
      let end = i + 2;
      while (end < rows.length && rows[end].includes('|') && rows[end].trim() !== '') end += 1;
      const align = cells(rows[i + 1]).map((c) =>
        c.startsWith(':') && c.endsWith(':') ? 'center' : c.endsWith(':') ? 'right' : undefined,
      );
      const head = cells(row);
      out.push(
        <table key={at} className="notes-table" data-line={at}>
          <thead>
            <tr>
              {head.map((c, n) => (
                <th key={n} scope="col" style={align[n] ? { textAlign: align[n] } : undefined}>
                  {inline(c, ctx)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.slice(i + 2, end).map((r, k) => (
              <tr key={k} data-line={at + 2 + k}>
                {head.map((_, n) => (
                  <td key={n} style={align[n] ? { textAlign: align[n] } : undefined}>
                    {inline(cells(r)[n] ?? '', ctx)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>,
      );
      i = end;
      continue;
    }
    if (isItem(row)) {
      let end = i + 1;
      // The list goes on through its items and the lines indented under them.
      while (end < rows.length && rows[end].trim() !== '' && (isItem(rows[end]) || /^\s/.test(rows[end]))) end += 1;
      out.push(...list(items(rows.slice(i, end), at), ctx));
      i = end;
      continue;
    }
    let end = i + 1;
    while (end < rows.length && rows[end].trim() !== '' && !starts(rows, end)) end += 1;
    out.push(
      <p key={at} className="notes-p" data-line={at}>
        {lines(
          rows.slice(i, end).map((r) => r.trim()),
          ctx,
        )}
      </p>,
    );
    i = end;
  }
  return out;
}

/** The text as React nodes. `base` is the line of the whole note it starts on, for its checkboxes. */
export function renderMarkdown(text: string, hooks: Hooks = {}, base = 0): ReactNode[] {
  return blocks(text.split('\n'), 0, { hooks, base });
}

export function Markdown({ text, hooks, base = 0 }: { text: string; hooks?: Hooks; base?: number }) {
  return <>{renderMarkdown(text, hooks, base)}</>;
}
