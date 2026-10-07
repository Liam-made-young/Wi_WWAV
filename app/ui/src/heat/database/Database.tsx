// The Database tab (docs/ASK.md): Learn's own records, and tables of your
// own, as a spreadsheet. The sidebar lists every table and its saved views;
// the grid shows one, with a toolbar to search, filter, sort, group and pick
// columns; a pivot and charts open under it.
//
// The core does the reading and the arithmetic (`db.query`, `db.pivot`,
// `db.chart`) and makes every change (`db.cells.set` and the rest); this
// file holds what is showing and asks. It reads again whenever Learn
// changes, so an edit made in another tab, by Claude, or by ⌘Z shows here.

import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { setScreen } from '../../ask/context';
import { navigate, onNavigate } from '../../ask/nav';
import { useCoreEvent } from '../../shell/hooks';
import { keys } from '../../shell/platform';
import { sentenceOf } from '../fmt';
import { type TabId, useSidebarSlot, useTabActs, useTabKeys, useTabScope, useTabs } from '../frame';
import { useHeat } from '../store';
import {
  AGGS,
  type Agg,
  type ChartData,
  type ChartSpec,
  type Column,
  db,
  type Edit,
  type Filter,
  OPS,
  type Pivot,
  type PivotSpec,
  type Query,
  type Spec,
  type TableInfo,
  type View,
  type Written,
} from './api';
import { Chart, MAX_SERIES } from './Chart';
import './database.css';
import { Grid, type GridHandle } from './Grid';

/** Fields a table opens without: they are there, under Columns, when wanted. */
const QUIET = new Set([
  'id', 'sourceId', 'claudeReason', 'estBy', 'estReason', 'adjustMin', 'tag', 'link', 'source', 'resultType',
  'resultId', 'triagedAt', 'origin', 'room', 'view', 'public', 'groupKind', 'groupLabel', 'persona', 'hue',
  'parentTaskId', 'milestoneId', 'group', 'rrule', 'doneAt', 'postedAt', 'order',
]);

interface Meta {
  tables: TableInfo[];
  views: View[];
  layouts: Record<string, Spec>;
  functions: string[];
}

type Pop =
  | { kind: 'column'; column: Column; x: number; y: number }
  | { kind: 'add'; x: number; y: number; edit?: Column }
  | { kind: 'filter' | 'sort' | 'columns' | 'save' | 'table'; x: number; y: number }
  | null;

const below = (r: DOMRect) => ({ x: r.left, y: r.bottom + 4 });

