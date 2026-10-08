// The Wiki tab (heat/wiki/Wiki.tsx), on a stand-in for the core's wiki.*.
// What a fail looks like: a picture, a script or raw markup reaches the
// page; a link to an article isn't one, or opens anywhere but the tab; a
// web link opens inside Learn; ⌘click leaves the article being read; back,
// forward or the trail lose the path; the prompt box isn't told what is on
// screen.

import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { CoreError } from '../../bridge';
import { screen } from '../../ask/context';
import { navigate, onNavigate, type Target } from '../../ask/nav';
import { standIn, type StandIn } from '../../ask/testkit';
import { $, $$, button, click, mountHeat, press, type Rig, settle, type, wait } from '../testkit';
import type { Article } from './api';

const article = (title: string, more: Partial<Article> = {}): Article => ({
  title,
  description: `About ${title}`,
  revision: '1',
  modified: '2026-09-28T01:44:15Z',
  url: `https://en.wikipedia.org/wiki/${title.replace(/ /g, '_')}`,
  sections: [],
  blocks: [{ t: 'p', c: [`${title} is a thing. See `, { t: 'a', title: 'Fourier transform', c: ['the transform'] }, '.'] }],
  refs: [],
  ...more,
});

const FOURIER = article('Fourier transform', {
  sections: [
    { id: 'Definition', title: 'Definition', level: 1, number: '1' },
    { id: 'History', title: 'History', level: 2, number: '1.1' },
  ],
  blocks: [
    { t: 'note', c: ['Not to be confused with ', { t: 'a', title: 'Fourier series', c: ['Fourier series'] }, '.'] },
    {
      t: 'p',
      c: [
        'An ',
        { t: 'a', title: 'Integral transform', frag: 'History', c: [{ t: 'i', c: ['integral transform'] }] },
        ' written ',
        { t: 'math', s: 'f̂(ξ)' },
        { t: 'ref', n: '1', id: 'cite_note-1' },
        '. See ',
        { t: 'j', frag: 'History', c: ['its history'] },
        ', a file as plain words, and ',
        { t: 'x', href: 'https://example.org/paper.pdf', c: ['a paper'] },
        '. <img src=x onerror=alert(1)>',
      ],
    },
    { t: 'table', box: true, rows: [[{ h: true, c: ['Field'] }, { c: ['Harmonic analysis'] }]] },
    { t: 'h', level: 2, id: 'Definition', text: 'Definition' },
    { t: 'math', text: 'f̂(ξ) = ∫ f(x) e^(−i2πξx) dx' },
    { t: 'ul', items: [{ c: ['One'], sub: [{ t: 'ul', items: [{ c: ['Nested'], sub: [] }] }] }] },
    { t: 'table', caption: ['Pairs'], rows: [[{ h: true, c: ['Function'] }, { h: true, c: ['Transform'] }], [{ c: ['rect'] }, { c: ['sinc'] }]] },
    { t: 'h', level: 3, id: 'History', text: 'History' },
    { t: 'p', c: ['Joseph Fourier, 1822.'] },
  ],
  refs: [{ id: 'cite_note-1', n: '1', c: ['Stein & Weiss, 1971.'] }],
});

let rig: Rig | null = null;
let core: StandIn;
let off: (() => void) | null = null;
const web: Target[] = [];

