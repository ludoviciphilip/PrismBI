/**
 * Shared, pure "Sort by" logic for every report visual (bar/column/line/pie/
 * table/matrix/slicer) and for the Filters pane value lists.
 *
 * Semantics (mirrors Power BI Desktop):
 *  - Numbers sort numerically and dates chronologically — never as strings,
 *    even when the value arrives as a string (categories are always strings
 *    after the query layer, e.g. "10" or "2025-01-31").
 *  - Text uses pt-BR collation (`Intl.Collator('pt-BR')`, the same ordering as
 *    `localeCompare(…, 'pt-BR')`).
 *  - Blanks (null/undefined/''/NaN) always go last, in both directions.
 *  - A category column with `sortByColumn` sorts by that column's values
 *    ("Nome do Mês" sorts by "Mês") — callers pass the lookup built with
 *    `buildSortByLookup`.
 *  - TopN/limit is applied AFTER sorting.
 *
 * The same function is applied to rows coming from the Rust native visual
 * query and from the JS QueryPlanner, so both paths render identically.
 */

export type SortDirection = 'asc' | 'desc';

/** Kind rank: numbers < dates < text; blanks are handled separately (always last). */
const KIND_NUMBER = 0;
const KIND_DATE = 1;
const KIND_TEXT = 2;
const KIND_BLANK = 3;

interface NormalizedKey {
  kind: number;
  num: number;
  text: string;
}

const collator = new Intl.Collator('pt-BR');

const NUMERIC_RE = /^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/;
const ISO_DATE_RE = /^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:?\d{2})?)?$/;
const BR_DATE_RE = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/;

export function isSortBlank(v: unknown): boolean {
  if (v === null || v === undefined) return true;
  if (typeof v === 'number') return Number.isNaN(v);
  if (typeof v === 'string') return v.trim() === '';
  if (v instanceof Date) return Number.isNaN(v.getTime());
  return false;
}

export function normalizeSortKey(v: unknown): NormalizedKey {
  if (isSortBlank(v)) return { kind: KIND_BLANK, num: 0, text: '' };
  if (typeof v === 'number') return { kind: KIND_NUMBER, num: v, text: '' };
  if (typeof v === 'bigint') return { kind: KIND_NUMBER, num: Number(v), text: '' };
  if (typeof v === 'boolean') return { kind: KIND_NUMBER, num: v ? 1 : 0, text: '' };
  if (v instanceof Date) return { kind: KIND_DATE, num: v.getTime(), text: '' };
  const s = String(v).trim();
  if (NUMERIC_RE.test(s)) return { kind: KIND_NUMBER, num: Number(s), text: '' };
  if (ISO_DATE_RE.test(s)) {
    const t = Date.parse(s);
    if (!Number.isNaN(t)) return { kind: KIND_DATE, num: t, text: '' };
  }
  const br = BR_DATE_RE.exec(s);
  if (br) {
    const t = Date.UTC(Number(br[3]), Number(br[2]) - 1, Number(br[1]));
    if (!Number.isNaN(t)) return { kind: KIND_DATE, num: t, text: '' };
  }
  return { kind: KIND_TEXT, num: 0, text: s };
}

function compareNormalized(a: NormalizedKey, b: NormalizedKey, dir: SortDirection): number {
  // Blanks last regardless of direction.
  if (a.kind === KIND_BLANK || b.kind === KIND_BLANK) {
    if (a.kind === b.kind) return 0;
    return a.kind === KIND_BLANK ? 1 : -1;
  }
  let cmp: number;
  if (a.kind !== b.kind) cmp = a.kind - b.kind;
  else if (a.kind === KIND_TEXT) cmp = collator.compare(a.text, b.text);
  else cmp = a.num === b.num ? 0 : a.num < b.num ? -1 : 1;
  return dir === 'desc' ? -cmp : cmp;
}

/** Compare two raw values with the shared semantics (blanks last in both directions). */
export function compareSortValues(a: unknown, b: unknown, dir: SortDirection = 'asc'): number {
  return compareNormalized(normalizeSortKey(a), normalizeSortKey(b), dir);
}