export function Database() {
  const { active } = useTabScope();
  const slot = useSidebarSlot();
  const { say } = useHeat();
  const { setTab } = useTabs();
  const [meta, setMeta] = useState<Meta | null>(null);
  const [tableId, setTableId] = useState('task');
  const [viewId, setViewId] = useState<string | null>(null);
  const [spec, setSpecState] = useState<Spec>({});
  const [dirty, setDirty] = useState(false);
  const [data, setData] = useState<Query | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The pivot and the chart open under the grid, side by side when both are wanted.
  const [panels, setPanels] = useState({ pivot: false, chart: false });
  const togglePanel = (which: 'pivot' | 'chart') => setPanels((p) => ({ ...p, [which]: !p[which] }));
  const [pop, setPop] = useState<Pop>(null);
  const grid = useRef<GridHandle>(null);
  const searchField = useRef<HTMLInputElement>(null);
  const file = useRef<HTMLInputElement>(null);
  const host = useRef<HTMLDivElement>(null);
  const reveal = useRef<string | null>(null);
  const stale = useRef(true);
  const asked = useRef(0);
  const settled = useRef<string | null>(null);

  const loadMeta = useCallback(
    () =>
      db.tables().then(
        (m) => {
          setMeta(m);
          return m;
        },
        (e) => {
          setError(sentenceOf(e));
          return null;
        },
      ),
    [],
  );

  // What the core is asked for: everything in the view that changes the rows.
  const asking = useMemo(
    () => JSON.stringify([tableId, spec.filters, spec.match, spec.search, spec.sorts, spec.group]),
    [tableId, spec.filters, spec.match, spec.search, spec.sorts, spec.group],
  );
  const specRef = useRef(spec);
  specRef.current = spec;

  const load = useCallback(() => {
    const mine = ++asked.current;
    return db.query(tableId, specRef.current).then(
      (q) => {
        if (mine !== asked.current) return;
        setData(q);
        setError(null);
        stale.current = false;
      },
      (e) => mine === asked.current && setError(sentenceOf(e)),
    );
  }, [tableId]);

  // Nothing is read until the tab is first shown; after that it keeps up.
  useEffect(() => {
    if (!active) return;
    if (!meta) void loadMeta();
    void load();
  }, [active, asking]); // eslint-disable-line react-hooks/exhaustive-deps

  const again = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useCoreEvent<{ kinds?: string[] }>('heat', () => {
    stale.current = true;
    if (!active) return;
    clearTimeout(again.current);
    again.current = setTimeout(() => {
      void load();
      void loadMeta();
    }, 60);
  });
  useEffect(() => {
    if (active && stale.current && meta) {
      void load();
      void loadMeta();
    }
  }, [active]); // eslint-disable-line react-hooks/exhaustive-deps

  // A table opened for the first time hides its housekeeping columns.
  useEffect(() => {
    if (!data || data.table.id !== tableId || settled.current === tableId) return;
    settled.current = tableId;
    if (spec.hidden === undefined && !viewId) {
      const hidden = data.columns.filter((c) => !c.added && (QUIET.has(c.id) || c.type === 'json')).map((c) => c.id);
      // What a row is called comes first and stays put; numbers start with their sum.
      const first = data.columns.find((c) => ['title', 'name', 'code', 'subject', 'text'].includes(c.id));
      const summary: Spec['summary'] = {};
      for (const c of data.columns) if (c.type === 'number' && !c.relation) summary[c.id] = 'sum';
      if (first) summary[first.id] = 'count';
      setSpecState((s) =>
        s.hidden === undefined
          ? { ...s, hidden, order: first ? [first.id] : s.order, frozen: 1, summary: { ...summary, ...s.summary } }
          : s,
      );
    }
  }, [data, tableId, spec.hidden, viewId]);

  const saveLayout = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const setSpec = useCallback(
    (change: (s: Spec) => Spec) => {
      setSpecState((s) => {
        const next = change(s);
        if (viewId) setDirty(true);
        else {
          clearTimeout(saveLayout.current);
          saveLayout.current = setTimeout(() => void db.setLayout(tableId, next).catch(() => {}), 400);
        }
        return next;
      });
    },
    [viewId, tableId],
  );

  const openTable = useCallback(
    (id: string, view?: View | null, m: Meta | null = meta) => {
      setPop(null);
      setTableId(id);
      setViewId(view?.id ?? null);
      setDirty(false);
      settled.current = view || m?.layouts[id] ? id : null;
      setSpecState(view?.spec ?? m?.layouts[id] ?? {});
      setData((d) => (d?.table.id === id ? d : null));
      setPanels({ pivot: !!view?.spec.pivot?.rows.length, chart: !!view?.spec.charts?.length });
    },
    [meta],
  );

  // A link to a row or a table, from the prompt box, a relation, or another tab.
  useEffect(
    () =>
      onNavigate((t) => {
        if (t.what === 'tab') return void setTab(t.tab as TabId);
        if (t.what !== 'row' && t.what !== 'table') return;
        setTab('database');
        void (meta ? Promise.resolve(meta) : loadMeta()).then((m) => {
          const table = m?.tables.find((x) => x.id === t.table || x.name.toLowerCase() === t.table.toLowerCase());
          if (!table) return say(`There is no table called ‘${t.table}’.`);
          if (t.what === 'table') {
            const view = t.view ? m?.views.find((v) => v.id === t.view || v.name === t.view) : null;
            return openTable(table.id, view, m);
          }
          reveal.current = t.id;
          // The row is shown whatever the table was filtered to.
          if (table.id !== tableId || viewId) openTable(table.id, null, m);
          setSpecState((s) => ({ ...s, filters: [], search: '' }));
          void load();
        });
      }),
    [meta, tableId, viewId, loadMeta, openTable, setTab, say, load],
  );
  useEffect(() => {
    if (!data || !reveal.current || data.table.id !== tableId) return;
    const id = reveal.current;
    if (data.rows.some((r) => r.id === id)) {
      reveal.current = null;
      requestAnimationFrame(() => {
        grid.current?.reveal(id);
        grid.current?.focus();
      });
    }
  }, [data, tableId]);

  const columns = data?.table.id === tableId ? data.columns : [];
  // The view's own order first, then any column it doesn't place, without the hidden ones.
  const shown = useMemo(() => {
    const hidden = new Set(spec.hidden ?? []);
    const placed = (spec.order ?? []).map((id) => columns.find((c) => c.id === id)).filter((c): c is Column => !!c);
    const rest = columns.filter((c) => !placed.includes(c));
    return [...placed, ...rest].filter((c) => !hidden.has(c.id));
  }, [columns, spec.hidden, spec.order]);
  const at = useMemo(() => shown.map((c) => columns.indexOf(c)), [shown, columns]);
  const table = meta?.tables.find((t) => t.id === tableId);
  const views = (meta?.views ?? []).filter((v) => v.table === tableId);
  const view = views.find((v) => v.id === viewId) ?? null;
  const isUser = (data?.table.origin ?? table?.origin) === 'user';
  const nameOf = (id: string) => columns.find((c) => c.id === id)?.name ?? id;

  const wrote = useCallback(
    async (w: Promise<Written | unknown>, done?: (r: Written) => void) => {
      try {
        const r = (await w) as Written;
        const failed = r?.failed ?? [];
        if (failed.length === 1) say(failed[0].message);
        else if (failed.length > 1) say(`${failed.length} cells couldn’t be changed. The first: ${failed[0].message}`);
        await Promise.all([load(), loadMeta()]);
        if (r) done?.(r);
      } catch (e) {
        say(sentenceOf(e));
      }
    },
    [load, loadMeta, say],
  );

  const edit = (edits: Edit[], label?: string) => void wrote(db.setCells(tableId, edits, label));

  // The line that adds a row asks for what the row is called.
  const labelColumn = useMemo(() => {
    if (!data || data.table.locked || data.table.origin === 'derived') return null;
    const names = ['title', 'name', 'code', 'text', 'date'];
    return (
      columns.find((c) => names.includes(c.id) && !c.locked) ??
      (isUser ? columns.find((c) => !c.locked && c.type !== 'formula') : undefined) ??
      null
    );
  }, [data, columns, isUser]);
  const addRow = (text: string) => {
    if (!labelColumn) return;
    void wrote(db.addRows(tableId, [{ [labelColumn.id]: text }]), (r) => {
      if (r.ids?.[0]) reveal.current = r.ids[0];
      void load();
    });
  };

  const deleteSelected = () => {
    const picked = grid.current?.selected();
    if (!picked || picked.rows.length === 0) return say('Select the rows to delete first.');
    void wrote(db.deleteRows(tableId, picked.rows));
  };

  const newTable = () => {
    const taken = new Set((meta?.tables ?? []).map((t) => t.name.toLowerCase()));
    let name = 'Untitled table';
    for (let n = 2; taken.has(name.toLowerCase()); n++) name = `Untitled table ${n}`;
    db.createTable(name).then(
      async (r) => {
        const m = await loadMeta();
        openTable(r.table.id, null, m);
        say(`Made ‘${name}’. ${keys('⌘Z')} takes it back.`);
      },
      (e) => say(sentenceOf(e)),
    );
  };

  const importCsv = (f: File) => {
    void f.text().then((csv) =>
      db.importCsv(f.name, csv).then(
        async (r) => {
          const m = await loadMeta();
          openTable(r.table.id, null, m);
          say(`Imported ${r.rows} rows into ‘${r.table.name}’.`);
        },
        (e) => say(sentenceOf(e)),
      ),
    );
  };

  const exportCsv = () =>
    db.exportCsv(tableId, spec).then(
      (r) => say(r.file ? `Saved ${r.file} to Downloads: ${r.rows} rows.` : `Exported ${r.rows} rows.`),
      (e) => say(sentenceOf(e)),
    );

  const saveView = (name: string) =>
    db.saveView(tableId, name, spec, view && view.name === name ? view.id : undefined).then(
      async (r) => {
        await loadMeta();
        setViewId(r.view.id);
        setDirty(false);
        setPop(null);
        say(`Saved the view ‘${name}’.`);
      },
      (e) => say(sentenceOf(e)),
    );

  useTabActs({
    plus: labelColumn
      ? { run: () => host.current?.querySelector<HTMLInputElement>('.db-new-input')?.focus() }
      : { run: () => {}, disabled: data?.table.why ?? 'Rows are added where these records are made.' },
    // No secondary act here: ⇧click extends the selection, as in any spreadsheet,
    // and the frame gives ⇧click to a tab's secondary act when it has one.
    count: data ? (data.total === data.all ? `${data.all} rows` : `${data.total} of ${data.all} rows`) : null,
  });
  useTabKeys({
    key: (e) => (pop ? false : (grid.current?.key(e) ?? false)),
    escape: () => {
      if (pop) {
        setPop(null);
        return true;
      }
      return grid.current?.escape() ?? false;
    },
    filter: () => searchField.current?.focus(),
  });

  // What the prompt box is told is on screen here.
  useEffect(() => {
    setScreen(
      'database',
      active && data
        ? () => {
            const picked = grid.current?.selected();
            return {
              table: data.table.name,
              tableId: data.table.id,
              view: view?.name,
              columns: shown.map((c) => c.name),
              filters: spec.filters?.length ? spec.filters.map((f) => ({ ...f, column: nameOf(f.column) })) : undefined,
              rowsShown: data.total,
              selectedRows: picked?.rows.slice(0, 60),
              selectedColumns: picked?.columns.map(nameOf),
            };
          }
        : null,
    );
  });

  const filters = spec.filters ?? [];
  const sorts = spec.sorts ?? [];
  const chart = spec.charts?.[0] ?? null;

  return (
    <div className="db" ref={host}>
      {slot &&
        createPortal(
          <nav className="db-side" aria-label="Tables">
            <h2 className="heat-side-heading" data-text="secondary">
              Tables
            </h2>
            {(meta?.tables ?? []).map((t) => (
              <div key={t.id}>
                <button
                  type="button"
                  className="heat-side-row"
                  data-dense
                  aria-current={t.id === tableId && !viewId ? 'true' : undefined}
                  onClick={() => openTable(t.id)}
                >
                  <span className="heat-side-name">{t.name}</span>
                  <span className="heat-side-count" data-text="secondary">
                    {t.count}
                  </span>
                </button>
                {(meta?.views ?? [])
                  .filter((v) => v.table === t.id)
                  .map((v) => (
                    <button
                      key={v.id}
                      type="button"
                      className="heat-side-row db-side-view"
                      data-dense
                      aria-current={v.id === viewId ? 'true' : undefined}
                      onClick={() => openTable(t.id, v)}
                    >
                      <span className="heat-side-name">{v.name}</span>
                    </button>
                  ))}
              </div>
            ))}
            <button type="button" className="heat-side-row heat-side-new" data-dense onClick={newTable}>
              New table…
            </button>
            <button type="button" className="heat-side-row heat-side-new" data-dense onClick={() => file.current?.click()}>
              Import CSV…
            </button>
          </nav>,
          slot,
        )}
      <input
        ref={file}
        type="file"
        accept=".csv,text/csv"
        hidden
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) importCsv(f);
          e.target.value = '';
        }}
      />

      <div className="db-bar" role="toolbar" aria-label="View">
        <button
          type="button"
          className="db-title"
          title={isUser ? 'Rename or delete this table' : undefined}
          disabled={!isUser}
          onClick={(e) => setPop({ kind: 'table', ...below(e.currentTarget.getBoundingClientRect()) })}
        >
          {data?.table.name ?? table?.name ?? 'Database'}
          {view && <span className="db-title-view"> · {view.name}{dirty ? ' (edited)' : ''}</span>}
        </button>
        <input
          ref={searchField}
          className="db-search"
          type="search"
          placeholder={`Search this table (${keys('⌘F')})`}
          aria-label="Search this table"
          value={spec.search ?? ''}
          onChange={(e) => setSpec((s) => ({ ...s, search: e.target.value }))}
        />
        <BarButton on={filters.length > 0} onClick={(r) => setPop({ kind: 'filter', ...below(r) })}>
          Filter{filters.length ? ` (${filters.length})` : ''}
        </BarButton>
        <BarButton on={sorts.length > 0 || !!spec.group} onClick={(r) => setPop({ kind: 'sort', ...below(r) })}>
          Sort{sorts.length ? ` (${sorts.length})` : ''}
          {spec.group ? ' · grouped' : ''}
        </BarButton>
        <BarButton on={(spec.hidden?.length ?? 0) > 0} onClick={(r) => setPop({ kind: 'columns', ...below(r) })}>
          Columns
        </BarButton>
        <span className="db-bar-gap" />
        <BarButton on={panels.pivot} onClick={() => togglePanel('pivot')}>
          Pivot
        </BarButton>
        <BarButton on={panels.chart} onClick={() => togglePanel('chart')}>
          Chart
        </BarButton>
        <BarButton onClick={(r) => setPop({ kind: 'save', ...below(r) })}>{view && dirty ? 'Save view*' : 'Save view'}</BarButton>
        <BarButton onClick={() => void exportCsv()}>Export CSV</BarButton>
        <BarButton onClick={deleteSelected}>Delete rows</BarButton>
      </div>

      {data?.table.locked && data.table.why && (
        <p className="db-locked" data-text="secondary">
          Read-only. {data.table.why}
        </p>
      )}
      {error && (
        <p className="db-locked" role="alert">
          {error}
        </p>
      )}

      {data && data.table.id === tableId ? (
        <Grid
          ref={grid}
          columns={shown}
          at={at}
          widths={spec.widths ?? {}}
          frozen={Math.min(spec.frozen ?? 1, shown.length)}
          rows={data.rows}
          groups={data.groups}
          summary={data.summary}
          picked={spec.summary ?? {}}
          sorts={sorts}
          lockedWhy={data.table.locked ? (data.table.why ?? 'This table can’t be typed in.') : null}
          onEdit={edit}
          onSay={say}
          onLink={(t, id) => navigate({ what: 'row', table: t, id })}
          onHeader={(column, r) => setPop({ kind: 'column', column, ...below(r) })}
          onResize={(id, w) => setSpec((s) => ({ ...s, widths: { ...s.widths, [id]: w } }))}
          onReorder={(from, to) =>
            setSpec((s) => {
              const order = shown.map((c) => c.id).filter((id) => id !== from);
              order.splice(order.indexOf(to), 0, from);
              return { ...s, order };
            })
          }
          onPick={(id, agg) => setSpec((s) => ({ ...s, summary: { ...s.summary, [id]: agg } }))}
          newRow={labelColumn ? { placeholder: `New ${labelColumn.name.toLowerCase()}, then Return`, add: addRow } : null}
          onAddColumn={(r) => setPop({ kind: 'add', x: Math.max(8, r.left - 280), y: r.bottom + 4 })}
        />
      ) : (
        <p className="db-empty">{error ? '' : 'Reading…'}</p>
      )}

      {(panels.pivot || panels.chart) && data && (
        <div className="db-panels">
      {panels.pivot && (
        <PivotPanel
          table={tableId}
          spec={spec}
          columns={columns}
          stamp={data}
          onChange={(pivot) => setSpec((s) => ({ ...s, pivot }))}
          onClose={() => togglePanel('pivot')}
        />
      )}
      {panels.chart && (
        <ChartPanel
          table={tableId}
          spec={spec}
          columns={columns}
          stamp={data}
          chart={chart}
          selection={() => grid.current?.selected() ?? null}
          onChange={(c) => setSpec((s) => ({ ...s, charts: c ? [c] : [] }))}
          onClose={() => togglePanel('chart')}
          onSay={say}
        />
      )}
        </div>
      )}

      {pop && (
        <>
          <div className="db-pop-back" onClick={() => setPop(null)} />
          <div
            className="db-pop"
            role="dialog"
            style={{ left: Math.min(pop.x, window.innerWidth - 340), top: Math.min(pop.y, window.innerHeight - 120) }}
          >
            {pop.kind === 'column' && (
              <ColumnMenu
                column={pop.column}
                frozen={shown.indexOf(pop.column) < (spec.frozen ?? 1)}
                run={(what) => {
                  const c = pop.column;
                  setPop(null);
                  if (what === 'asc' || what === 'desc') setSpec((s) => ({ ...s, sorts: [{ column: c.id, dir: what }] }));
                  else if (what === 'unsort') setSpec((s) => ({ ...s, sorts: (s.sorts ?? []).filter((x) => x.column !== c.id) }));
                  else if (what === 'filter') {
                    setSpec((s) => ({ ...s, filters: [...(s.filters ?? []), { column: c.id, op: c.type === 'bool' ? 'eq' : 'contains', value: c.type === 'bool' ? true : '' }] }));
                    setPop({ kind: 'filter', x: pop.x, y: pop.y });
                  } else if (what === 'group') setSpec((s) => ({ ...s, group: s.group === c.id ? null : c.id }));
                  else if (what === 'hide') setSpec((s) => ({ ...s, hidden: [...(s.hidden ?? []), c.id] }));
                  else if (what === 'freeze') setSpec((s) => ({ ...s, frozen: shown.indexOf(c) + 1 }));
                  else if (what === 'unfreeze') setSpec((s) => ({ ...s, frozen: 0 }));
                  else if (what === 'edit') setPop({ kind: 'add', x: pop.x, y: pop.y, edit: c });
                  else if (what === 'delete') void wrote(db.deleteColumn(tableId, c.id));
                }}
                sorted={sorts.find((s) => s.column === pop.column.id)?.dir ?? null}
                grouped={spec.group === pop.column.id}
              />
            )}
            {pop.kind === 'add' && (
              <ColumnForm
                table={tableId}
                tableName={data?.table.name ?? ''}
                formulaOnly={!isUser}
                columns={columns}
                functions={meta?.functions ?? []}
                edit={pop.edit}
                onDone={(p) => {
                  setPop(null);
                  void wrote(p);
                }}
              />
            )}
            {pop.kind === 'filter' && (
              <FilterForm
                columns={columns}
                filters={filters}
                match={spec.match ?? 'all'}
                onChange={(f, match) => setSpec((s) => ({ ...s, filters: f, match }))}
              />
            )}
            {pop.kind === 'sort' && (
              <SortForm
                columns={columns}
                sorts={sorts}
                group={spec.group ?? null}
                onChange={(next, group) => setSpec((s) => ({ ...s, sorts: next, group }))}
              />
            )}
            {pop.kind === 'columns' && (
              <div className="db-form">
                <h3 className="db-form-title">Columns</h3>
                <div className="db-checks">
                  {columns.map((c) => (
                    <label key={c.id} className="db-check-row">
                      <input
                        type="checkbox"
                        checked={!(spec.hidden ?? []).includes(c.id)}
                        onChange={(e) =>
                          setSpec((s) => ({
                            ...s,
                            hidden: e.target.checked ? (s.hidden ?? []).filter((h) => h !== c.id) : [...(s.hidden ?? []), c.id],
                          }))
                        }
                      />
                      {c.name}
                    </label>
                  ))}
                </div>
                <div className="db-form-row">
                  <button type="button" className="db-quiet" onClick={() => setSpec((s) => ({ ...s, hidden: [] }))}>
                    Show all
                  </button>
                </div>
              </div>
            )}
            {pop.kind === 'save' && (
              <SaveForm
                name={view?.name ?? ''}
                existing={view}
                onSave={(name) => void saveView(name)}
                onDelete={
                  view
                    ? () => {
                        setPop(null);
                        db.deleteView(view.id).then(
                          async () => {
                            await loadMeta();
                            setViewId(null);
                            say(`Deleted the view ‘${view.name}’. ${keys('⌘Z')} brings it back.`);
                          },
                          (e) => say(sentenceOf(e)),
                        );
                      }
                    : undefined
                }
              />
            )}
            {pop.kind === 'table' && (
              <SaveForm
                title="Table"
                name={data?.table.name ?? ''}
                existing={null}
                saveWord="Rename"
                onSave={(name) => {
                  setPop(null);
                  void wrote(db.renameTable(tableId, name));
                }}
                onDelete={() => {
                  setPop(null);
                  db.deleteTable(tableId).then(
                    async () => {
                      const m = await loadMeta();
                      openTable('task', null, m);
                      say(`Deleted the table. ${keys('⌘Z')} brings it back.`);
                    },
                    (e) => say(sentenceOf(e)),
                  );
                }}
                deleteWord="Delete table"
              />
            )}
          </div>
        </>
      )}
    </div>
  );
}