async function mount(more: Parameters<typeof standIn>[0] = {}): Promise<Rig> {
  core = standIn({
    'wiki.article': ({ title }) => {
      if (title === 'Fourier transform' || title === 'FT') return { article: FOURIER, fetchedAt: 1_790_000_000_000, cached: title === 'FT' };
      if (title === 'Nowhere') throw new CoreError('not_found', 'Wikipedia has no article called ‘Nowhere’.');
      return { article: article(String(title)), fetchedAt: 1_790_000_000_000, cached: false };
    },
    'wiki.related': () => ({ results: [{ title: 'Laplace transform', description: 'Integral transform' }, { title: 'Fourier series', description: '' }] }),
    'wiki.summary': ({ title }) => ({ title, description: '', extract: `${title}: the first paragraph.`, disambiguation: false }),
    'wiki.saved': () => ({ articles: [{ title: 'Kanji', fetchedAt: 1 }] }),
    'wiki.suggest': ({ q }) => ({ results: String(q).startsWith('four') ? [{ title: 'Fourier transform', description: 'Mathematical transform' }] : [] }),
    'wiki.search': () => ({ results: [{ title: 'Kanji', snippet: 'Logographic characters' }] }),
    ...more,
  });
  web.length = 0;
  off = onNavigate((t) => {
    if (t.what === 'web') web.push(t);
  });
  rig = await mountHeat();
  return rig;
}

afterEach(() => {
  off?.();
  rig?.unmount();
  rig = null;
});

const go = async (target: Target) => {
  await act(async () => {
    navigate(target);
    await new Promise((r) => setTimeout(r, 0));
  });
  await settle(6);
};
const title = () => $(rig, '.wiki-title')?.textContent;
const trail = () => $$(rig, '.wiki-trail li').map((li) => li.textContent);
const tab = () => $(rig, '[role="tab"][aria-selected="true"]')!.textContent;
const side = (heading: string) => {
  const h = $$(document.body, '.wiki-side .heat-side-heading').find((x) => x.textContent?.trim() === heading);
  const out: string[] = [];
  for (let el = h?.nextElementSibling; el && !el.matches('.heat-side-heading'); el = el.nextElementSibling) out.push(el.textContent ?? '');
  return out;
};
const cmd = (key: string, code: string) => press(rig!, key, { code, metaKey: true, ctrlKey: true });
const cmdClick = async (el: Element) => {
  await act(async () => {
    el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, metaKey: true }));
    await new Promise((r) => setTimeout(r, 0));
  });
  await settle();
};

