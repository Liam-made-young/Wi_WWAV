import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { type Hooks, Markdown } from './markdown';

// docs/NOTES.md, "Links, tags, checkboxes". What a fail looks like: a
// construct drawn as its own markers; anything inside backticks or a fence
// read as a link, a tag or a checkbox; `^t-<id>` on show; a checkbox that
// names the wrong line of the note; a link that isn't a web address given
// to the browser to open.

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const html = (text: string, hooks?: Hooks, base?: number) => renderToStaticMarkup(<Markdown text={text} hooks={hooks} base={base} />);
/** The markup without the classes and line marks, to read the structure. */
const bare = (text: string, hooks?: Hooks) =>
  html(text, hooks)
    .replace(/ (class|data-line|data-dense)="[^"]*"/g, '')
    // React asks the page to fetch an image early; that is not the note's markup.
    .replace(/<link rel="preload"[^>]*>/g, '');

let host: HTMLElement | undefined;
afterEach(() => {
  host?.remove();
  vi.restoreAllMocks();
});
function mount(text: string, hooks: Hooks, base = 0) {
  host = document.createElement('div');
  document.body.append(host);
  const root = createRoot(host);
  act(() => root.render(<Markdown text={text} hooks={hooks} base={base} />));
  return host;
}

describe('blocks', () => {
  it('draws headings from # to ####, and leaves a # with no space alone', () => {
    expect(bare('# One\n## Two\n### Three\n#### Four')).toBe('<h1>One</h1><h2>Two</h2><h3>Three</h3><h4>Four</h4>');
    expect(bare('## Closed ##')).toBe('<h2>Closed</h2>');
    expect(bare('#5 of them')).toBe('<p>#5 of them</p>');
  });

  it('keeps a paragraph’s lines one under another', () => {
    expect(bare('one\ntwo')).toBe('<p>one<br/>two</p>');
  });

  it('draws a fenced block as written, with its language', () => {
    expect(bare('```js\nconst a = 1;\n\n  b();\n```')).toBe('<pre><code data-lang="js">const a = 1;\n\n  b();</code></pre>');
    expect(bare('~~~\nplain\n~~~')).toBe('<pre><code>plain</code></pre>');
    // A fence nothing closes runs to the end.
    expect(bare('```\nopen')).toBe('<pre><code>open</code></pre>');
  });

  it('draws a quote, and what is inside it', () => {
    expect(bare('> Less is more.\n> - one')).toBe('<blockquote><p>Less is more.</p><ul><li><span>one</span></li></ul></blockquote>');
  });

  it('draws bulleted and numbered lists, nested by their indent', () => {
    expect(bare('- a\n    - b\n        - c\n- d')).toBe(
      '<ul><li><span>a</span><ul><li><span>b</span><ul><li><span>c</span></li></ul></li></ul></li><li><span>d</span></li></ul>',
    );
    expect(bare('1. one\n2. two')).toBe('<ol><li><span>one</span></li><li><span>two</span></li></ol>');
    expect(bare('3. three\n4. four')).toBe('<ol start="3"><li><span>three</span></li><li><span>four</span></li></ol>');
    expect(bare('* star\n+ plus')).toBe('<ul><li><span>star</span></li><li><span>plus</span></li></ul>');
    // A line indented under an item goes on with it.
    expect(bare('- a\n  more')).toBe('<ul><li><span>a<br/>more</span></li></ul>');
  });

  it('draws checkboxes as real ones, and never shows the task marker', () => {
    const out = bare('- [ ] Do worksheet 4 ^t-01JABC\n- [x] Read 3.2');
    expect(out).toBe(
      '<ul><li data-box=""><input type="checkbox" aria-label="Do worksheet 4" disabled=""/><span>Do worksheet 4</span></li>' +
        '<li data-box=""><input type="checkbox" aria-label="Read 3.2" disabled="" checked=""/><span>Read 3.2</span></li></ul>',
    );
    expect(out).not.toContain('^t-');
  });

  it('draws a table from its pipes and the rule under its heading row', () => {
    expect(bare('| Group | Te-form |\n|---|--:|\n| One | 書いて |\n| Two | 食べて |')).toBe(
      '<table><thead><tr><th scope="col">Group</th><th scope="col" style="text-align:right">Te-form</th></tr></thead>' +
        '<tbody><tr><td>One</td><td style="text-align:right">書いて</td></tr><tr><td>Two</td><td style="text-align:right">食べて</td></tr></tbody></table>',
    );
    // Without the rule it is only a line with pipes in it.
    expect(bare('a | b')).toBe('<p>a | b</p>');
  });

  it('draws a rule for ---, *** and - - -', () => {
    expect(bare('---\n***\n- - -')).toBe('<hr/><hr/><hr/>');
  });

  it('marks each piece with the line it starts on', () => {
    expect(html('# T\ntext\n- a\n- b')).toContain('<li data-line="3">');
  });
});

