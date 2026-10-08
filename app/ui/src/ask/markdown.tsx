// Claude's answer, drawn. The answer is Markdown; this reads the part of it
// an answer uses (paragraphs, lists, small tables, bold, italic, code and
// links) into React elements. Nothing is ever set as HTML, so nothing in an
// answer can run or load: a link is one of three kinds the box knows how to
// open, and any other address is shown as plain words.

import { Fragment, type ReactNode } from 'react';
import { type Target, targetOf } from './nav';

type Open = (target: Target, background: boolean) => void;

/** `text` with **bold**, *italic*, `code` and [links](…) drawn. */
export function inline(text: string, onOpen: Open, keyBase = ''): ReactNode[] {
  const out: ReactNode[] = [];
  let rest = text;
  let n = 0;
  const push = (node: ReactNode) => out.push(<Fragment key={`${keyBase}${n++}`}>{node}</Fragment>);
  while (rest.length > 0) {
    // The nearest mark decides what comes next.
    const marks = [
      { at: rest.indexOf('['), kind: 'link' },
      { at: rest.indexOf('**'), kind: 'bold' },
      { at: rest.indexOf('`'), kind: 'code' },
      { at: firstSingle(rest, '*'), kind: 'italic' },
      { at: firstWordEdge(rest, '_'), kind: 'under' },
    ].filter((m) => m.at >= 0);
    marks.sort((a, b) => a.at - b.at);
    const mark = marks[0];
    if (!mark) {
      push(rest);
      break;
    }
    if (mark.at > 0) push(rest.slice(0, mark.at));
    rest = rest.slice(mark.at);
    if (mark.kind === 'link') {
      const link = readLink(rest);
      if (link) {
        const target = targetOf(link.href);
        const words = inline(link.text, onOpen, `${keyBase}${n}l`);
        if (target) {
          push(
            <a
              className="ask-link"
              data-kind={target.what}
              href="#"
              title={target.what === 'web' ? target.url : target.what === 'wiki' ? `Wikipedia: ${target.title}` : undefined}
              onClick={(e) => {
                e.preventDefault();
                onOpen(target, e.metaKey || e.ctrlKey);
              }}
            >
              {words}
            </a>,
          );
        } else push(words);
        rest = rest.slice(link.length);
        continue;
      }
      push('[');
      rest = rest.slice(1);
      continue;
    }
    const fence = mark.kind === 'bold' ? '**' : mark.kind === 'code' ? '`' : mark.kind === 'under' ? '_' : '*';
    const end = rest.indexOf(fence, fence.length);
    if (end < 0 || end === fence.length) {
      push(fence);
      rest = rest.slice(fence.length);
      continue;
    }
    const inner = rest.slice(fence.length, end);
    if (mark.kind === 'code') push(<code>{inner}</code>);
    else if (mark.kind === 'bold') push(<strong>{inline(inner, onOpen, `${keyBase}${n}b`)}</strong>);
    else push(<em>{inline(inner, onOpen, `${keyBase}${n}i`)}</em>);
    rest = rest.slice(end + fence.length);
  }
  return out;
}

/** The first `*` that isn't half of `**` and has a word after it. */
function firstSingle(s: string, c: string): number {
  for (let i = s.indexOf(c); i >= 0; i = s.indexOf(c, i + 1)) {
    if (s[i + 1] === c) {
      i += 1;
      continue;
    }
    if (s[i - 1] === c) continue;
    if (s[i + 1] && s[i + 1] !== ' ') return i;
  }
  return -1;
}