describe('the Wiki tab', () => {
  it('asks for nothing until it is shown, then lists what is saved on this Mac', async () => {
    await mount();
    expect(core.calls).toHaveLength(0);
    await click(button(rig, 'Wiki'));
    expect(core.asked('wiki.saved')).toHaveLength(1);
    expect($(rig, '.wiki-empty')!.textContent).toContain('Saved on this Mac');
    await click($(rig, '.wiki-empty .wiki-a'));
    expect(title()).toBe('Kanji');
  });

  it('draws an article as text, with nothing in it that runs or loads', async () => {
    await mount();
    await go({ what: 'wiki', title: 'Fourier transform' });
    expect(tab()).toBe('Wiki');
    expect(title()).toBe('Fourier transform');
    const page = $(rig, '.wiki-article')!;
    expect(page.querySelector('img, script, iframe, video, style')).toBeNull();
    // Markup in an article's words is words.
    expect(page.textContent).toContain('<img src=x onerror=alert(1)>');
    expect(page.querySelector('.wiki-description')!.textContent).toBe('About Fourier transform');
    expect($$(page, '.wiki-h').map((h) => `${h.tagName} ${h.textContent}`)).toEqual(['H2 Definition', 'H3 History']);
    expect(page.querySelector('.wiki-math-block')!.textContent).toBe('f̂(ξ) = ∫ f(x) e^(−i2πξx) dx');
    expect(page.querySelector('.wiki-list .wiki-list li')!.textContent).toBe('Nested');
    expect($$(page, '.wiki-table:not([data-box]) th').map((c) => c.textContent)).toEqual(['Function', 'Transform']);
    // The infobox and the references are folded; the licence is at the foot.
    expect(page.querySelector<HTMLDetailsElement>('.wiki-box')!.open).toBe(false);
    expect(page.querySelector<HTMLDetailsElement>('.wiki-refs')!.open).toBe(false);
    expect(page.querySelector('.wiki-refs summary')!.textContent).toBe('References (1)');
    const licence = page.querySelector('.wiki-licence')!.textContent!;
    expect(licence).toContain('“Fourier transform” on Wikipedia');
    expect(licence).toContain('Creative Commons Attribution-ShareAlike 4.0');
    expect(rig!.status().count).toBe('Fourier transform · read from Wikipedia just now');
    // A reference's number opens the list at it.
    await click(page.querySelector('.wiki-ref a'));
    expect(page.querySelector<HTMLDetailsElement>('.wiki-refs')!.open).toBe(true);
  });

  it('opens a linked article in the tab, and a web link in the browser', async () => {
    await mount();
    await go({ what: 'wiki', title: 'Fourier transform' });
    const links = $$(rig, '.wiki-article .wiki-a');
    expect(links.map((a) => a.textContent)).toEqual(['Fourier series', 'integral transform']);
    expect(links.every((a) => a.getAttribute('href') === '#')).toBe(true);
    await click(links[1]);
    expect(title()).toBe('Integral transform');
    expect(core.asked('wiki.article').map((a) => a.title)).toEqual(['Fourier transform', 'Integral transform']);
    await go({ what: 'wiki', title: 'Fourier transform' });
    await click($(rig, '.wiki-article .wiki-x'));
    await click($$(rig, '.wiki-licence .wiki-x')[0]);
    expect(web).toEqual([
      { what: 'web', url: 'https://example.org/paper.pdf' },
      { what: 'web', url: 'https://en.wikipedia.org/wiki/Fourier_transform' },
    ]);
    expect(title()).toBe('Fourier transform');
  });

  it('keeps the path: the trail, back and forward', async () => {
    await mount();
    await go({ what: 'wiki', title: 'Fourier transform' });
    await click($$(rig, '.wiki-article .wiki-a')[1]);
    await click($(rig, '.wiki-article .wiki-a'));
    await click($$(rig, '.wiki-article .wiki-a')[0]);
    expect(trail()).toEqual(['Fourier transform', 'Integral transform', 'Fourier transform', 'Fourier series']);
    expect(await cmd('[', 'BracketLeft')).toBe(true);
    expect(title()).toBe('Fourier transform');
    await cmd('[', 'BracketLeft');
    expect(title()).toBe('Integral transform');
    expect(trail()).toEqual(['Fourier transform', 'Integral transform']);
    await cmd(']', 'BracketRight');
    await cmd(']', 'BracketRight');
    await cmd(']', 'BracketRight');
    expect(title()).toBe('Fourier series');
    // Any step of the trail can be gone back to, and forward is still there after.
    await click($$(rig, '.wiki-crumb')[0]);
    expect(title()).toBe('Fourier transform');
    expect(trail()).toEqual(['Fourier transform']);
    expect(button(rig, /^Forward/)!.hasAttribute('disabled')).toBe(false);
    expect(button(rig, /^Back/)!.hasAttribute('disabled')).toBe(true);
    // Opening a link from the middle of the path drops what was ahead.
    await click($$(rig, '.wiki-article .wiki-a')[0]);
    expect(trail()).toEqual(['Fourier transform', 'Fourier series']);
    expect(button(rig, /^Forward/)!.hasAttribute('disabled')).toBe(true);
  });

  it('opens a ⌘clicked link behind what is being read', async () => {
    await mount();
    await go({ what: 'wiki', title: 'Fourier transform' });
    await cmdClick($$(rig, '.wiki-article .wiki-a')[1]);
    expect(title()).toBe('Fourier transform');
    expect(trail()).toEqual(['Fourier transform']);
    // It was read into this Mac, and waits in the sidebar.
    expect(core.asked('wiki.article').map((a) => a.title)).toEqual(['Fourier transform', 'Integral transform']);
    expect(side('Opened for later')).toEqual(['Integral transform']);
    await click($$(document.body, '.wiki-side .heat-side-row').find((b) => b.textContent === 'Integral transform'));
    expect(title()).toBe('Integral transform');
    expect(side('Opened for later')).toEqual([]);
    // The prompt box's link opens in the background the same way.
    await go({ what: 'wiki', title: 'Kanji', background: true });
    expect(title()).toBe('Integral transform');
    expect(side('Opened for later')).toEqual(['Kanji']);
  });

  it('previews a link after a short rest, with its title and first paragraph', async () => {
    await mount();
    await go({ what: 'wiki', title: 'Fourier transform' });
    const link = $$(rig, '.wiki-article .wiki-a')[1];
    await act(async () => {
      link.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });
    expect($(rig, '.wiki-preview')).toBeNull();
    await wait(420);
    await settle();
    expect($(rig, '.wiki-preview strong')!.textContent).toBe('Integral transform');
    expect($(rig, '.wiki-preview p')!.textContent).toBe('Integral transform: the first paragraph.');
    expect($(rig, '.wiki-preview')!.querySelector('img, a')).toBeNull();
    await act(async () => {
      link.dispatchEvent(new MouseEvent('mouseout', { bubbles: true }));
    });
    expect($(rig, '.wiki-preview')).toBeNull();
    // Resting on it again asks Wikipedia nothing more.
    await act(async () => {
      link.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });
    await wait(420);
    expect(core.asked('wiki.summary')).toHaveLength(1);
  });

  it('lists the contents and related articles in the sidebar', async () => {
    await mount();
    await go({ what: 'wiki', title: 'Fourier transform' });
    expect(side('Contents')).toEqual(['Fourier transform', 'Definition', 'History']);
    expect(side('Related')).toEqual(['Laplace transform', 'Fourier series']);
    await click($$(document.body, '.wiki-side .heat-side-row').find((b) => b.textContent === 'Laplace transform'));
    expect(title()).toBe('Laplace transform');
    expect(trail()).toEqual(['Fourier transform', 'Laplace transform']);
  });

  it('suggests titles as you type, and Return opens the first', async () => {
    await mount();
    await click(button(rig, 'Wiki'));
    const field = $(rig, '.wiki-search input') as HTMLInputElement;
    await type(field, 'four');
    await wait(200);
    await settle();
    expect($$(rig, '.wiki-suggestions li').map((li) => li.textContent)).toEqual(['Fourier transformMathematical transform']);
    await act(async () => {
      field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    await settle(6);
    expect(title()).toBe('Fourier transform');
    // With no title to suggest, Return lists articles about the words.
    await type(field, 'writing systems');
    await wait(200);
    await act(async () => {
      field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    await settle(6);
    expect($(rig, '.wiki-title')!.textContent).toBe('Articles about “writing systems”');
    expect($(rig, '.wiki-results li')!.textContent).toBe('KanjiLogographic characters');
  });

  it('says so when an article can’t be had, and when the copy shown is the saved one', async () => {
    await mount({
      'wiki.article': ({ title }) => {
        if (title === 'Nowhere') throw new CoreError('not_found', 'Wikipedia has no article called ‘Nowhere’.');
        return { article: FOURIER, fetchedAt: Date.UTC(2026, 9, 7, 16), cached: true, offline: true };
      },
    });
    await go({ what: 'wiki', title: 'Nowhere' });
    expect($(rig, '.wiki-status[role="alert"]')!.textContent).toBe('Wikipedia has no article called ‘Nowhere’.');
    await go({ what: 'wiki', title: 'Fourier transform' });
    expect($(rig, '.wiki-offline')!.textContent).toBe(
      'Wikipedia can’t be reached. This is the copy saved on this Mac on October 7, 2026.',
    );
    expect(rig!.status().count).toContain('showing the copy saved on this Mac');
  });

  it('tells the prompt box the article and the section on screen', async () => {
    await mount();
    await go({ what: 'wiki', title: 'Fourier transform' });
    expect(screen()!.wiki).toEqual({
      title: 'Fourier transform',
      section: '',
      sections: ['Definition', 'History'],
      url: 'https://en.wikipedia.org/wiki/Fourier_transform',
    });
    expect(screen()!.tab).toBe('wiki');
    await click(button(rig, 'Tasks'));
    expect(screen()!.wiki).toBeUndefined();
  });
});