describe('inline', () => {
  it('draws bold, italic, strike and code', () => {
    expect(bare('**bold** *it* ~~no~~ `code` ***both*** __b__ _i_')).toBe(
      '<p><strong>bold</strong> <em>it</em> <del>no</del> <code>code</code> <strong><em>both</em></strong> <strong>b</strong> <em>i</em></p>',
    );
    // An underscore inside a word is a letter, and a star with a space after it is a star.
    expect(bare('snake_case_name and 5 * 3 * 2')).toBe('<p>snake_case_name and 5 * 3 * 2</p>');
    expect(bare('\\*not\\* \\#tag')).toBe('<p>*not* #tag</p>');
  });

  it('draws [text](url) and a bare address as links that open in the browser', () => {
    vi.spyOn(window, 'open').mockImplementation(() => null);
    const el = mount('See [the page](https://example.com/a) or https://example.com/b.', {});
    const links = [...el.querySelectorAll('a')];
    expect(links.map((a) => [a.textContent, a.getAttribute('href')])).toEqual([
      ['the page', 'https://example.com/a'],
      ['https://example.com/b', 'https://example.com/b'],
    ]);
    act(() => links[0].click());
    expect(window.open).toHaveBeenLastCalledWith('https://example.com/a', '_blank', 'noopener,noreferrer');
    // Only a web address is ever a link.
    expect(bare('[run](javascript:alert(1))')).not.toContain('href');
  });

  it('draws an image, with a path in angle brackets when it has spaces', () => {
    expect(bare('![a page](https://example.com/p.png)')).toBe('<p><img src="https://example.com/p.png" alt="a page"/></p>');
    const seen: [string, string][] = [];
    const image: Hooks['image'] = (src, alt) => {
      seen.push([src, alt]);
      return <i>{src}</i>;
    };
    expect(bare('![](attachments/p.jpg) ![two](<attachments/my page.jpg>)', { image })).toBe(
      '<p><i>attachments/p.jpg</i> <i>attachments/my page.jpg</i></p>',
    );
    expect(seen).toEqual([
      ['attachments/p.jpg', ''],
      ['attachments/my page.jpg', 'two'],
    ]);
  });

  it('draws a wikilink by what it finds, with the words it was given', () => {
    const resolve: Hooks['resolve'] = (target) =>
      target === 'Verb groups' ? { kind: 'note', id: 'n-1' } : target === 'JPN 201' ? { kind: 'course', id: 'c-1' } : { kind: 'missing', id: null };
    const out = html('[[Verb groups]] [[JPN 201|the class]] [[Keigo]] [[Verb groups#Group one]]', { resolve });
    expect([...out.matchAll(/data-kind="(\w+)"[^>]*>([^<]*)</g)].map((m) => [m[1], m[2]])).toEqual([
      ['note', 'Verb groups'],
      ['course', 'the class'],
      ['missing', 'Keigo'],
      ['note', 'Verb groups#Group one'],
    ]);
  });

  it('tells the caller which link and which tag were clicked', () => {
    const onLink = vi.fn();
    const onTag = vi.fn();
    const el = mount('[[Keigo]] and #grammar', { onLink, onTag });
    act(() => el.querySelector<HTMLElement>('.notes-wikilink')!.click());
    expect(onLink).toHaveBeenCalledWith('Keigo', { kind: 'missing', id: null });
    act(() => el.querySelector<HTMLElement>('.notes-tag')!.click());
    expect(onTag).toHaveBeenCalledWith('grammar');
  });

  it('draws #tags as chips, but not a heading, a number or the end of a word', () => {
    expect(bare('#grammar and (#te-form) #会話')).toBe('<p><span>#grammar</span> and (<span>#te-form</span>) <span>#会話</span></p>');
    expect(bare('#1 a#b https://example.com/a#part')).toBe('<p>#1 a#b <a href="https://example.com/a#part">https://example.com/a#part</a></p>');
  });

  it('leaves everything inside code alone', () => {
    expect(bare('`[[Verb groups]] #tag **b** https://example.com`')).toBe('<p><code>[[Verb groups]] #tag **b** https://example.com</code></p>');
    expect(bare('```\n[[Verb groups]] #tag\n- [ ] not a box\n```')).toBe('<pre><code>[[Verb groups]] #tag\n- [ ] not a box</code></pre>');
  });
});

describe('checkboxes', () => {
  it('names the line of the whole note when one is ticked', () => {
    const onBox = vi.fn();
    const el = mount('- [ ] one\n- [x] two', { onBox }, 7);
    const boxes = [...el.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')];
    expect(boxes.map((b) => b.checked)).toEqual([false, true]);
    act(() => boxes[0].click());
    expect(onBox).toHaveBeenLastCalledWith(7, true);
    act(() => boxes[1].click());
    expect(onBox).toHaveBeenLastCalledWith(8, false);
  });

  it('shows the state and the words the caller gives for a box', () => {
    const box: Hooks['box'] = (line, written, text) => ({ done: line === 2, beside: <b>{`${line}:${written}:${text}`}</b> });
    const el = mount('- [ ] one ^t-9', { box, onBox: () => {} }, 2);
    expect(el.querySelector<HTMLInputElement>('input')!.checked).toBe(true);
    expect(el.querySelector('b')!.textContent).toBe('2:false:one ^t-9');
    expect(el.querySelector('.notes-item')!.textContent).toBe('one');
  });
});