/** The first `_` that opens a word, so a_b_c stays as it is. */
function firstWordEdge(s: string, c: string): number {
  for (let i = s.indexOf(c); i >= 0; i = s.indexOf(c, i + 1)) {
    const before = s[i - 1];
    if ((before === undefined || /\s|[(]/.test(before)) && s[i + 1] && s[i + 1] !== ' ' && s[i + 1] !== c) return i;
  }
  return -1;
}

/** `[text](href)` at the start of `s`. The address may hold spaces and one level of brackets. */
function readLink(s: string): { text: string; href: string; length: number } | null {
  let depth = 0;
  let close = -1;
  for (let i = 0; i < s.length; i++) {
    if (s[i] === '[') depth++;
    else if (s[i] === ']') {
      depth--;
      if (depth === 0) {
        close = i;
        break;
      }
    } else if (s[i] === '\n') return null;
  }
  if (close < 0 || s[close + 1] !== '(') return null;
  let paren = 0;
  for (let i = close + 1; i < s.length; i++) {
    if (s[i] === '(') paren++;
    else if (s[i] === ')') {
      paren--;
      if (paren === 0) {
        return { text: s.slice(1, close), href: s.slice(close + 2, i).trim(), length: i + 1 };
      }
    } else if (s[i] === '\n') return null;
  }
  return null;
}

const cellsOf = (line: string) =>
  line
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split('|')
    .map((c) => c.trim());

const isRule = (line: string) => /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/.test(line);
const bullet = /^(\s*)([-*•]|\d+[.)])\s+(.*)$/;

/** An answer's blocks, top to bottom. */
export function Markdown({ text, onOpen }: { text: string; onOpen: Open }) {
  const lines = text.replace(/\r\n/g, '\n').split('\n');
  const out: ReactNode[] = [];
  let i = 0;
  let key = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }
    if (line.trim().startsWith('```')) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith('```')) body.push(lines[i++]);
      i++;
      out.push(
        <pre key={key++} className="ask-pre">
          {body.join('\n')}
        </pre>,
      );
      continue;
    }
    if (line.includes('|') && i + 1 < lines.length && isRule(lines[i + 1])) {
      const head = cellsOf(line);
      const rows: string[][] = [];
      i += 2;
      while (i < lines.length && lines[i].includes('|') && lines[i].trim()) rows.push(cellsOf(lines[i++]));
      const k = key++;
      out.push(
        <table key={k} className="ask-table">
          <thead>
            <tr>
              {head.map((h, c) => (
                <th key={c}>{inline(h, onOpen, `${k}h${c}`)}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((r, n) => (
              <tr key={n}>
                {head.map((_, c) => (
                  <td key={c}>{inline(r[c] ?? '', onOpen, `${k}r${n}c${c}`)}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>,
      );
      continue;
    }
    const first = bullet.exec(line);
    if (first) {
      const ordered = /\d/.test(first[2]);
      const items: { depth: number; text: string }[] = [];
      while (i < lines.length) {
        const m = bullet.exec(lines[i]);
        if (m) items.push({ depth: Math.floor(m[1].length / 2), text: m[3] });
        else if (lines[i].trim() && /^\s+/.test(lines[i]) && items.length) items[items.length - 1].text += ` ${lines[i].trim()}`;
        else break;
        i++;
      }
      const k = key++;
      const List = ordered ? 'ol' : 'ul';
      out.push(
        <List key={k} className="ask-list">
          {items.map((it, n) => (
            <li key={n} style={it.depth ? { marginInlineStart: `${it.depth * 16}px` } : undefined}>
              {inline(it.text, onOpen, `${k}i${n}`)}
            </li>
          ))}
        </List>,
      );
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) {
      const k = key++;
      out.push(
        <p key={k} className="ask-heading">
          {inline(heading[2], onOpen, `${k}h`)}
        </p>,
      );
      i++;
      continue;
    }
    if (/^\s*>\s?/.test(line)) {
      const body: string[] = [];
      while (i < lines.length && /^\s*>\s?/.test(lines[i])) body.push(lines[i++].replace(/^\s*>\s?/, ''));
      const k = key++;
      out.push(
        <blockquote key={k} className="ask-quote">
          {inline(body.join(' '), onOpen, `${k}q`)}
        </blockquote>,
      );
      continue;
    }
    // A paragraph: lines up to a blank one or the start of another kind of block.
    const para: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() &&
      !bullet.test(lines[i]) &&
      !lines[i].trim().startsWith('```') &&
      !/^#{1,6}\s/.test(lines[i]) &&
      !(lines[i].includes('|') && i + 1 < lines.length && isRule(lines[i + 1]))
    ) {
      para.push(lines[i++].trim());
    }
    const k = key++;
    out.push(<p key={k}>{inline(para.join(' '), onOpen, `${k}p`)}</p>);
  }
  return <div className="ask-markdown">{out}</div>;
}