function BarButton({ children, on, onClick }: { children: ReactNode; on?: boolean; onClick(rect: DOMRect): void }) {
  return (
    <button
      type="button"
      className="db-button"
      aria-pressed={on || undefined}
      onClick={(e) => onClick(e.currentTarget.getBoundingClientRect())}
    >
      {children}
    </button>
  );
}

function ColumnMenu({
  column,
  sorted,
  grouped,
  frozen,
  run,
}: {
  column: Column;
  sorted: 'asc' | 'desc' | null;
  grouped: boolean;
  frozen: boolean;
  run(what: string): void;
}) {
  const item = (what: string, label: string) => (
    <button type="button" role="menuitem" className="db-menu-item" onClick={() => run(what)}>
      {label}
    </button>
  );
  return (
    <div className="db-menu" role="menu" aria-label={column.name}>
      <p className="db-menu-head">
        {column.name}
        <span data-text="secondary">
          {' '}
          · {column.type === 'bool' ? 'checkbox' : column.type}
          {column.locked ? ' · locked' : ''}
        </span>
      </p>
      {column.locked && column.why && (
        <p className="db-menu-why" data-text="secondary">
          {column.why}
        </p>
      )}
      {column.formula && <p className="db-menu-formula">= {column.formula}</p>}
      {item('asc', sorted === 'asc' ? '✓ Sort ascending' : 'Sort ascending')}
      {item('desc', sorted === 'desc' ? '✓ Sort descending' : 'Sort descending')}
      {sorted && item('unsort', 'Clear sort')}
      {item('filter', 'Filter by this column…')}
      {item('group', grouped ? 'Stop grouping by this column' : 'Group by this column')}
      {item('hide', 'Hide column')}
      {frozen ? item('unfreeze', 'Unfreeze columns') : item('freeze', 'Freeze up to this column')}
      {column.added && item('edit', column.type === 'formula' ? 'Edit formula or name…' : 'Rename…')}
      {column.added && item('delete', 'Delete column')}
    </div>
  );
}

