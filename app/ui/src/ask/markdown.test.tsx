// Claude's answer, drawn (ask/markdown.tsx). What a fail looks like: a
// record or an article in the answer isn't a link, a link of any other kind
// is one, or anything in the answer reaches the page as markup.

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import { Markdown } from './markdown';
import { type Target, targetOf } from './nav';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement | null = null;
const opened: { target: Target; background: boolean }[] = [];

function draw(text: string): HTMLElement {
  host = document.createElement('div');
  document.body.append(host);
  opened.length = 0;
  const root = createRoot(host);
  act(() => root.render(<Markdown text={text} onOpen={(target, background) => opened.push({ target, background })} />));
  return host;
}

afterEach(() => {
  host?.remove();
  host = null;
});

describe('an answer’s links', () => {
  it('opens a Learn record, a Wikipedia article and a web page, each as what it is', () => {
    const el = draw(
      'Move [Kanji quiz 4](learn://task/01ABC) and see [Fourier transform](wiki://Fourier transform) or [the paper](https://example.org/a_(b).pdf).',
    );
    const links = [...el.querySelectorAll<HTMLAnchorElement>('a.ask-link')];
    expect(links.map((a) => [a.textContent, a.dataset.kind])).toEqual([
      ['Kanji quiz 4', 'row'],
      ['Fourier transform', 'wiki'],
      ['the paper', 'web'],
    ]);
    act(() => links[0].click());
    act(() => links[1].dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, metaKey: true })));
    act(() => links[2].click());
    expect(opened).toEqual([
      { target: { what: 'row', table: 'task', id: '01ABC' }, background: false },
      { target: { what: 'wiki', title: 'Fourier transform', frag: undefined }, background: true },
      { target: { what: 'web', url: 'https://example.org/a_(b).pdf' }, background: false },
    ]);
    // A link never navigates the window.
    expect(links.every((a) => a.getAttribute('href') === '#')).toBe(true);
  });

  it('shows any other address as plain words', () => {
    const el = draw('[run](javascript:alert(1)) [file](file:///etc/passwd) [odd](learn://) [x](data:text/html,hi)');
    expect(el.querySelectorAll('a')).toHaveLength(0);
    expect(el.textContent).toBe('run file odd x');
  });

  it('never makes markup of what an answer says', () => {
    const el = draw('<img src=x onerror=alert(1)> and <script>alert(2)</script> **<b>bold</b>**');
    expect(el.querySelector('img, script, b')).toBeNull();
    expect(el.textContent).toContain('<img src=x onerror=alert(1)>');
    expect(el.querySelector('strong')!.textContent).toBe('<b>bold</b>');
  });
});

describe('an answer’s blocks', () => {
  it('draws paragraphs, lists, a table, bold, italic and code', () => {
    const el = draw(
      [
        'Two tasks are due in **JPN 101** this week, *both* open. Use `SUM`.',
        '',
        '- [Kanji quiz 4](learn://task/1): Fri, Oct 9',
        '- Genki workbook',
        '  continued on a second line',
        '',
        '1. First',
        '2. Second',
        '',
        '| Task | Due |',
        '|---|---|',
        '| [Lab](learn://task/2) | Thu |',
        '| Essay | Sat |',
        '',
        '## A heading reads as a strong line',
        '> quoted',
      ].join('\n'),
    );
    expect(el.querySelectorAll('p')).toHaveLength(2);
    expect(el.querySelector('strong')!.textContent).toBe('JPN 101');
    expect(el.querySelector('em')!.textContent).toBe('both');
    expect(el.querySelector('code')!.textContent).toBe('SUM');
    expect([...el.querySelectorAll('ul li')].map((li) => li.textContent)).toEqual([
      'Kanji quiz 4: Fri, Oct 9',
      'Genki workbook continued on a second line',
    ]);
    expect(el.querySelectorAll('ol li')).toHaveLength(2);
    expect([...el.querySelectorAll('th')].map((c) => c.textContent)).toEqual(['Task', 'Due']);
    expect([...el.querySelectorAll('tbody tr')].map((r) => r.textContent)).toEqual(['LabThu', 'EssaySat']);
    expect(el.querySelector('tbody a.ask-link')!.textContent).toBe('Lab');
    expect(el.querySelector('.ask-heading')!.textContent).toBe('A heading reads as a strong line');
    expect(el.querySelector('blockquote')!.textContent).toBe('quoted');
  });

  it('leaves signs that only look like marks as they are', () => {
    const el = draw('2 * 3 = 6, a_b_c, 5 * x * y and [not a link] (no address)');
    expect(el.querySelector('em, a')).toBeNull();
    expect(el.textContent).toBe('2 * 3 = 6, a_b_c, 5 * x * y and [not a link] (no address)');
  });
});

describe('an address', () => {
  it('is a record, an article, a web page, or nothing', () => {
    expect(targetOf('learn://course/abc')).toEqual({ what: 'row', table: 'course', id: 'abc' });
    expect(targetOf('wiki://Erd%C5%91s%E2%80%93R%C3%A9nyi_model#Definition')).toEqual({
      what: 'wiki',
      title: 'Erdős–Rényi model',
      frag: 'Definition',
    });
    expect(targetOf('http://example.org')).toEqual({ what: 'web', url: 'http://example.org' });
    for (const no of ['', 'javascript:alert(1)', 'learn://task', 'wiki://', 'https://a b', 'ftp://x', '//example.org'])
      expect(targetOf(no)).toBeNull();
  });
});
