// The Wiki tab (docs/ASK.md): Wikipedia as a reader, in Learn's own type.
// Text only, read-only: headings, paragraphs, lists and tables, with the
// references folded at the foot and the licence under them.
//
// Every link to another article is a link here. A click opens it in the tab,
// from this Mac when it has been read before; ⌘click opens it behind what
// you are reading, in "Opened for later"; resting on it shows the article's
// title and first paragraph. Links to files, templates and talk pages
// arrive as plain words (the core sorts them), and a link to the web opens
// in the system browser, never here.
//
// ⌘[ and ⌘] go back and forward. The trail across the top is the path of
// articles read this session, and any step of it can be gone back to. The
// sidebar holds the article's contents and related articles. The prompt box
// is told which article and section is on screen.

import {
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';
import { setScreen } from '../../ask/context';
import { navigate, onNavigate } from '../../ask/nav';
import { latestOnly } from '../../shell/palette';
import { keys } from '../../shell/platform';
import { sentenceOf } from '../fmt';
import { useSidebarSlot, useTabActs, useTabKeys, useTabScope, useTabs } from '../frame';
import { useHeat } from '../store';
import { type Article, type Block, type Inline, type Item, type Loaded, type Suggestion, type Summary, wiki } from './api';
import './wiki.css';

interface Step {
  title: string;
  frag?: string;
}

const PREVIEW_AFTER_MS = 350;
const sectionId = (id: string) => `wiki-sec-${id}`;
/** A value inside an attribute selector's quotes. */
const quoted = (s: string) => s.replace(/["\\]/g, '\\$&');

export function Wiki() {
  const { active } = useTabScope();
  const slot = useSidebarSlot();
  const { say } = useHeat();
  const { setTab } = useTabs();

  // The path through the articles read this session, and where on it the reader stands.
  const [steps, setSteps] = useState<Step[]>([]);
  const [at, setAt] = useState(-1);
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [loading, setLoading] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [related, setRelated] = useState<Suggestion[]>([]);
  const [later, setLater] = useState<string[]>([]);
  const [saved, setSaved] = useState<{ title: string; fetchedAt: number }[]>([]);
  const [section, setSection] = useState('');
  const [refsOpen, setRefsOpen] = useState(false);
  const [results, setResults] = useState<{ q: string; list: { title: string; snippet?: string; description?: string }[] } | null>(null);
  const [preview, setPreview] = useState<{ title: string; x: number; y: number; above: boolean; summary: Summary | null } | null>(null);

  const page = useRef<HTMLDivElement>(null);
  const trail = useRef<HTMLOListElement>(null);
  const searchField = useRef<HTMLInputElement>(null);
  const asked = useRef(0);
  const hover = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const here = at >= 0 ? steps[at] : null;
  const article = loaded?.article ?? null;

  // Reads the article the reader stands on, whenever that changes.
  useEffect(() => {
    if (!here) return;
    const mine = ++asked.current;
    setLoading(here.title);
    setError(null);
    setResults(null);
    setPreview(null);
    wiki.article(here.title).then(
      (got) => {
        if (mine !== asked.current) return;
        setLoaded(got);
        setLoading(null);
        setRefsOpen(false);
        // The path names the article as Wikipedia does, whatever was typed or linked.
        setSteps((all) => all.map((s, i) => (i === at && s.title !== got.article.title ? { ...s, title: got.article.title } : s)));
        requestAnimationFrame(() => {
          const el = here.frag ? page.current?.querySelector<HTMLElement>(`[data-frag="${quoted(here.frag)}"]`) : null;
          if (el) el.scrollIntoView?.();
          else if (page.current) page.current.scrollTop = 0;
        });
        wiki.related(got.article.title).then(
          (r) => mine === asked.current && setRelated(r.results),
          () => mine === asked.current && setRelated([]),
        );
      },
      (e) => {
        if (mine !== asked.current) return;
        setLoading(null);
        setError(sentenceOf(e));
      },
    );
  }, [here?.title, at]); // eslint-disable-line react-hooks/exhaustive-deps

  const open = useCallback(
    (title: string, frag?: string) => {
      setSteps((all) => [...all.slice(0, at + 1), { title, frag }]);
      setAt(at + 1);
      setLater((l) => l.filter((t) => t !== title));
    },
    [at],
  );

  /** ⌘click: the article is read into this Mac and waits in the sidebar. */
  const openLater = useCallback(
    (title: string) => {
      setLater((l) => (l.includes(title) ? l : [...l, title]));
      wiki.article(title).then(
        (got) => say(`‘${got.article.title}’ is open for later.`),
        (e) => {
          setLater((l) => l.filter((t) => t !== title));
          say(sentenceOf(e));
        },
      );
    },
    [say],
  );

  const go = useCallback(
    (to: number) => {
      if (to < 0 || to >= steps.length || to === at) return;
      setAt(to);
    },
    [steps.length, at],
  );

  // The trail keeps its newest step in view.
  useEffect(() => {
    const el = trail.current;
    if (el) el.scrollLeft = el.scrollWidth;
  }, [steps, at]);

  // A link to an article from anywhere: the prompt box, a search result, another tab.
  useEffect(
    () =>
      onNavigate((t) => {
        if (t.what !== 'wiki') return;
        if (t.background) return openLater(t.title);
        setTab('wiki');
        open(t.title, t.frag);
      }),
    [open, openLater, setTab],
  );

  // The tab opened with nothing read yet lists what is kept on this Mac.
  useEffect(() => {
    if (active && !here) wiki.saved().then((s) => setSaved(s.articles), () => {});
  }, [active, here]);

  const jump = (frag: string) => {
    const el =
      page.current?.querySelector<HTMLElement>(`[data-frag="${quoted(frag)}"]`) ??
      page.current?.querySelector<HTMLElement>(`[data-frag="${quoted(frag.replace(/ /g, '_'))}"]`);
    if (el) el.scrollIntoView?.({ block: 'start' });
  };

  const showRef = (id: string) => {
    setRefsOpen(true);
    requestAnimationFrame(() => page.current?.querySelector<HTMLElement>(`[data-ref="${quoted(id)}"]`)?.scrollIntoView?.({ block: 'center' }));
  };

  // Which section is being read: the last heading above the top of the page.
  const onScroll = () => {
    const el = page.current;
    if (!el) return;
    setPreview(null);
    let current = '';
    for (const h of el.querySelectorAll<HTMLElement>('[data-heading]')) {
      if (h.offsetTop - el.scrollTop > 80) break;
      current = h.dataset.heading ?? '';
    }
    setSection((was) => (was === current ? was : current));
  };

  const linkOver = (e: ReactMouseEvent, title: string) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const host = page.current?.getBoundingClientRect();
    if (!host) return;
    clearTimeout(hover.current);
    hover.current = setTimeout(() => {
      const above = r.bottom + 190 > host.bottom;
      const at = { title, x: Math.min(Math.max(r.left - host.left, 8), host.width - 348), y: above ? r.top - host.top - 6 : r.bottom - host.top + 6, above };
      setPreview({ ...at, summary: null });
      wiki.summary(title).then(
        (summary) => setPreview((p) => (p?.title === title ? { ...p, summary } : p)),
        () => setPreview((p) => (p?.title === title ? null : p)),
      );
    }, PREVIEW_AFTER_MS);
  };
  const linkOut = () => {
    clearTimeout(hover.current);
    setPreview(null);
  };

  const linkClick = (e: ReactMouseEvent, title: string, frag?: string) => {
    e.preventDefault();
    linkOut();
    if (e.metaKey || e.ctrlKey) openLater(title);
    else open(title, frag);
  };

  // No secondary act: the frame gives ⇧click to it, and here ⇧click selects text.
  // "On Wikipedia" is the link at the foot of every article.
  useTabActs({
    count: article
      ? `${article.title} · ${loaded?.offline ? 'Wikipedia can’t be reached: showing the copy saved on this Mac' : loaded?.cached ? 'saved on this Mac' : 'read from Wikipedia just now'}`
      : null,
  });
  useTabKeys({
    key: (e) => {
      if (!e.command || e.alt || e.shift) return false;
      if (e.code === 'BracketLeft') {
        go(at - 1);
        return true;
      }
      if (e.code === 'BracketRight') {
        go(at + 1);
        return true;
      }
      return false;
    },
    escape: () => {
      if (preview) {
        setPreview(null);
        return true;
      }
      return false;
    },
    filter: () => searchField.current?.focus(),
  });

  // The prompt box is told which article and which section is being read.
  useEffect(() => {
    const title = article?.sections.find((s) => s.id === section)?.title;
    setScreen(
      'wiki',
      active && article
        ? { title: article.title, section: title ?? '', sections: article.sections.map((s) => s.title), url: article.url }
        : null,
    );
  }, [active, article, section]);

  const draw = useMemo(
    () => makeDrawer({ click: linkClick, over: linkOver, out: linkOut, jump, showRef }),
    [open, openLater], // eslint-disable-line react-hooks/exhaustive-deps
  );
  const body = useMemo(() => (article ? article.blocks.map((b, i) => draw.block(b, `b${i}`)) : null), [article, draw]);

  const search = (q: string) => {
    const mine = ++asked.current;
    setLoading(q);
    setError(null);
    wiki.search(q).then(
      (r) => {
        if (mine !== asked.current) return;
        setLoading(null);
        setResults({ q, list: r.results });
        setLoaded(null);
      },
      (e) => {
        if (mine !== asked.current) return;
        setLoading(null);
        setError(sentenceOf(e));
      },
    );
  };

  return (
    <div className="wiki">
      {slot &&
        createPortal(
          <nav className="wiki-side" aria-label="Contents">
            {article && article.sections.length > 0 && (
              <>
                <h2 className="heat-side-heading" data-text="secondary">
                  Contents
                </h2>
                <button type="button" className="heat-side-row" data-dense aria-current={section === '' ? 'true' : undefined} onClick={() => page.current && (page.current.scrollTop = 0)}>
                  <span className="heat-side-name">{article.title}</span>
                </button>
                {article.sections.map((s) => (
                  <button
                    key={s.id}
                    type="button"
                    className="heat-side-row wiki-toc"
                    data-dense
                    data-level={Math.min(s.level, 3)}
                    aria-current={section === s.id ? 'true' : undefined}
                    title={s.title}
                    onClick={() => jump(s.id)}
                  >
                    <span className="heat-side-name">{s.title}</span>
                  </button>
                ))}
              </>
            )}
            {article && related.length > 0 && (
              <>
                <h2 className="heat-side-heading" data-text="secondary">
                  Related
                </h2>
                {related.slice(0, 8).map((r) => (
                  <button
                    key={r.title}
                    type="button"
                    className="heat-side-row"
                    data-dense
                    title={r.description || r.title}
                    onClick={(e) => (e.metaKey || e.ctrlKey ? openLater(r.title) : open(r.title))}
                  >
                    <span className="heat-side-name">{r.title}</span>
                  </button>
                ))}
              </>
            )}
            {later.length > 0 && (
              <>
                <h2 className="heat-side-heading" data-text="secondary">
                  Opened for later
                </h2>
                {later.map((t) => (
                  <button key={t} type="button" className="heat-side-row" data-dense onClick={() => open(t)}>
                    <span className="heat-side-name">{t}</span>
                  </button>
                ))}
              </>
            )}
          </nav>,
          slot,
        )}

      <div className="wiki-bar" role="toolbar" aria-label="Wiki">
        <button type="button" className="wiki-step" aria-label={`Back (${keys('⌘[')})`} title={`Back (${keys('⌘[')})`} disabled={at <= 0} onClick={() => go(at - 1)}>
          ‹
        </button>
        <button
          type="button"
          className="wiki-step"
          aria-label={`Forward (${keys('⌘]')})`}
          title={`Forward (${keys('⌘]')})`}
          disabled={at >= steps.length - 1}
          onClick={() => go(at + 1)}
        >
          ›
        </button>
        <ol className="wiki-trail" ref={trail} aria-label="Articles read this session">
          {steps.slice(0, at + 1).map((s, i) => (
            <li key={i}>
              {i === at ? (
                <span aria-current="page">{s.title}</span>
              ) : (
                <button type="button" className="wiki-crumb" onClick={() => go(i)}>
                  {s.title}
                </button>
              )}
            </li>
          ))}
        </ol>
        <SearchBox field={searchField} onOpen={(t) => open(t)} onSearch={search} />
      </div>

      <div className="wiki-page" ref={page} onScroll={onScroll} tabIndex={-1}>
        {loading && (
          <p className="wiki-status" role="status">
            Opening {loading}…
          </p>
        )}
        {error && (
          <p className="wiki-status" role="alert">
            {error}
          </p>
        )}

        {!here && !results && !loading && (
          <div className="wiki-article wiki-empty">
            <h1 className="wiki-title">Wiki</h1>
            <p>
              Wikipedia, as text, in Learn. Search for an article above, or ask in the prompt box ({keys('⌘K')}) and follow the
              link under the answer. What you open is kept on this Mac and reads with the network off.
            </p>
            {saved.length > 0 && (
              <>
                <h2 className="wiki-h" data-level="2">
                  Saved on this Mac
                </h2>
                <ul className="wiki-list">
                  {saved.slice(0, 40).map((s) => (
                    <li key={s.title}>
                      <a className="wiki-a" href="#" onClick={(e) => linkClick(e, s.title)}>
                        {s.title}
                      </a>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
        )}

        {results && !loading && (
          <div className="wiki-article">
            <h1 className="wiki-title">Articles about “{results.q}”</h1>
            {results.list.length === 0 && <p>Wikipedia found nothing for that.</p>}
            <ul className="wiki-results">
              {results.list.map((r) => (
                <li key={r.title}>
                  <a className="wiki-a" href="#" onClick={(e) => linkClick(e, r.title)} onMouseEnter={(e) => linkOver(e, r.title)} onMouseLeave={linkOut}>
                    {r.title}
                  </a>
                  <span>{r.snippet ?? r.description ?? ''}</span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {article && !results && (
          <article className="wiki-article" aria-busy={!!loading} lang="en">
            <h1 className="wiki-title">{article.title}</h1>
            {article.description && <p className="wiki-description">{article.description}</p>}
            {loaded?.offline && (
              <p className="wiki-offline" role="status">
                Wikipedia can’t be reached. This is the copy saved on this Mac on{' '}
                {new Date(loaded.fetchedAt).toLocaleDateString('en-US', { month: 'long', day: 'numeric', year: 'numeric' })}.
              </p>
            )}
            {body}

            {article.refs.length > 0 && (
              <details className="wiki-refs" open={refsOpen} onToggle={(e) => setRefsOpen((e.target as HTMLDetailsElement).open)}>
                <summary>References ({article.refs.length})</summary>
                <ol>
                  {article.refs.map((r) => (
                    <li key={r.id || r.n} data-ref={r.id}>
                      {draw.inline(r.c, `r${r.n}`)}
                    </li>
                  ))}
                </ol>
              </details>
            )}

            <footer className="wiki-licence">
              <p>
                From{' '}
                <a
                  className="wiki-x"
                  href="#"
                  onClick={(e) => {
                    e.preventDefault();
                    navigate({ what: 'web', url: article.url });
                  }}
                >
                  “{article.title}” on Wikipedia
                </a>
                , by its contributors
                {article.modified ? `, as last edited ${new Date(article.modified).toLocaleDateString('en-US', { month: 'long', day: 'numeric', year: 'numeric' })}` : ''}
                . Text is available under the{' '}
                <a
                  className="wiki-x"
                  href="#"
                  onClick={(e) => {
                    e.preventDefault();
                    navigate({ what: 'web', url: 'https://creativecommons.org/licenses/by-sa/4.0/' });
                  }}
                >
                  Creative Commons Attribution-ShareAlike 4.0 licence
                </a>
                . Pictures are left out; formulas are shown as text.
              </p>
            </footer>
          </article>
        )}

        {preview && (
          <aside
            className="wiki-preview"
            role="tooltip"
            data-above={preview.above || undefined}
            style={{ left: preview.x, top: preview.y + (page.current?.scrollTop ?? 0) }}
          >
            <strong>{preview.summary?.title ?? preview.title}</strong>
            {preview.summary ? (
              <p>{preview.summary.extract || preview.summary.description || 'Wikipedia has no summary for this article.'}</p>
            ) : (
              <p data-text="secondary">…</p>
            )}
          </aside>
        )}
      </div>
    </div>
  );
}

/** The search box: titles as you type; Return opens the first, or lists articles about the words. */
function SearchBox({
  field,
  onOpen,
  onSearch,
}: {
  field: React.RefObject<HTMLInputElement | null>;
  onOpen(title: string): void;
  onSearch(q: string): void;
}) {
  const [text, setText] = useState('');
  const [list, setList] = useState<Suggestion[]>([]);
  const [pick, setPick] = useState(0);
  const [openList, setOpenList] = useState(false);
  const suggest = useMemo(
    () =>
      latestOnly(
        (q) => wiki.suggest(q).then((r) => r.results),
        (_q, found) => {
          setList(found);
          setPick(0);
        },
        140,
      ),
    [],
  );
  useEffect(() => {
    if (text.trim()) suggest.ask(text.trim());
    else {
      suggest.cancel();
      setList([]);
    }
  }, [text, suggest]);

  const choose = (title: string) => {
    setText('');
    setList([]);
    setOpenList(false);
    field.current?.blur();
    onOpen(title);
  };

  return (
    <div className="wiki-search">
      <input
        ref={field}
        type="search"
        role="combobox"
        aria-expanded={openList && list.length > 0}
        aria-controls="wiki-suggestions"
        aria-activedescendant={list[pick] ? `wiki-suggestion-${pick}` : undefined}
        aria-label="Search Wikipedia"
        placeholder={`Search Wikipedia (${keys('⌘F')})`}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          setOpenList(true);
        }}
        onFocus={() => setOpenList(true)}
        onBlur={() => setTimeout(() => setOpenList(false), 150)}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown') {
            e.preventDefault();
            setPick((p) => Math.min(p + 1, list.length - 1));
          } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            setPick((p) => Math.max(p - 1, 0));
          } else if (e.key === 'Enter' && text.trim()) {
            e.preventDefault();
            if (list[pick]) choose(list[pick].title);
            else {
              onSearch(text.trim());
              setOpenList(false);
            }
          }
        }}
      />
      {openList && list.length > 0 && (
        <ul id="wiki-suggestions" className="wiki-suggestions" role="listbox" aria-label="Articles">
          {list.map((s, i) => (
            <li
              key={s.title}
              id={`wiki-suggestion-${i}`}
              role="option"
              aria-selected={i === pick}
              onPointerEnter={() => setPick(i)}
              onPointerDown={(e) => {
                e.preventDefault();
                choose(s.title);
              }}
            >
              <span>{s.title}</span>
              {(s.description || s.saved) && <span data-text="secondary">{s.description || 'Saved on this Mac'}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

interface Links {
  click(e: ReactMouseEvent, title: string, frag?: string): void;
  over(e: ReactMouseEvent, title: string): void;
  out(): void;
  jump(frag: string): void;
  showRef(id: string): void;
}

/** Draws an article's blocks. Nothing is set as HTML: every node is made here from the article's own shapes. */
function makeDrawer(links: Links) {
  const inline = (nodes: Inline[], key: string): ReactNode[] =>
    nodes.map((n, i) => {
      const k = `${key}.${i}`;
      if (typeof n === 'string') return n;
      switch (n.t) {
        case 'b':
          return <strong key={k}>{inline(n.c, k)}</strong>;
        case 'i':
          return <em key={k}>{inline(n.c, k)}</em>;
        case 'sup':
          return <sup key={k}>{inline(n.c, k)}</sup>;
        case 'sub':
          return <sub key={k}>{inline(n.c, k)}</sub>;
        case 'code':
          return <code key={k}>{inline(n.c, k)}</code>;
        case 'a':
          return (
            <a
              key={k}
              className="wiki-a"
              href="#"
              data-title={n.title}
              onClick={(e) => links.click(e, n.title, n.frag)}
              onMouseEnter={(e) => links.over(e, n.title)}
              onMouseLeave={links.out}
              onFocus={(e) => links.over(e as unknown as ReactMouseEvent, n.title)}
              onBlur={links.out}
            >
              {inline(n.c, k)}
            </a>
          );
        case 'j':
          return (
            <a
              key={k}
              className="wiki-j"
              href="#"
              onClick={(e) => {
                e.preventDefault();
                links.jump(n.frag);
              }}
            >
              {inline(n.c, k)}
            </a>
          );
        case 'x':
          return (
            <a
              key={k}
              className="wiki-x"
              href="#"
              title={`Opens in your browser: ${n.href}`}
              onClick={(e) => {
                e.preventDefault();
                navigate({ what: 'web', url: n.href });
              }}
            >
              {inline(n.c, k)}
            </a>
          );
        case 'math':
          return (
            <span key={k} className="wiki-math">
              {n.s}
            </span>
          );
        case 'ref':
          return (
            <sup key={k} className="wiki-ref">
              <a
                href="#"
                aria-label={`Reference ${n.n}`}
                onClick={(e) => {
                  e.preventDefault();
                  links.showRef(n.id);
                }}
              >
                {n.n}
              </a>
            </sup>
          );
        case 'br':
          return <br key={k} />;
        default:
          return null;
      }
    });

  const items = (list: Item[], key: string) =>
    list.map((it, i) => {
      const k = `${key}.${i}`;
      const Tag = it.term === undefined ? 'li' : it.term ? 'dt' : 'dd';
      return (
        <Tag key={k}>
          {inline(it.c, k)}
          {it.sub.map((b, n) => block(b, `${k}s${n}`))}
        </Tag>
      );
    });

  const block = (b: Block, key: string): ReactNode => {
    switch (b.t) {
      case 'h': {
        const Tag = `h${Math.min(Math.max(b.level, 2), 6)}` as 'h2';
        return (
          <Tag key={key} id={sectionId(b.id)} className="wiki-h" data-level={b.level} data-frag={b.id} data-heading={b.id}>
            {b.text}
          </Tag>
        );
      }
      case 'p':
        return <p key={key}>{inline(b.c, key)}</p>;
      case 'note':
        return (
          <p key={key} className="wiki-note">
            {inline(b.c, key)}
          </p>
        );
      case 'ul':
        return (
          <ul key={key} className="wiki-list">
            {items(b.items, key)}
          </ul>
        );
      case 'ol':
        return (
          <ol key={key} className="wiki-list">
            {items(b.items, key)}
          </ol>
        );
      case 'dl':
        return (
          <dl key={key} className="wiki-dl">
            {items(b.items, key)}
          </dl>
        );
      case 'quote':
        return (
          <blockquote key={key} className="wiki-quote">
            {b.c.map((inner, i) => block(inner, `${key}q${i}`))}
          </blockquote>
        );
      case 'pre':
        return (
          <pre key={key} className="wiki-pre">
            {b.text}
          </pre>
        );
      case 'math':
        return (
          <p key={key} className="wiki-math wiki-math-block">
            {b.text}
          </p>
        );
      case 'table': {
        const table = (
          <div className="wiki-table-wrap" key={key}>
            <table className="wiki-table" data-box={b.box || undefined}>
              {b.caption && <caption>{inline(b.caption, `${key}c`)}</caption>}
              <tbody>
                {b.rows.map((row, r) => (
                  <tr key={r}>
                    {row.map((cell, c) => {
                      const Cell = cell.h ? 'th' : 'td';
                      return (
                        <Cell key={c} colSpan={cell.span}>
                          {inline(cell.c, `${key}r${r}c${c}`)}
                        </Cell>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        );
        // An infobox is folded: the article opens on its words.
        return b.box ? (
          <details key={key} className="wiki-box">
            <summary>Facts at a glance</summary>
            {table}
          </details>
        ) : (
          table
        );
      }
      default:
        return null;
    }
  };

  return { inline, block };
}

export type { Article };