/** Stable permutation of `0..n-1` ordering items by `keyOf`. */
export function sortPermutation<T>(
  items: readonly T[],
  keyOf: (item: T, index: number) => unknown,
  dir: SortDirection = 'asc'
): number[] {
  const keys = items.map((it, i) => normalizeSortKey(keyOf(it, i)));
  const idx = items.map((_, i) => i);
  idx.sort((a, b) => compareNormalized(keys[a], keys[b], dir) || a - b);
  return idx;
}

/**
 * Builds a `categoryLabel -> sortBy value` map for a column configured with
 * `sortByColumn` (first non-blank value wins, as in Power BI where the
 * relationship must be 1:1).
 */
export function buildSortByLookup(
  rows: readonly Record<string, any>[],
  column: string,
  sortByColumn: string
): Map<string, unknown> {
  const map = new Map<string, unknown>();
  for (const r of rows) {
    const label = r[column];
    if (isSortBlank(label)) continue;
    const key = String(label);
    if (map.has(key)) continue;
    const w = r[sortByColumn];
    if (!isSortBlank(w)) map.set(key, w);
  }
  return map;
}

function stripFieldDecorations(f: string): string {
  let s = f.trim().replace(/^Σ\s*/, '');
  // "Tabela.Coluna" / "Tabela[Coluna]" -> "Coluna"
  const bracket = /\[([^\]]+)\]$/.exec(s);
  if (bracket) s = bracket[1];
  else if (s.includes('.')) s = s.slice(s.indexOf('.') + 1);
  return s.toLowerCase();
}

/** Field-name equality tolerant to table qualification ("T.Col" == "Col") and the "Σ " prefix. */
export function fieldMatches(a: string | undefined, b: string | undefined): boolean {
  if (!a || !b) return false;
  if (a.toLowerCase() === b.toLowerCase()) return true;
  return stripFieldDecorations(a) === stripFieldDecorations(b);
}

/** Sort detail rows (table visual / raw rows) by one column. */
export function sortRows<T extends Record<string, any>>(
  rows: readonly T[],
  field: string | undefined,
  dir: SortDirection = 'asc',
  options: { sortByColumn?: string } = {}
): T[] {
  if (!field || rows.length === 0) return [...rows];
  const sample = rows[0];
  const actual =
    Object.keys(sample).find(k => k === field) ??
    Object.keys(sample).find(k => fieldMatches(k, field)) ??
    field;
  const sortBy = options.sortByColumn && options.sortByColumn in sample ? options.sortByColumn : undefined;
  const perm = sortPermutation(rows, r => (sortBy ? r[sortBy] : r[actual]), dir);
  return perm.map(i => rows[i]);
}

export interface CategorySeriesData {
  categories: string[];
  series: { name: string; data: (number | null)[] }[];
}

export interface VisualSortSpec {
  sortField?: string;
  sortOrder?: SortDirection;
  /** TopN / row limit, applied after sorting. */
  limit?: number;
  /** Fields that identify the category axis (raw config field, resolved drill level, …). */
  categoryFields: string[];
  /** Value fields (yAxisFields). With a legend, sorting by one of them sorts by the row total. */
  valueFields: string[];
  /** categoryLabel -> sortBy value (from the category column's `sortByColumn`). */
  categorySortKeys?: Map<string, unknown>;
  /** legendLabel -> sortBy value (from the legend column's `sortByColumn`). */
  legendSortKeys?: Map<string, unknown>;
}

export type ResolvedSortTarget =
  | { kind: 'category' }
  | { kind: 'series'; index: number }
  | { kind: 'total' }
  | { kind: 'none' };

export function resolveSortTarget(data: CategorySeriesData, spec: VisualSortSpec): ResolvedSortTarget {
  const f = spec.sortField;
  if (!f) return { kind: 'none' };
  const exact = data.series.findIndex(s => s.name.toLowerCase() === f.toLowerCase());
  if (exact !== -1) return { kind: 'series', index: exact };
  if (spec.categoryFields.some(c => fieldMatches(c, f))) return { kind: 'category' };
  const loose = data.series.findIndex(s => fieldMatches(s.name, f));
  if (loose !== -1) return { kind: 'series', index: loose };
  if (spec.valueFields.some(v => fieldMatches(v, f)) && data.series.length > 0) return { kind: 'total' };
  return { kind: 'none' };
}

