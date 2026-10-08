// The Database tab's calls to the core (docs/ASK.md): `db.*`. The core does
// the work: it reads the tables, filters, sorts, totals and works formulas
// out, and every edit here is a command there. This file is the shapes.

import { call } from '../../bridge';

export type ColType = 'text' | 'number' | 'bool' | 'date' | 'datetime' | 'relation' | 'json' | 'formula';

export interface Column {
  id: string;
  name: string;
  type: ColType;
  locked: boolean;
  /** Why it can't be typed over. */
  why?: string;
  /** The table a relation points into. */
  relation?: string;
  options?: string[];
  formula?: string;
  /** A column the person added: it can be renamed and deleted. */
  added: boolean;
}

/** A cell: a value, a link to another row, or a formula's failure. */
export type Cell =
  | null
  | string
  | number
  | boolean
  | { id: string; label: string }
  | { error: string; message: string };

export interface Row {
  id: string;
  cells: Cell[];
}

export interface TableInfo {
  id: string;
  name: string;
  origin: 'learn' | 'derived' | 'user';
  count: number;
}

export type Agg = 'sum' | 'average' | 'count' | 'filled' | 'empty' | 'unique' | 'min' | 'max' | 'median' | 'checked';

export interface Filter {
  column: string;
  op: string;
  value?: unknown;
}

export interface Sort {
  column: string;
  dir: 'asc' | 'desc';
}

export interface PivotSpec {
  rows: string[];
  values: { column: string; agg: Agg }[];
  /** With two columns: the second runs across the top. */
  across?: boolean;
}

export interface ChartSpec {
  id: string;
  type: 'bar' | 'line' | 'scatter';
  x: string;
  y: string[];
  agg?: Agg | '';
  /** Only these rows: a chart made from a selection. */
  rows?: string[];
}

/** A view of a table: what it shows, in what order, and how it is laid out. */
export interface Spec {
  filters?: Filter[];
  match?: 'all' | 'any';
  search?: string;
  sorts?: Sort[];
  group?: string | null;
  hidden?: string[];
  order?: string[];
  widths?: Record<string, number>;
  frozen?: number;
  summary?: Record<string, Agg | ''>;
  pivot?: PivotSpec | null;
  charts?: ChartSpec[];
}

export interface View {
  id: string;
  table: string;
  name: string;
  spec: Spec;
}

export interface Query {
  table: { id: string; name: string; origin: 'learn' | 'derived' | 'user'; locked: boolean; why?: string | null };
  columns: Column[];
  rows: Row[];
  total: number;
  all: number;
  summary: Record<string, Partial<Record<Agg, Cell>>>;
  groups: { key: Cell; label: string; start: number; count: number }[];
}

export interface Pivot {
  by: { id: string; name: string }[];
  values: { id: string; name: string; agg: Agg }[];
  groups: { keys: string[]; count: number; values: Cell[] }[];
  total: { count: number; values: Cell[] };
}

export interface ChartData {
  x: { id: string; name: string; kind: 'category' | 'number' | 'date' };
  labels: string[];
  xs: (number | null)[];
  series: { name: string; values: (number | null)[] }[];
}

export interface Edit {
  row: string;
  column: string;
  value: unknown;
}

export interface Written {
  changed?: number;
  ids?: string[];
  deleted?: number;
  failed: { row?: string | number; column?: string | null; message: string }[];
  undo: string | null;
}

/** What only the grid reads is left out of what the core is asked to compute. */
export function queryPart(spec: Spec): Spec {
  const { filters, match, search, sorts, group } = spec;
  return {
    filters: (filters ?? []).filter((f) => f.column && (f.op === 'blank' || f.op === 'not_blank' || f.value !== undefined)),
    match,
    search,
    sorts,
    group: group ?? undefined,
  };
}