/** A new column, or an added one renamed or given a new formula. The formula is checked as it is typed. */
function ColumnForm({
  table,
  tableName,
  formulaOnly,
  columns,
  functions,
  edit,
  onDone,
}: {
  table: string;
  tableName: string;
  formulaOnly: boolean;
  columns: Column[];
  functions: string[];
  edit?: Column;
  onDone(write: Promise<unknown>): void;
}) {
  const [name, setName] = useState(edit?.name ?? '');
  const [type, setType] = useState(edit?.type === 'bool' ? 'checkbox' : (edit?.type ?? (formulaOnly ? 'formula' : 'text')));
  const [formula, setFormula] = useState(edit?.formula ?? '');
  const [check, setCheck] = useState<{ ok: boolean; message?: string; sample?: unknown[] } | null>(null);
  const area = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (type !== 'formula' || !formula.trim()) return setCheck(null);
    const t = setTimeout(() => {
      db.checkFormula(table, formula).then(setCheck, () => setCheck(null));
    }, 200);
    return () => clearTimeout(t);
  }, [table, type, formula]);

  const insert = (text: string) => {
    const el = area.current;
    const at = el?.selectionStart ?? formula.length;
    setFormula(formula.slice(0, at) + text + formula.slice(el?.selectionEnd ?? at));
    el?.focus();
  };
  const ready = name.trim() && (type !== 'formula' || (formula.trim() && check?.ok !== false));
  const submit = () => {
    if (!ready) return;
    onDone(
      edit
        ? db.updateColumn(table, edit.id, { name: name.trim(), ...(type === 'formula' ? { formula } : {}) })
        : db.addColumn(table, name.trim(), type, type === 'formula' ? formula : undefined),
    );
  };
  const sample = (check?.sample ?? [])
    .map((v) => (v === null ? '(empty)' : typeof v === 'object' ? ((v as { error?: string }).error ?? '') : String(v)))
    .join(' · ');

  return (
    <div className="db-form db-form-wide">
      <h3 className="db-form-title">{edit ? `Edit ${edit.name}` : `New column in ${tableName}`}</h3>
      <label className="db-form-row">
        <span>Name</span>
        <input autoFocus value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && type !== 'formula' && submit()} />
      </label>
      {!edit && (
        <label className="db-form-row">
          <span>Kind</span>
          <select value={type} onChange={(e) => setType(e.target.value)}>
            {!formulaOnly && <option value="text">Text</option>}
            {!formulaOnly && <option value="number">Number</option>}
            {!formulaOnly && <option value="checkbox">Checkbox</option>}
            {!formulaOnly && <option value="date">Date</option>}
            <option value="formula">Formula</option>
          </select>
        </label>
      )}
      {formulaOnly && !edit && (
        <p className="db-form-note" data-text="secondary">
          {tableName} is Learn’s own table, so a column added to it is worked out by a formula.
        </p>
      )}
      {type === 'formula' && (
        <>
          <textarea
            ref={area}
            className="db-formula"
            rows={3}
            spellCheck={false}
            placeholder={'IF([Due] < TODAY(), "Late", "On time")'}
            aria-label="Formula"
            value={formula}
            onChange={(e) => setFormula(e.target.value)}
          />
          <p className="db-form-note" data-ok={check?.ok ?? undefined} role="status">
            {check === null
              ? '[Name] is this row’s value. Table[Name] is a whole column, for SUM, COUNTIF and LOOKUP.'
              : check.ok
                ? `First rows: ${sample || '(empty)'}`
                : check.message}
          </p>
          <div className="db-form-row">
            <select aria-label="Insert a column" value="" onChange={(e) => e.target.value && insert(`[${e.target.value}]`)}>
              <option value="">Insert a column…</option>
              {columns.filter((c) => c.id !== edit?.id).map((c) => (
                <option key={c.id} value={c.name}>
                  {c.name}
                </option>
              ))}
            </select>
            <select aria-label="Insert a function" value="" onChange={(e) => e.target.value && insert(`${e.target.value}()`)}>
              <option value="">Insert a function…</option>
              {functions.map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </select>
          </div>
          <p className="db-form-note" data-text="secondary">
            Or press {keys('⌘K')} and say what the column should be: “add a column for hours logged per course this week”.
          </p>
        </>
      )}
      <div className="db-form-row db-form-end">
        <button type="button" className="gel" disabled={!ready} onClick={submit}>
          {edit ? 'Save' : 'Add column'}
        </button>
      </div>
    </div>
  );
}

