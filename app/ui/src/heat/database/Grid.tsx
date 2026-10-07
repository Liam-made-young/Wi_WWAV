// The Database tab's grid (docs/ASK.md): a spreadsheet over one table.
//
// Rows are drawn only where they show: every row is the same height, so the
// rows in view come from the scroll position and a table of thousands costs
// what a screenful costs. Cells are selected as a rectangle (click, ⇧click,
// drag, ⇧arrows), typed into in place, copied and pasted as a spreadsheet
// copies and pastes (tab between cells, a line per row), and filled down.
//
// The grid changes nothing itself. An edit, a paste or a fill is handed up
// as a list of cells to set, and the core makes it as one undo step. A cell
// that can't be typed over is drawn locked and says why when tried.
//
// Keys reach it through the frame's router (useTabKeys); it listens to the
// keyboard only inside its own cell editor, and to the clipboard's events.

import {
  type ClipboardEvent,
  forwardRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { HeatKey } from '../frame';
import { AGGS, type Agg, type Cell, cellInput, cellText, type Column, type Edit, isError, isLink, type Query, type Sort } from './api';

export const ROW = 28;
const HEAD = 30;
const GUTTER = 46;
const OVERSCAN = 8;
export const DEFAULT_WIDTH: Record<string, number> = {
  text: 180,
  number: 110,
  bool: 80,
  date: 120,
  datetime: 150,
  relation: 150,
  json: 160,
  formula: 130,
};

export interface GridHandle {
  /** A key the frame didn't take: true when the grid used it. */
  key(e: HeatKey): boolean;
  /** Esc: leave the editor, else drop the selection. True when it did either. */
  escape(): boolean;
  /** The rows and columns the selection covers, by id. */
  selected(): { rows: string[]; columns: string[] } | null;
  /** Puts the selection on a row and brings it into view. */
  reveal(rowId: string): void;
  focus(): void;
}

interface Props {
  /** The columns shown, in order. */
  columns: Column[];
  /** For each shown column, where its cell is in a row. */
  at: number[];
  widths: Record<string, number>;
  frozen: number;
  rows: Query['rows'];
  groups: Query['groups'];
  summary: Query['summary'];
  picked: Record<string, Agg | ''>;
  sorts: Sort[];
  /** Why the whole table can't be typed in, when that is so. */
  lockedWhy: string | null;
  onEdit(edits: Edit[], label?: string): void;
  onSay(text: string): void;
  onLink(table: string, id: string): void;
  onHeader(column: Column, anchor: DOMRect): void;
  onResize(column: string, width: number): void;
  onReorder(from: string, to: string): void;
  onPick(column: string, agg: Agg | ''): void;
  /** The line under the last row that adds one: what it asks for. */
  newRow: { placeholder: string; add(text: string): void } | null;
  onAddColumn(anchor: DOMRect): void;
}

interface Point {
  r: number;
  c: number;
}

type Item = { kind: 'group'; label: string; count: number } | { kind: 'row'; r: number };

const clamp = (n: number, lo: number, hi: number) => Math.min(Math.max(n, lo), hi);

export const Grid = forwardRef<GridHandle, Props>(function Grid(p, ref) {
  const { columns, at, rows, groups } = p;
  const scroller = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(480);
  const [anchor, setAnchor] = useState<Point | null>(null);
  const [focus, setFocus] = useState<Point | null>(null);
  const [editing, setEditing] = useState<{ r: number; c: number; text: string } | null>(null);
  const dragging = useRef(false);
  const [draft, setDraft] = useState('');

  const widthOf = useCallback((c: Column) => p.widths[c.id] ?? DEFAULT_WIDTH[c.type] ?? 150, [p.widths]);
  const lefts = useMemo(() => {
    const out: number[] = [];
    let x = GUTTER;
    for (const c of columns) {
      out.push(x);
      x += widthOf(c);
    }
    return { at: out, total: x };
  }, [columns, widthOf]);

  // Group headings sit between the rows as rows of their own.
  const items = useMemo<Item[]>(() => {
    if (groups.length === 0) return rows.map((_, r) => ({ kind: 'row', r }));
    const out: Item[] = [];
    for (const g of groups) {
      out.push({ kind: 'group', label: g.label || '(empty)', count: g.count });
      for (let r = g.start; r < g.start + g.count; r++) out.push({ kind: 'row', r });
    }
    return out;
  }, [rows, groups]);
  const itemOfRow = useMemo(() => {
    const map: number[] = [];
    items.forEach((it, i) => {
      if (it.kind === 'row') map[it.r] = i;
    });
    return map;
  }, [items]);

  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const measure = () => setHeight(el.clientHeight);
    measure();
    const seen = new ResizeObserver(measure);
    seen.observe(el);
    return () => seen.disconnect();
  }, []);

  // A table that changed under the selection keeps it inside what is there.
  useEffect(() => {
    const fit = (pt: Point | null) =>
      pt && rows.length > 0 && columns.length > 0
        ? { r: clamp(pt.r, 0, rows.length - 1), c: clamp(pt.c, 0, columns.length - 1) }
        : null;
    setAnchor((a) => fit(a));
    setFocus((f) => fit(f));
  }, [rows.length, columns.length]);

  const first = Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN);
  const last = Math.min(items.length, Math.ceil((scrollTop + height) / ROW) + OVERSCAN);

  const rect = anchor && focus
    ? { r0: Math.min(anchor.r, focus.r), r1: Math.max(anchor.r, focus.r), c0: Math.min(anchor.c, focus.c), c1: Math.max(anchor.c, focus.c) }
    : null;

  const show = useCallback(
    (pt: Point) => {
      const el = scroller.current;
      if (!el) return;
      const y = (itemOfRow[pt.r] ?? pt.r) * ROW;
      if (y < el.scrollTop) el.scrollTop = y;
      else if (y + ROW > el.scrollTop + el.clientHeight - HEAD - ROW * 2) el.scrollTop = y + ROW * 3 + HEAD - el.clientHeight;
      const x = lefts.at[pt.c] ?? 0;
      const w = columns[pt.c] ? widthOf(columns[pt.c]) : 0;
      const frozenWidth = (lefts.at[p.frozen] ?? GUTTER) - 0;
      if (pt.c >= p.frozen) {
        if (x - frozenWidth < el.scrollLeft) el.scrollLeft = x - frozenWidth;
        else if (x + w > el.scrollLeft + el.clientWidth) el.scrollLeft = x + w - el.clientWidth;
      }
    },
    [itemOfRow, lefts, columns, widthOf, p.frozen],
  );

  const select = (pt: Point, extend: boolean) => {
    setFocus(pt);
    if (!extend || !anchor) setAnchor(pt);
    show(pt);
  };

  const whyLocked = (c: Column) => p.lockedWhy ?? (c.locked ? (c.why ?? 'This can’t be typed over.') : null);

  const startEdit = (pt: Point, typed?: string) => {
    const col = columns[pt.c];
    const row = rows[pt.r];
    if (!col || !row) return;
    const why = whyLocked(col);
    if (why) return p.onSay(why);
    const cell = row.cells[at[pt.c]];
    // A checkbox has no editor: it flips.
    if (col.type === 'bool') return p.onEdit([{ row: row.id, column: col.id, value: !(cell === true) }]);
    const text = typed ?? cellInput(cell);
    setDraft(text);
    setEditing({ r: pt.r, c: pt.c, text });
  };

  const commit = (move: Point | null) => {
    if (!editing) return;
    const col = columns[editing.c];
    const row = rows[editing.r];
    setEditing(null);
    scroller.current?.focus({ preventScroll: true });
    if (col && row && draft !== editing.text) p.onEdit([{ row: row.id, column: col.id, value: draft }]);
    if (move && rows.length > 0) {
      select({ r: clamp(editing.r + move.r, 0, rows.length - 1), c: clamp(editing.c + move.c, 0, columns.length - 1) }, false);
    }
  };

  const rectEdits = (value: (r: number, c: number) => unknown | undefined): Edit[] => {
    if (!rect) return [];
    const out: Edit[] = [];
    for (let r = rect.r0; r <= rect.r1; r++) {
      for (let c = rect.c0; c <= rect.c1; c++) {
        const v = value(r, c);
        if (v !== undefined && rows[r] && columns[c]) out.push({ row: rows[r].id, column: columns[c].id, value: v });
      }
    }
    return out;
  };

  /** Every cell that can be typed over in the selection: locked ones are passed over. */
  const editable = (c: number) => !whyLocked(columns[c]);

  const clear = () => {
    const edits = rectEdits((_r, c) => (editable(c) ? (columns[c].type === 'bool' ? false : '') : undefined));
    if (edits.length === 0) return rect && p.onSay(whyLocked(columns[rect.c0]) ?? 'Nothing here can be cleared.');
    p.onEdit(edits, edits.length > 1 ? 'clear cells' : undefined);
  };

  const fillDown = () => {
    if (!rect || rect.r1 === rect.r0) return p.onSay('Select the cell to copy and the cells under it, then fill down.');
    const edits = rectEdits((r, c) => {
      if (r === rect.r0 || !editable(c)) return undefined;
      const top = rows[rect.r0].cells[at[c]];
      return columns[c].type === 'bool' ? top === true : isLink(top) ? top.id : cellInput(top);
    });
    if (edits.length) p.onEdit(edits, 'fill down');
  };

  const copyText = (): string | null => {
    if (!rect) return null;
    const lines: string[] = [];
    for (let r = rect.r0; r <= rect.r1; r++) {
      const line: string[] = [];
      for (let c = rect.c0; c <= rect.c1; c++) line.push(cellText(rows[r].cells[at[c]]).replace(/[\t\r\n]+/g, ' '));
      lines.push(line.join('\t'));
    }
    return lines.join('\n');
  };

  const onCopy = (e: ClipboardEvent) => {
    if (editing) return;
    const text = copyText();
    if (text === null) return;
    e.clipboardData.setData('text/plain', text);
    e.preventDefault();
  };

  const onCut = (e: ClipboardEvent) => {
    if (editing) return;
    onCopy(e);
    clear();
  };

  const onPaste = (e: ClipboardEvent) => {
    if (editing || !rect) return;
    const text = e.clipboardData.getData('text/plain');
    if (!text) return;
    e.preventDefault();
    const block = text.replace(/\r\n?/g, '\n').replace(/\n$/, '').split('\n').map((l) => l.split('\t'));
    const one = block.length === 1 && block[0].length === 1;
    const edits: Edit[] = [];
    if (one) {
      // One value over a selection fills the selection.
      for (const edit of rectEdits((_r, c) => (editable(c) ? block[0][0] : undefined))) edits.push(edit);
    } else {
      block.forEach((line, dr) => {
        line.forEach((value, dc) => {
          const r = rect.r0 + dr;
          const c = rect.c0 + dc;
          if (rows[r] && columns[c] && editable(c)) edits.push({ row: rows[r].id, column: columns[c].id, value });
        });
      });
      const r1 = clamp(rect.r0 + block.length - 1, 0, rows.length - 1);
      const c1 = clamp(rect.c0 + Math.max(...block.map((l) => l.length)) - 1, 0, columns.length - 1);
      setAnchor({ r: rect.r0, c: rect.c0 });
      setFocus({ r: r1, c: c1 });
    }
    if (edits.length === 0) return p.onSay('Nothing here can be pasted over.');
    p.onEdit(edits, 'paste');
  };

  useImperativeHandle(ref, () => ({
    focus: () => scroller.current?.focus({ preventScroll: true }),
    selected() {
      if (!rect) return null;
      const out = { rows: [] as string[], columns: [] as string[] };
      for (let r = rect.r0; r <= rect.r1; r++) if (rows[r]) out.rows.push(rows[r].id);
      for (let c = rect.c0; c <= rect.c1; c++) if (columns[c]) out.columns.push(columns[c].id);
      return out;
    },
    reveal(rowId) {
      const r = rows.findIndex((row) => row.id === rowId);
      if (r < 0) return;
      setAnchor({ r, c: 0 });
      setFocus({ r, c: Math.max(0, columns.length - 1) });
      const el = scroller.current;
      if (el) el.scrollTop = Math.max(0, (itemOfRow[r] ?? r) * ROW - el.clientHeight / 3);
    },
    escape() {
      if (editing) {
        setEditing(null);
        scroller.current?.focus({ preventScroll: true });
        return true;
      }
      if (anchor) {
        setAnchor(null);
        setFocus(null);
        return true;
      }
      return false;
    },
    key(e) {
      if (editing || rows.length === 0 || columns.length === 0) return false;
      const pt = focus;
      const lastR = rows.length - 1;
      const lastC = columns.length - 1;
      if (e.command && !e.alt) {
        if (e.code === 'KeyA') {
          setAnchor({ r: 0, c: 0 });
          setFocus({ r: lastR, c: lastC });
          return true;
        }
        if (e.code === 'KeyD' && pt) {
          fillDown();
          return true;
        }
        if (!pt) return false;
        const jump: Record<string, Point> = {
          ArrowUp: { r: 0, c: pt.c },
          ArrowDown: { r: lastR, c: pt.c },
          ArrowLeft: { r: pt.r, c: 0 },
          ArrowRight: { r: pt.r, c: lastC },
        };
        if (jump[e.key]) {
          select(jump[e.key], e.shift);
          return true;
        }
        // ⌘C, ⌘V and ⌘X are the clipboard's own events.
        return false;
      }
      if (e.alt) return false;
      if (!pt) {
        // Nothing selected: an arrow starts at the first cell, and other keys are the frame's.
        if (e.key.startsWith('Arrow')) {
          select({ r: 0, c: 0 }, false);
          return true;
        }
        return false;
      }
      const page = Math.max(1, Math.floor(height / ROW) - 3);
      const to = (r: number, c: number, extend = e.shift) => {
        select({ r: clamp(r, 0, lastR), c: clamp(c, 0, lastC) }, extend);
        return true;
      };
      switch (e.key) {
        case 'ArrowUp':
          return to(pt.r - 1, pt.c);
        case 'ArrowDown':
          return to(pt.r + 1, pt.c);
        case 'ArrowLeft':
          return to(pt.r, pt.c - 1);
        case 'ArrowRight':
          return to(pt.r, pt.c + 1);
        case 'Tab':
          return to(pt.r, pt.c + (e.shift ? -1 : 1), false);
        case 'Home':
          return to(pt.r, 0);
        case 'End':
          return to(pt.r, lastC);
        case 'PageUp':
          return to(pt.r - page, pt.c);
        case 'PageDown':
          return to(pt.r + page, pt.c);
        case 'Enter':
        case 'F2':
          startEdit(pt);
          return true;
        case 'Backspace':
        case 'Delete':
          clear();
          return true;
        default:
          // A letter or a digit starts typing over the cell.
          if (e.key.length === 1 && e.key !== ' ' && !e.repeat) {
            startEdit(pt, e.key);
            return true;
          }
          return false;
      }
    },
  }));

  const down = (e: ReactPointerEvent, pt: Point) => {
    if (e.button !== 0) return;
    if (editing) commit(null);
    dragging.current = true;
    select(pt, e.shiftKey);
    scroller.current?.focus({ preventScroll: true });
  };
  useEffect(() => {
    const up = () => (dragging.current = false);
    window.addEventListener('pointerup', up);
    return () => window.removeEventListener('pointerup', up);
  }, []);

  const editorKey = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      commit({ r: e.shiftKey ? -1 : 1, c: 0 });
    } else if (e.key === 'Tab') {
      e.preventDefault();
      commit({ r: 0, c: e.shiftKey ? -1 : 1 });
    }
  };

  const resizing = useRef<{ id: string; x: number; w: number } | null>(null);
  const resizeDown = (e: ReactPointerEvent, c: Column) => {
    e.preventDefault();
    e.stopPropagation();
    resizing.current = { id: c.id, x: e.clientX, w: widthOf(c) };
    const move = (m: PointerEvent) => {
      const r = resizing.current;
      if (r) p.onResize(r.id, clamp(Math.round(r.w + m.clientX - r.x), 48, 720));
    };
    const up = () => {
      resizing.current = null;
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };

  const frozenStyle = (c: number) =>
    c < p.frozen ? { position: 'sticky' as const, left: lefts.at[c], zIndex: 2 } : undefined;
  const sortOf = (id: string) => p.sorts.find((s) => s.column === id);
  const canvasHeight = HEAD + items.length * ROW + (p.newRow ? ROW : 0) + ROW;
  const [adding, setAdding] = useState('');

  return (
    <div
      ref={scroller}
      className="db-grid"
      role="grid"
      aria-rowcount={rows.length}
      aria-colcount={columns.length}
      tabIndex={0}
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
      onCopy={onCopy}
      onCut={onCut}
      onPaste={onPaste}
    >
      <div className="db-canvas" style={{ width: lefts.total + 40, height: canvasHeight }}>
        <div className="db-head" role="row" style={{ width: lefts.total + 40, height: HEAD }}>
          <div className="db-corner" style={{ width: GUTTER }} />
          {columns.map((c, i) => {
            const sort = sortOf(c.id);
            return (
              <div
                key={c.id}
                role="columnheader"
                className="db-th"
                data-frozen={i < p.frozen || undefined}
                data-last-frozen={i === p.frozen - 1 || undefined}
                style={{ width: widthOf(c), ...frozenStyle(i), zIndex: i < p.frozen ? 5 : 3 }}
                title={c.locked ? `${c.name}. ${c.why ?? ''}` : c.name}
                draggable
                onDragStart={(e) => e.dataTransfer.setData('application/x-db-column', c.id)}
                onDragOver={(e) => e.dataTransfer.types.includes('application/x-db-column') && e.preventDefault()}
                onDrop={(e) => {
                  const from = e.dataTransfer.getData('application/x-db-column');
                  if (from && from !== c.id) p.onReorder(from, c.id);
                }}
                onClick={(e) => p.onHeader(c, e.currentTarget.getBoundingClientRect())}
              >
                <span className="db-th-kind" aria-hidden="true">
                  {c.type === 'formula' ? 'ƒ' : c.locked ? '🔒' : ''}
                </span>
                <span className="db-th-name">{c.name}</span>
                {sort && (
                  <span className="db-th-sort" aria-label={sort.dir === 'asc' ? 'sorted up' : 'sorted down'}>
                    {sort.dir === 'asc' ? '▲' : '▼'}
                  </span>
                )}
                <span className="db-resize" onPointerDown={(e) => resizeDown(e, c)} onClick={(e) => e.stopPropagation()} />
              </div>
            );
          })}
          <button
            type="button"
            className="db-th db-add-column"
            title="Add a column"
            aria-label="Add a column"
            onClick={(e) => p.onAddColumn(e.currentTarget.getBoundingClientRect())}
          >
            +
          </button>
        </div>

        {items.slice(first, last).map((it, n) => {
          const i = first + n;
          const top = HEAD + i * ROW;
          if (it.kind === 'group') {
            return (
              <div key={`g${i}`} className="db-group" style={{ transform: `translateY(${top}px)`, width: lefts.total }}>
                <span className="db-group-name">{it.label}</span>
                <span className="db-group-count" data-text="secondary">
                  {it.count}
                </span>
              </div>
            );
          }
          const row = rows[it.r];
          const inRows = rect && it.r >= rect.r0 && it.r <= rect.r1;
          return (
            <div
              key={row.id}
              className="db-row"
              role="row"
              aria-rowindex={it.r + 1}
              data-odd={it.r % 2 === 1 || undefined}
              style={{ transform: `translateY(${top}px)`, width: lefts.total }}
            >
              <div
                className="db-gutter"
                style={{ width: GUTTER }}
                data-selected={(inRows && rect!.c0 === 0 && rect!.c1 === columns.length - 1) || undefined}
                onPointerDown={(e) => {
                  if (e.button !== 0) return;
                  setAnchor(e.shiftKey && anchor ? { r: anchor.r, c: 0 } : { r: it.r, c: 0 });
                  setFocus({ r: it.r, c: columns.length - 1 });
                  scroller.current?.focus({ preventScroll: true });
                }}
              >
                {it.r + 1}
              </div>
              {columns.map((c, ci) => {
                const cell = row.cells[at[ci]] as Cell;
                const selected = inRows && ci >= rect!.c0 && ci <= rect!.c1;
                const active = focus?.r === it.r && focus?.c === ci;
                const isEditing = editing?.r === it.r && editing?.c === ci;
                const numeric = typeof cell === 'number';
                return (
                  <div
                    key={c.id}
                    role="gridcell"
                    className="db-cell"
                    data-type={c.type}
                    data-selected={selected || undefined}
                    data-active={active || undefined}
                    data-locked={c.locked || !!p.lockedWhy || undefined}
                    data-numeric={numeric || undefined}
                    data-error={isError(cell) || undefined}
                    data-frozen={ci < p.frozen || undefined}
                    data-last-frozen={ci === p.frozen - 1 || undefined}
                    style={{ width: widthOf(c), ...frozenStyle(ci) }}
                    title={isError(cell) ? cell.message : undefined}
                    onPointerDown={(e) => down(e, { r: it.r, c: ci })}
                    onPointerEnter={() => dragging.current && setFocus({ r: it.r, c: ci })}
                    onDoubleClick={() => startEdit({ r: it.r, c: ci })}
                  >
                    {isEditing ? (
                      <input
                        className="db-editor"
                        autoFocus
                        value={draft}
                        aria-label={`${c.name}, row ${it.r + 1}`}
                        list={c.type === 'relation' || c.options?.length ? `db-options-${c.id}` : undefined}
                        onChange={(e) => setDraft(e.target.value)}
                        onKeyDown={editorKey}
                        onBlur={() => commit(null)}
                      />
                    ) : c.type === 'bool' ? (
                      <span
                        className="db-check"
                        role="checkbox"
                        aria-checked={cell === true}
                        data-on={cell === true || undefined}
                        onClick={(e) => {
                          e.stopPropagation();
                          select({ r: it.r, c: ci }, false);
                          startEdit({ r: it.r, c: ci });
                        }}
                      />
                    ) : isLink(cell) && c.relation ? (
                      <a
                        className="db-link"
                        href="#"
                        title={`Go to ${cell.label}`}
                        onPointerDown={(e) => e.stopPropagation()}
                        onClick={(e) => {
                          e.preventDefault();
                          p.onLink(c.relation!, cell.id);
                        }}
                      >
                        {cell.label}
                      </a>
                    ) : (
                      <span className="db-text">{cellText(cell)}</span>
                    )}
                  </div>
                );
              })}
            </div>
          );
        })}

        {p.newRow && (
          <div className="db-row db-new" style={{ transform: `translateY(${HEAD + items.length * ROW}px)`, width: lefts.total }}>
            <div className="db-gutter" style={{ width: GUTTER }}>
              +
            </div>
            <input
              className="db-new-input"
              placeholder={p.newRow.placeholder}
              aria-label={p.newRow.placeholder}
              value={adding}
              onChange={(e) => setAdding(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && adding.trim()) {
                  p.newRow!.add(adding.trim());
                  setAdding('');
                }
              }}
            />
          </div>
        )}

        <div className="db-summary" role="row" style={{ width: lefts.total + 40, height: ROW }}>
          <div className="db-corner" style={{ width: GUTTER }}>
            Σ
          </div>
          {columns.map((c, i) => {
            const agg = p.picked[c.id] ?? '';
            const value = agg ? p.summary[c.id]?.[agg] : undefined;
            const offered = AGGS.filter((a) => p.summary[c.id]?.[a.id] !== undefined);
            return (
              <label
                key={c.id}
                className="db-total"
                data-last-frozen={i === p.frozen - 1 || undefined}
                style={{ width: widthOf(c), ...frozenStyle(i) }}
              >
                <select
                  className="db-total-pick"
                  aria-label={`Total for ${c.name}`}
                  value={agg}
                  onChange={(e) => p.onPick(c.id, e.target.value as Agg | '')}
                >
                  <option value="">—</option>
                  {offered.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name}
                    </option>
                  ))}
                </select>
                {agg && (
                  <span className="db-total-value">
                    <span data-text="secondary">{AGGS.find((a) => a.id === agg)?.name}</span> {cellText((value ?? null) as Cell)}
                  </span>
                )}
              </label>
            );
          })}
        </div>
      </div>

      {columns
        .filter((c) => c.type === 'relation' || c.options?.length)
        .map((c) => {
          const seen = new Set<string>(c.options ?? []);
          const idx = at[columns.indexOf(c)];
          for (const r of rows) {
            const cell = r.cells[idx];
            if (isLink(cell)) seen.add(cell.label);
            if (seen.size > 200) break;
          }
          return (
            <datalist key={c.id} id={`db-options-${c.id}`}>
              {[...seen].map((s) => (
                <option key={s} value={s} />
              ))}
            </datalist>
          );
        })}
    </div>
  );
});