export const db = {
  tables: () =>
    call<{ tables: TableInfo[]; views: View[]; layouts: Record<string, Spec>; functions: string[] }>('db.tables'),
  query: (table: string, spec: Spec) => call<Query>('db.query', { table, spec: queryPart(spec) }),
  pivot: (table: string, spec: Spec, pivot: PivotSpec) =>
    call<Pivot>('db.pivot', { table, spec: queryPart(spec), rows: pivot.rows, values: pivot.values }),
  chart: (table: string, spec: Spec, chart: ChartSpec) =>
    call<ChartData>('db.chart', {
      table,
      spec: { ...queryPart(spec), ...(chart.rows ? { rows: chart.rows } : {}) },
      x: chart.x,
      y: chart.y,
      agg: chart.agg || undefined,
    }),
  setCells: (table: string, edits: Edit[], label?: string) =>
    call<Written>('db.cells.set', { table, edits, ...(label ? { label } : {}) }),
  addRows: (table: string, rows: Record<string, unknown>[]) => call<Written>('db.rows.add', { table, rows }),
  deleteRows: (table: string, rows: string[]) => call<Written>('db.rows.delete', { table, rows }),
  createTable: (name: string) => call<{ table: { id: string; name: string } }>('db.table.create', { name }),
  renameTable: (table: string, name: string) => call('db.table.rename', { table, name }),
  deleteTable: (table: string) => call('db.table.delete', { table }),
  addColumn: (table: string, name: string, type: string, formula?: string) =>
    call('db.column.add', { table, name, type, ...(formula ? { formula } : {}) }),
  updateColumn: (table: string, column: string, set: { name?: string; formula?: string }) =>
    call('db.column.update', { table, column, ...set }),
  deleteColumn: (table: string, column: string) => call('db.column.delete', { table, column }),
  checkFormula: (table: string, formula: string) =>
    call<{ ok: boolean; message?: string; at?: number; sample?: Cell[] }>('db.formula.check', { table, formula }),
  saveView: (table: string, name: string, spec: Spec, id?: string) =>
    call<{ view: View }>('db.view.save', { table, name, spec, ...(id ? { id } : {}) }),
  deleteView: (id: string) => call('db.view.delete', { id }),
  setLayout: (table: string, spec: Spec) => call('db.layout.set', { table, spec }),
  importCsv: (name: string, csv: string) =>
    call<{ table: { id: string; name: string }; rows: number; columns: number }>('db.csv.import', { name, csv }),
  exportCsv: (table: string, spec: Spec) =>
    call<{ file?: string; path?: string; rows: number }>('db.csv.export', {
      table,
      spec: { ...queryPart(spec), hidden: spec.hidden, order: spec.order },
      to: 'downloads',
    }),
};

/** A cell as the grid shows it. */
export function cellText(cell: Cell): string {
  if (cell === null || cell === undefined) return '';
  if (typeof cell === 'object') return 'label' in cell ? cell.label : cell.error;
  if (typeof cell === 'boolean') return cell ? 'TRUE' : 'FALSE';
  return String(cell);
}

/** A cell as it goes into an editor: what would be typed to make it. */
export function cellInput(cell: Cell): string {
  if (cell === null || cell === undefined) return '';
  if (typeof cell === 'object') return 'label' in cell ? cell.label : '';
  if (typeof cell === 'boolean') return cell ? 'yes' : 'no';
  return String(cell);
}

export const isError = (cell: Cell): cell is { error: string; message: string } =>
  typeof cell === 'object' && cell !== null && 'error' in cell;
export const isLink = (cell: Cell): cell is { id: string; label: string } =>
  typeof cell === 'object' && cell !== null && 'label' in cell;

export const AGGS: { id: Agg; name: string }[] = [
  { id: 'sum', name: 'Sum' },
  { id: 'average', name: 'Average' },
  { id: 'count', name: 'Count' },
  { id: 'filled', name: 'Filled' },
  { id: 'unique', name: 'Unique' },
  { id: 'min', name: 'Min' },
  { id: 'max', name: 'Max' },
  { id: 'median', name: 'Median' },
  { id: 'checked', name: 'Checked' },
];

export const OPS: { id: string; name: string; needsValue: boolean }[] = [
  { id: 'contains', name: 'contains', needsValue: true },
  { id: 'not_contains', name: 'doesn’t contain', needsValue: true },
  { id: 'eq', name: 'is', needsValue: true },
  { id: 'ne', name: 'is not', needsValue: true },
  { id: 'gt', name: 'is after / more than', needsValue: true },
  { id: 'ge', name: 'is on or after / at least', needsValue: true },
  { id: 'lt', name: 'is before / less than', needsValue: true },
  { id: 'le', name: 'is on or before / at most', needsValue: true },
  { id: 'starts', name: 'starts with', needsValue: true },
  { id: 'ends', name: 'ends with', needsValue: true },
  { id: 'blank', name: 'is empty', needsValue: false },
  { id: 'not_blank', name: 'is not empty', needsValue: false },
];