function FilterForm({
  columns,
  filters,
  match,
  onChange,
}: {
  columns: Column[];
  filters: Filter[];
  match: 'all' | 'any';
  onChange(filters: Filter[], match: 'all' | 'any'): void;
}) {
  const set = (i: number, patch: Partial<Filter>) => onChange(filters.map((f, n) => (n === i ? { ...f, ...patch } : f)), match);
  return (
    <div className="db-form db-form-wide">
      <h3 className="db-form-title">Filter</h3>
      {filters.length > 1 && (
        <label className="db-form-row">
          <span>Show rows that pass</span>
          <select value={match} onChange={(e) => onChange(filters, e.target.value as 'all' | 'any')}>
            <option value="all">every filter</option>
            <option value="any">any filter</option>
          </select>
        </label>
      )}
      {filters.map((f, i) => {
        const col = columns.find((c) => c.id === f.column);
        const op = OPS.find((o) => o.id === f.op);
        return (
          <div key={i} className="db-form-row db-filter">
            <select aria-label="Column" value={f.column} onChange={(e) => set(i, { column: e.target.value })}>
              {columns.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
            <select aria-label="How" value={f.op} onChange={(e) => set(i, { op: e.target.value })}>
              {OPS.map((o) => (
                <option key={o.id} value={o.id}>
                  {o.name}
                </option>
              ))}
            </select>
            {op?.needsValue !== false &&
              (col?.type === 'bool' ? (
                <select aria-label="Value" value={String(f.value === true)} onChange={(e) => set(i, { value: e.target.value === 'true' })}>
                  <option value="true">checked</option>
                  <option value="false">not checked</option>
                </select>
              ) : (
                <input
                  aria-label="Value"
                  type={col?.type === 'date' || col?.type === 'datetime' ? 'date' : 'text'}
                  value={String(f.value ?? '')}
                  onChange={(e) => set(i, { value: e.target.value })}
                />
              ))}
            <button type="button" className="db-quiet" aria-label="Remove this filter" onClick={() => onChange(filters.filter((_, n) => n !== i), match)}>
              ×
            </button>
          </div>
        );
      })}
      <div className="db-form-row">
        <button
          type="button"
          className="db-quiet"
          onClick={() => columns[0] && onChange([...filters, { column: (columns.find((c) => c.type !== 'json') ?? columns[0]).id, op: 'contains', value: '' }], match)}
        >
          + Add a filter
        </button>
        {filters.length > 0 && (
          <button type="button" className="db-quiet" onClick={() => onChange([], match)}>
            Clear all
          </button>
        )}
      </div>
    </div>
  );
}

function SortForm({
  columns,
  sorts,
  group,
  onChange,
}: {
  columns: Column[];
  sorts: { column: string; dir: 'asc' | 'desc' }[];
  group: string | null;
  onChange(sorts: { column: string; dir: 'asc' | 'desc' }[], group: string | null): void;
}) {
  return (
    <div className="db-form db-form-wide">
      <h3 className="db-form-title">Sort and group</h3>
      {sorts.map((s, i) => (
        <div key={i} className="db-form-row db-filter">
          <span>{i === 0 ? 'Sort by' : 'then by'}</span>
          <select aria-label="Column" value={s.column} onChange={(e) => onChange(sorts.map((x, n) => (n === i ? { ...x, column: e.target.value } : x)), group)}>
            {columns.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
          <select aria-label="Direction" value={s.dir} onChange={(e) => onChange(sorts.map((x, n) => (n === i ? { ...x, dir: e.target.value as 'asc' | 'desc' } : x)), group)}>
            <option value="asc">ascending</option>
            <option value="desc">descending</option>
          </select>
          <button type="button" className="db-quiet" aria-label="Remove this sort" onClick={() => onChange(sorts.filter((_, n) => n !== i), group)}>
            ×
          </button>
        </div>
      ))}
      <div className="db-form-row">
        <button type="button" className="db-quiet" onClick={() => columns[0] && onChange([...sorts, { column: columns[0].id, dir: 'asc' }], group)}>
          + Add a sort
        </button>
      </div>
      <label className="db-form-row">
        <span>Group by</span>
        <select value={group ?? ''} onChange={(e) => onChange(sorts, e.target.value || null)}>
          <option value="">nothing</option>
          {columns.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}

function SaveForm({
  title = 'Save this view',
  name: initial,
  existing,
  saveWord,
  onSave,
  onDelete,
  deleteWord = 'Delete view',
}: {
  title?: string;
  name: string;
  existing: View | null;
  saveWord?: string;
  onSave(name: string): void;
  onDelete?: () => void;
  deleteWord?: string;
}) {
  const [name, setName] = useState(initial);
  return (
    <div className="db-form">
      <h3 className="db-form-title">{title}</h3>
      <label className="db-form-row">
        <span>Name</span>
        <input autoFocus value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && name.trim() && onSave(name.trim())} />
      </label>
      {title === 'Save this view' && (
        <p className="db-form-note" data-text="secondary">
          A view keeps the filters, sorts, columns, pivot and chart. It is listed under its table.
        </p>
      )}
      <div className="db-form-row db-form-end">
        {onDelete && (
          <button type="button" className="db-quiet" onClick={onDelete}>
            {deleteWord}
          </button>
        )}
        <button type="button" className="gel" disabled={!name.trim()} onClick={() => onSave(name.trim())}>
          {saveWord ?? (existing && existing.name === name.trim() ? 'Save' : 'Save as new')}
        </button>
      </div>
    </div>
  );
}

/** The view's rows grouped by one or two columns, with totals. */
function PivotPanel({
  table,
  spec,
  columns,
  stamp,
  onChange,
  onClose,
}: {
  table: string;
  spec: Spec;
  columns: Column[];
  stamp: unknown;
  onChange(p: PivotSpec | null): void;
  onClose(): void;
}) {
  const pivot = spec.pivot ?? null;
  const [result, setResult] = useState<Pivot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const key = JSON.stringify([pivot?.rows, pivot?.values, spec.filters, spec.match, spec.search]);
  useEffect(() => {
    if (!pivot || pivot.rows.length === 0) return setResult(null);
    let live = true;
    db.pivot(table, spec, pivot).then(
      (r) => live && (setResult(r), setError(null)),
      (e) => live && setError(sentenceOf(e)),
    );
    return () => {
      live = false;
    };
  }, [table, key, stamp]); // eslint-disable-line react-hooks/exhaustive-deps

  const numeric = columns.filter((c) => c.type === 'number' || c.type === 'formula');
  const set = (patch: Partial<PivotSpec>) => onChange({ rows: [], values: [], ...pivot, ...patch });
  const across = !!pivot?.across && result?.by.length === 2;
  // Two columns deep, the second across the top: one line per first key, one column per second.
  const matrix = useMemo(() => {
    if (!across || !result) return null;
    const cols = [...new Set(result.groups.map((g) => g.keys[1]))];
    const lines = [...new Set(result.groups.map((g) => g.keys[0]))];
    const cell = new Map(result.groups.map((g) => [`${g.keys[0]}\u0000${g.keys[1]}`, g]));
    return { cols, lines, cell };
  }, [across, result]);
  const show = (v: unknown) => (v === null || v === undefined ? '' : String(v));

  return (
    <section className="db-panel" aria-label="Pivot">
      <header className="db-panel-bar">
        <strong>Pivot</strong>
        <label>
          Group by
          <select value={pivot?.rows[0] ?? ''} onChange={(e) => set({ rows: [e.target.value, ...(pivot?.rows.slice(1) ?? [])].filter(Boolean) })}>
            <option value="">choose a column</option>
            {columns.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          then by
          <select
            value={pivot?.rows[1] ?? ''}
            disabled={!pivot?.rows[0]}
            onChange={(e) => set({ rows: [pivot!.rows[0], e.target.value].filter(Boolean) })}
          >
            <option value="">nothing</option>
            {columns.filter((c) => c.id !== pivot?.rows[0]).map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        {pivot?.rows.length === 2 && (
          <label className="db-inline">
            <input type="checkbox" checked={!!pivot.across} onChange={(e) => set({ across: e.target.checked })} />
            across the top
          </label>
        )}
        <label>
          Total
          <select
            value={pivot?.values[0]?.column ?? ''}
            onChange={(e) => set({ values: e.target.value ? [{ column: e.target.value, agg: pivot?.values[0]?.agg ?? 'sum' }] : [] })}
          >
            <option value="">count only</option>
            {(numeric.length ? numeric : columns).map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        {pivot?.values[0] && (
          <select aria-label="How to total" value={pivot.values[0].agg} onChange={(e) => set({ values: [{ ...pivot.values[0], agg: e.target.value as Agg }] })}>
            {AGGS.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        )}
        <span className="db-bar-gap" />
        <button type="button" className="db-quiet" onClick={onClose}>
          Close
        </button>
      </header>
      <div className="db-panel-body">
        {error && <p className="db-chart-empty">{error}</p>}
        {!pivot?.rows.length && <p className="db-chart-empty">Choose a column to group by.</p>}
        {result && !matrix && (
          <table className="db-pivot">
            <thead>
              <tr>
                {result.by.map((b) => (
                  <th key={b.id}>{b.name}</th>
                ))}
                <th data-numeric>Rows</th>
                {result.values.map((v) => (
                  <th key={v.id} data-numeric>
                    {AGGS.find((a) => a.id === v.agg)?.name} of {v.name}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {result.groups.map((g, i) => (
                <tr key={i}>
                  {g.keys.map((k, n) => (
                    <td key={n}>{k || '(empty)'}</td>
                  ))}
                  <td data-numeric>{g.count}</td>
                  {g.values.map((v, n) => (
                    <td key={n} data-numeric>
                      {show(v)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr>
                <td colSpan={result.by.length}>All</td>
                <td data-numeric>{result.total.count}</td>
                {result.total.values.map((v, n) => (
                  <td key={n} data-numeric>
                    {show(v)}
                  </td>
                ))}
              </tr>
            </tfoot>
          </table>
        )}
        {result && matrix && (
          <table className="db-pivot">
            <thead>
              <tr>
                <th>
                  {result.by[0].name} \ {result.by[1].name}
                </th>
                {matrix.cols.map((c) => (
                  <th key={c} data-numeric>
                    {c || '(empty)'}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {matrix.lines.map((line) => (
                <tr key={line}>
                  <td>{line || '(empty)'}</td>
                  {matrix.cols.map((c) => {
                    const g = matrix.cell.get(`${line}\u0000${c}`);
                    return (
                      <td key={c} data-numeric>
                        {g ? show(result.values.length ? g.values[0] : g.count) : ''}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </section>
  );
}

/** A chart of the view, or of the rows and columns selected in the grid. */
function ChartPanel({
  table,
  spec,
  columns,
  stamp,
  chart,
  selection,
  onChange,
  onClose,
  onSay,
}: {
  table: string;
  spec: Spec;
  columns: Column[];
  stamp: unknown;
  chart: ChartSpec | null;
  selection(): { rows: string[]; columns: string[] } | null;
  onChange(c: ChartSpec | null): void;
  onClose(): void;
  onSay(text: string): void;
}) {
  const [data, setData] = useState<ChartData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const key = JSON.stringify([chart, spec.filters, spec.match, spec.search]);
  useEffect(() => {
    if (!chart || !chart.x || (chart.y.length === 0 && chart.agg !== 'count')) return setData(null);
    let live = true;
    db.chart(table, spec, chart).then(
      (r) => live && (setData(r), setError(null)),
      (e) => live && (setError(sentenceOf(e)), setData(null)),
    );
    return () => {
      live = false;
    };
  }, [table, key, stamp]); // eslint-disable-line react-hooks/exhaustive-deps

  const numeric = columns.filter((c) => c.type === 'number' || c.type === 'formula');
  const current: ChartSpec = chart ?? { id: 'c1', type: 'bar', x: '', y: [], agg: 'sum' };
  const set = (patch: Partial<ChartSpec>) => onChange({ ...current, ...patch });
  const fromSelection = () => {
    const picked = selection();
    if (!picked || picked.columns.length < 2) return onSay('Select at least two columns in the grid: the first runs along the bottom, the rest are drawn.');
    const [x, ...rest] = picked.columns;
    const y = rest.filter((id) => numeric.some((c) => c.id === id)).slice(0, MAX_SERIES);
    if (y.length === 0) return onSay('The columns after the first need to hold numbers.');
    onChange({ id: 'c1', type: current.type, x, y, agg: '', rows: picked.rows });
  };

  return (
    <section className="db-panel" aria-label="Chart">
      <header className="db-panel-bar">
        <strong>Chart</strong>
        <select aria-label="Kind of chart" value={current.type} onChange={(e) => set({ type: e.target.value as ChartSpec['type'] })}>
          <option value="bar">Bars</option>
          <option value="line">Line</option>
          <option value="scatter">Scatter</option>
        </select>
        <label>
          Along the bottom
          <select value={current.x} onChange={(e) => set({ x: e.target.value })}>
            <option value="">choose a column</option>
            {columns.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Draw
          <select value={current.y[0] ?? ''} onChange={(e) => set({ y: e.target.value ? [e.target.value, ...current.y.slice(1)] : [] })}>
            <option value="">{current.agg === 'count' ? 'the number of rows' : 'choose a column'}</option>
            {numeric.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        {current.y.length > 0 && current.y.length < MAX_SERIES && (
          <select aria-label="Add another series" value="" onChange={(e) => e.target.value && set({ y: [...current.y, e.target.value] })}>
            <option value="">+ and…</option>
            {numeric.filter((c) => !current.y.includes(c.id)).map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        )}
        {current.y.length > 1 && (
          <button type="button" className="db-quiet" onClick={() => set({ y: current.y.slice(0, 1) })}>
            one series
          </button>
        )}
        <label>
          as
          <select value={current.agg ?? ''} onChange={(e) => set({ agg: e.target.value as Agg | '' })}>
            <option value="">each row</option>
            <option value="sum">the sum per group</option>
            <option value="average">the average per group</option>
            <option value="count">the count per group</option>
            <option value="min">the least per group</option>
            <option value="max">the most per group</option>
          </select>
        </label>
        <button type="button" className="db-quiet" onClick={fromSelection}>
          From selection
        </button>
        {current.rows && (
          <button type="button" className="db-quiet" onClick={() => set({ rows: undefined })}>
            Whole view
          </button>
        )}
        <span className="db-bar-gap" />
        <button type="button" className="db-quiet" onClick={onClose}>
          Close
        </button>
      </header>
      <div className="db-panel-body">
        {!current.x ? (
          <p className="db-chart-empty">Choose what runs along the bottom and what to draw, or select cells in the grid and press From selection.</p>
        ) : (
          <Chart spec={current} data={data} error={error} />
        )}
      </div>
    </section>
  );
}