function categoryKey(spec: VisualSortSpec, cat: string): unknown {
  if (spec.categorySortKeys && spec.categorySortKeys.has(cat)) return spec.categorySortKeys.get(cat);
  return cat;
}

/**
 * Sorts categories (+ the aligned series data) and then applies the limit.
 *
 * Without an explicit (resolvable) sortField: categories are ordered by the
 * axis (honouring sortByColumn) in `sortOrder` direction (default ascending),
 * or — when a TopN limit is set — by the first series descending, so TopN
 * keeps the largest values.
 */
export function applyVisualSort<T extends CategorySeriesData>(data: T, spec: VisualSortSpec): T {
  const dir: SortDirection = spec.sortOrder === 'desc' ? 'desc' : 'asc';
  const { categories } = data;
  let { series } = data;
  if (spec.legendSortKeys && spec.legendSortKeys.size > 0 && series.length > 1) {
    const sPerm = sortPermutation(
      series,
      s => (spec.legendSortKeys!.has(s.name) ? spec.legendSortKeys!.get(s.name) : s.name),
      'asc'
    );
    series = sPerm.map(i => series[i]);
  }
  const workingData = { ...data, series };
  const limit = spec.limit && spec.limit > 0 ? spec.limit : undefined;
  const target = resolveSortTarget(workingData, spec);

  let perm: number[];
  if (target.kind === 'series') {
    const s = series[target.index];
    perm = sortPermutation(categories, (_, i) => s.data[i], dir);
  } else if (target.kind === 'total') {
    perm = sortPermutation(
      categories,
      (_, i) => {
        const vals = series.map(s => s.data[i]).filter((v): v is number => v !== null && v !== undefined && Number.isFinite(v));
        return vals.length === 0 ? null : vals.reduce((acc, v) => acc + v, 0);
      },
      dir
    );
  } else if (target.kind === 'category') {
    perm = sortPermutation(categories, c => categoryKey(spec, c), dir);
  } else if (limit && series.length > 0 && categories.length > limit) {
    perm = sortPermutation(categories, (_, i) => series[0].data[i], 'desc');
  } else {
    perm = sortPermutation(categories, c => categoryKey(spec, c), dir);
  }

  if (limit) perm = perm.slice(0, limit);
  return {
    ...workingData,
    categories: perm.map(i => categories[i]),
    series: series.map(s => ({ ...s, data: perm.map(i => s.data[i]) })),
  };
}

export type ValueListSortMode = 'asc' | 'desc' | 'count';

export interface ValueListItem {
  value: unknown;
  count: number;
}

/**
 * Orders a value list (slicer items, filter-card checklist).
 * 'count' sorts by frequency descending, ties broken A-Z.
 */
export function sortValueList<T extends ValueListItem>(
  items: readonly T[],
  mode: ValueListSortMode,
  sortKeys?: Map<string, unknown>
): T[] {
  const keyOf = (it: T) => {
    const k = String(it.value ?? '');
    return sortKeys && sortKeys.has(k) ? sortKeys.get(k) : it.value;
  };
  const asc = sortPermutation(items, keyOf, mode === 'desc' ? 'desc' : 'asc');
  if (mode !== 'count') return asc.map(i => items[i]);
  const rank = new Map<number, number>();
  asc.forEach((itemIdx, pos) => rank.set(itemIdx, pos));
  const idx = items.map((_, i) => i);
  idx.sort((a, b) => items[b].count - items[a].count || rank.get(a)! - rank.get(b)!);
  return idx.map(i => items[i]);
}

/** Sorts plain category labels (e.g. slicer items). */
export function sortCategoryLabels(
  labels: readonly string[],
  dir: SortDirection = 'asc',
  sortKeys?: Map<string, unknown>
): string[] {
  const perm = sortPermutation(labels, l => (sortKeys && sortKeys.has(l) ? sortKeys.get(l) : l), dir);
  return perm.map(i => labels[i]);
}
