/**
 * v0.10 Frente G: consultas de drill e matriz N níveis.
 *
 * Dois caminhos com o MESMO contrato de saída:
 * - nativo (Tauri): `query_visual_native` com `categoryLevels` / `drillFilters` /
 *   `matrixRows` (src-tauri/src/dax/query_planner.rs). O caminho de drill entra
 *   no `build_filter_context` do Rust.
 * - fallback TS (navegador/testes): o mesmo algoritmo sobre o `FilterContext` do
 *   motor TS. Cada categoria/nó/total é a medida avaliada pelo motor no seu próprio
 *   contexto de filtro; nada é somado a partir das linhas filhas.
 */
import { Filter, Table, Visual } from '../../../types/bi';
import { ModelAccess } from '../../../engine/model/modelAccess';
import { FilterContext, filterContextFromUIFilters } from '../../../engine/model/filterContext';
import { DaxEngine } from '../../../engine/daxEngine';
import { isBlank } from '../../../engine/expr/types';
import { parseNumber } from '../../../engine/shared/valueParser';
import { buildSortByLookup, sortPermutation } from '../../../engine/visualSort';
import {
  COLLAPSED_MATRIX,
  DrillFilterSpec,
  DrillLevel,
  MatrixExpansion,
  concatLevelLabels,
  datePartValue,
  isMatrixPathExpanded,
  levelValueLabel,
} from '../../../engine/drill';
import type { NativeVisualQueryRequest, NativeVisualQueryResult, NativeMatrixNodeDto } from '../../../bridge/tauriBridge';

export interface LevelQueryResult {
  categories: string[];
  /** Valor de cada nível agrupado, alinhado com `categories`. */
  categoryKeys: unknown[][];
  series: { name: string; data: (number | null)[] }[];
}

export interface MatrixNode {
  key: unknown;
  label: string;
  values: (number | null)[];
  rowTotal: number | null;
  hasChildren: boolean;
  expanded: boolean;
  children: MatrixNode[];
}

export interface MatrixQueryResult {
  seriesNames: string[];
  rows: MatrixNode[];
  columnTotals: (number | null)[];
  grandTotal: number | null;
  /** v0.11 Rodada 2 (Frente I): árvore de colunas quando a matriz tem N níveis
   * nas colunas (`matrixColumnFields`). `rows[].values`/`columnTotals` ficam
   * alinhados com a ordem DFS das colunas VISÍVEIS desta árvore (folha ou
   * subtotal recolhido), nunca com `seriesNames` (que fica vazio aqui). */
  columnHeader?: MatrixColumnNode[];
}

export interface MatrixColumnNode {
  key: unknown;
  label: string;
  value: number | null;
  hasChildren: boolean;
  expanded: boolean;
  children: MatrixColumnNode[];
}

const MAX_MATRIX_NODES = 50_000;
const MAX_LEVEL_CATEGORIES = 10_000;

// ---------------------------------------------------------------------------
// Fallback TS
// ---------------------------------------------------------------------------

interface ResolvedColumn {
  table: Table;
  col: string;
}

function resolveColumn(model: ModelAccess, base: Table, raw: string): ResolvedColumn {
  const clean = raw.trim();
  if (clean.includes('.')) {
    const [t, c] = clean.split('.', 2);
    const found = model.getTable(t);
    if (found) return { table: found, col: found.columns.find(x => x.name.toLowerCase() === c.toLowerCase())?.name ?? c };
  }
  const own = base.columns.find(c => c.name.toLowerCase() === clean.toLowerCase());
  if (own) return { table: base, col: own.name };
  for (const t of model.getTables()) {
    const c = t.columns.find(x => x.name.toLowerCase() === clean.toLowerCase());
    if (c) return { table: t, col: c.name };
  }
  return { table: base, col: clean };
}

const blankValue = (v: unknown) => v === null || v === undefined || v === '' || isBlank(v);

/** Restrição de um nível/passo do drill aplicada (com interseção) no contexto. */
function applyLevelFilter(ctx: FilterContext, rc: ResolvedColumn, level: Pick<DrillLevel, 'datePart'>, value: unknown) {
  if (level.datePart) {
    const part = level.datePart;
    const want = value === null || value === undefined ? null : Number(value);
    ctx.setColumnFilter(
      rc.table.id,
      rc.col,
      { tableId: rc.table.id, column: rc.col, predicate: v => datePartValue(v, part) === want },
      true
    );
    return;
  }
  ctx.setColumnFilter(
    rc.table.id,
    rc.col,
    {
      tableId: rc.table.id,
      column: rc.col,
      ...(blankValue(value) ? { predicate: (v: unknown) => blankValue(v) } : { allowedValues: new Set([value]) }),
    },
    true
  );
}

function baseContext(model: ModelAccess, visual: Visual, base: Table, filters: Filter[], drill: DrillFilterSpec[]) {
  const ctx = filterContextFromUIFilters(filters, model, visual.id);
  for (const df of drill) applyLevelFilter(ctx, resolveColumn(model, base, df.field), df, df.value);
  return ctx;
}

function compareKeys(a: unknown, b: unknown): number {
  if (blankValue(a)) return blankValue(b) ? 0 : -1;
  if (blankValue(b)) return 1;
  if (typeof a === 'number' && typeof b === 'number') return a - b;
  return String(a).localeCompare(String(b), 'pt-BR', { sensitivity: 'base', numeric: true });
}

/** Valores distintos de um nível visíveis no contexto (em ordem, honrando sortByColumn). */
function distinctKeys(model: ModelAccess, ctx: FilterContext, rc: ResolvedColumn, level: DrillLevel): unknown[] {
  const seen = new Map<string, unknown>();
  for (const r of ctx.getFilteredRows(rc.table.id, model)) {
    const raw = r[rc.col];
    const key = level.datePart ? datePartValue(raw, level.datePart) : blankValue(raw) ? null : raw;
    const id = typeof key === 'string' ? `s:${key.toLowerCase()}` : `v:${String(key)}`;
    if (!seen.has(id)) seen.set(id, key);
  }
  const values = Array.from(seen.values());
  if (!level.datePart) {
    const meta = rc.table.columns.find(c => c.name.toLowerCase() === rc.col.toLowerCase());
    if (meta?.sortByColumn) {
      const lookup = buildSortByLookup(rc.table.rows, rc.col, meta.sortByColumn);
      if (lookup.size > 0) {
        const perm = sortPermutation(values, k => {
          const s = blankValue(k) ? '' : String(k);
          return lookup.has(s) ? lookup.get(s) : k;
        }, 'asc');
        return perm.map(i => values[i]);
      }
    }
  }
  return values.sort(compareKeys);
}

function aggregate(values: unknown[], agg: string): number | null {
  const present = values.filter(v => !blankValue(v));
  switch (agg) {
    case 'COUNT':
      return present.length || null;
    case 'DISTINCTCOUNT':
      return new Set(present.map(v => String(v).toLowerCase())).size || null;
    default: {
      const nums = present.map(v => parseNumber(v).value).filter((n): n is number => typeof n === 'number' && Number.isFinite(n));
      if (!nums.length) return null;
      if (agg === 'AVG' || agg === 'AVERAGE') return nums.reduce((a, b) => a + b, 0) / nums.length;
      if (agg === 'MIN') return Math.min(...nums);
      if (agg === 'MAX') return Math.max(...nums);
      if (agg === 'NONE') return nums[0];
      return nums.reduce((a, b) => a + b, 0);
    }
  }
}

function evalField(model: ModelAccess, ctx: FilterContext, base: Table, field: string, aggOpt?: string): number | null {
  const clean = field.replace(/^Σ\s*/, '').trim();
  const m = model.getMeasure ? model.getMeasure(clean) : undefined;
  if (m) {
    const v = DaxEngine.evaluateWithContext(m.formula, model, ctx);
    if (blankValue(v)) return null;
    const n = typeof v === 'number' ? v : Number(v);
    return Number.isFinite(n) ? n : null;
  }
  const rc = resolveColumn(model, base, clean);
  const agg = !aggOpt || aggOpt === 'NONE' ? 'SUM' : aggOpt;
  return aggregate(
    ctx.getFilteredRows(rc.table.id, model).map(r => r[rc.col]),
    agg
  );
}

interface SeriesEval {
  names: string[];
  values(ctx: FilterContext): (number | null)[];
  rowTotal(ctx: FilterContext): number | null;
}

function seriesEval(model: ModelAccess, visual: Visual, base: Table, ctx: FilterContext): SeriesEval {
  const dc = visual.dataConfig;
  const fields = (dc.yAxisFields || []).map(f => f.replace(/^Σ\s*/, '').trim());
  const agg = dc.aggregation;
  if (dc.legendField) {
    const rc = resolveColumn(model, base, dc.legendField);
    const legendLevel: DrillLevel = { field: dc.legendField, caption: dc.legendField };
    const keys = distinctKeys(model, ctx, rc, legendLevel).filter(k => !blankValue(k));
    const primary = fields[0];
    return {
      names: keys.map(k => String(k)),
      values: c =>
        keys.map(k => {
          if (!primary) return null;
          const lc = c.clone();
          applyLevelFilter(lc, rc, legendLevel, k);
          return evalField(model, lc, base, primary, agg);
        }),
      rowTotal: c => (primary ? evalField(model, c, base, primary, agg) : null),
    };
  }
  return {
    names: fields,
    values: c => fields.map(f => evalField(model, c, base, f, agg)),
    rowTotal: () => null,
  };
}

export interface LevelQueryInput {
  visual: Visual;
  filters: Filter[];
  /** Níveis pelos quais agrupar (1 = drill; N = expandir todos os níveis). */
  levels: DrillLevel[];
  drillFilters: DrillFilterSpec[];
}

export function queryLevelsTs(model: ModelAccess, input: LevelQueryInput): LevelQueryResult {
  const base = model.getTable(input.visual.dataConfig.tableId);
  if (!base || input.levels.length === 0) return { categories: [], categoryKeys: [], series: [] };
  const ctx = baseContext(model, input.visual, base, input.filters, input.drillFilters);
  const series = seriesEval(model, input.visual, base, ctx);
  const cols = input.levels.map(l => resolveColumn(model, base, l.field));
  const out: LevelQueryResult = { categories: [], categoryKeys: [], series: series.names.map(name => ({ name, data: [] })) };
  let count = 0;

  const walk = (c: FilterContext, depth: number, keys: unknown[]) => {
    for (const k of distinctKeys(model, c, cols[depth], input.levels[depth])) {
      if (count >= MAX_LEVEL_CATEGORIES) return;
      const cc = c.clone();
      applyLevelFilter(cc, cols[depth], input.levels[depth], k);
      const path = [...keys, k];
      if (depth + 1 < input.levels.length) {
        walk(cc, depth + 1, path);
        continue;
      }
      const vals = series.values(cc);
      if (vals.every(v => v === null)) continue;
      count++;
      out.categories.push(concatLevelLabels(input.levels, path));
      out.categoryKeys.push(path);
      vals.forEach((v, i) => out.series[i].data.push(v));
    }
  };
  walk(ctx, 0, []);
  return out;
}

export interface MatrixQueryInput {
  visual: Visual;
  filters: Filter[];
  levels: DrillLevel[];
  expansion: MatrixExpansion;
  drillFilters?: DrillFilterSpec[];
  /** v0.11 Rodada 2 (Frente I): níveis/expansão das COLUNAS (N níveis). */
  columnLevels?: DrillLevel[];
  columnExpansion?: MatrixExpansion;
}

interface ColumnFilterStep {
  rc: ResolvedColumn;
  level: DrillLevel;
  key: unknown;
}

interface ColumnLeaf {
  steps: ColumnFilterStep[];
}

/**
 * v0.11 Rodada 2 (Frente I): constrói a árvore de colunas UMA VEZ a partir do
 * contexto base (igual à legenda hoje, que já é um caso particular de 1
 * nível), achatando (DFS) as colunas VISÍVEIS em `leavesOut` — cada uma com o
 * caminho de filtros completo (ancestrais + o próprio nível) para intersectar
 * com o contexto de qualquer linha depois. `node.value` é o subtotal do nível
 * (ou a folha) avaliado no contexto BASE, nunca a soma dos filhos.
 */
function buildColumnTree(
  model: ModelAccess,
  base: Table,
  ctx: FilterContext,
  levels: DrillLevel[],
  expansion: MatrixExpansion,
  primaryField: string,
  agg: string | undefined,
  depth: number,
  path: unknown[],
  steps: ColumnFilterStep[],
  leavesOut: ColumnLeaf[]
): MatrixColumnNode[] {
  const rc = resolveColumn(model, base, levels[depth].field);
  const keys = distinctKeys(model, ctx, rc, levels[depth]);
  const out: MatrixColumnNode[] = [];
  for (const k of keys) {
    path.push(k);
    steps.push({ rc, level: levels[depth], key: k });
    const nc = ctx.clone();
    for (const s of steps) applyLevelFilter(nc, s.rc, s.level, s.key);
    const value = primaryField ? evalField(model, nc, base, primaryField, agg) : null;
    const hasChildren = depth + 1 < levels.length;
    const expanded = hasChildren && isMatrixPathExpanded(expansion, path);
    const children = expanded
      ? buildColumnTree(model, base, nc, levels, expansion, primaryField, agg, depth + 1, path, steps, leavesOut)
      : (leavesOut.push({ steps: steps.slice() }), []);
    steps.pop();
    path.pop();
    out.push({ key: k, label: levelValueLabel(levels[depth], k), value, hasChildren, expanded, children });
  }
  return out;
}

function rowValuesForLeaves(model: ModelAccess, base: Table, rowCtx: FilterContext, leaves: ColumnLeaf[], primaryField: string, agg?: string): (number | null)[] {
  return leaves.map(leaf => {
    if (!primaryField) return null;
    const c = rowCtx.clone();
    for (const s of leaf.steps) applyLevelFilter(c, s.rc, s.level, s.key);
    return evalField(model, c, base, primaryField, agg);
  });
}

export function queryMatrixTs(model: ModelAccess, input: MatrixQueryInput): MatrixQueryResult {
  const base = model.getTable(input.visual.dataConfig.tableId);
  if (!base || input.levels.length === 0) return { seriesNames: [], rows: [], columnTotals: [], grandTotal: null };
  const ctx = baseContext(model, input.visual, base, input.filters, input.drillFilters || []);
  const columnLevels = input.columnLevels ?? [];
  const hasColumnLevels = columnLevels.length > 0;
  const series = seriesEval(model, input.visual, base, ctx);
  const cols = input.levels.map(l => resolveColumn(model, base, l.field));
  let budget = MAX_MATRIX_NODES;

  // v0.11 Rodada 2 (Frente I): com N níveis nas colunas, a árvore de colunas é
  // construída uma única vez (igual à legenda de hoje) e cada linha é
  // reavaliada contra as colunas VISÍVEIS (interseção linha×coluna pelo motor).
  const primaryField = (input.visual.dataConfig.yAxisFields || [])[0]?.replace(/^Σ\s*/, '').trim() ?? '';
  const agg = input.visual.dataConfig.aggregation;
  const columnLeaves: ColumnLeaf[] = [];
  const columnHeader = hasColumnLevels
    ? buildColumnTree(model, base, ctx, columnLevels, input.columnExpansion ?? COLLAPSED_MATRIX, primaryField, agg, 0, [], [], columnLeaves)
    : undefined;

  const build = (c: FilterContext, depth: number, path: unknown[]): MatrixNode[] => {
    const nodes: MatrixNode[] = [];
    for (const k of distinctKeys(model, c, cols[depth], input.levels[depth])) {
      if (budget <= 0) break;
      const nc = c.clone();
      applyLevelFilter(nc, cols[depth], input.levels[depth], k);
      const vals = hasColumnLevels ? rowValuesForLeaves(model, base, nc, columnLeaves, primaryField, agg) : series.values(nc);
      const rowTotal = series.rowTotal(nc);
      if (vals.every(v => v === null) && rowTotal === null) continue;
      budget--;
      const p = [...path, k];
      const hasChildren = depth + 1 < input.levels.length;
      const expanded = hasChildren && isMatrixPathExpanded(input.expansion, p);
      nodes.push({
        key: k,
        label: levelValueLabel(input.levels[depth], k),
        values: vals,
        rowTotal,
        hasChildren,
        expanded,
        children: expanded ? build(nc, depth + 1, p) : [],
      });
    }
    return nodes;
  };

  const rows = build(ctx, 0, []);
  const columnTotals = hasColumnLevels ? flattenVisibleColumnValues(columnHeader!) : series.values(ctx);
  return {
    seriesNames: hasColumnLevels ? [] : series.names,
    rows,
    columnTotals,
    grandTotal: series.rowTotal(ctx),
    columnHeader,
  };
}

function flattenVisibleColumnValues(nodes: MatrixColumnNode[]): (number | null)[] {
  const out: (number | null)[] = [];
  const walk = (ns: MatrixColumnNode[]) => {
    for (const n of ns) {
      if (n.expanded) walk(n.children);
      else out.push(n.value);
    }
  };
  walk(nodes);
  return out;
}

// ---------------------------------------------------------------------------
// Caminho nativo (Rust): montagem da requisição e conversão da resposta
// ---------------------------------------------------------------------------

export function toNativeLevels(levels: DrillLevel[]) {
  return levels.map(l => ({ field: l.field, datePart: l.datePart ?? null }));
}

export function toNativeDrillFilters(filters: DrillFilterSpec[]) {
  return filters.map(f => ({ field: f.field, datePart: f.datePart ?? null, value: f.value ?? null }));
}

/** Mesma conversão de filtros do relatório usada pelo VisualCard para o caminho nativo.
 *
 * v0.11 Rodada 2, Frente H/E (bug fix): preserva `f.operator` em vez de
 * espalhar `f.value` direto em `values` — um filtro `between` (segmentador
 * "Entre" / janelas de data relativa) guarda seu intervalo como
 * `value: [from, to]`, e sem o operador isso virava uma lista IN com só as
 * duas pontas (match apenas em `from` e `to`, nunca no meio). O Rust
 * (`NativeFilterInput.operator`, `FilterContext::range_filters`) usa o
 * operador para aplicar um range real — ver o mesmo fix em `VisualCard.tsx`.
 */
export function toNativeFilters(filters: Filter[]): NativeVisualQueryRequest['filters'] {
  return filters
    .map(f => ({
      tableId: f.tableId,
      column: f.column,
      operator: f.operator,
      values: Array.isArray(f.value) ? f.value : f.value !== undefined && f.value !== '' ? [f.value] : [],
    }))
    .filter(f => f.values.length > 0);
}

export function baseNativeRequest(visual: Visual, filters: Filter[]): NativeVisualQueryRequest {
  return {
    visualId: visual.id,
    visualType: visual.type,
    tableId: visual.dataConfig.tableId,
    xAxisField: visual.dataConfig.xAxisField,
    legendField: visual.dataConfig.legendField,
    yAxisFields: visual.dataConfig.yAxisFields || [],
    aggregation: visual.dataConfig.aggregation,
    filters: toNativeFilters(filters),
  };
}

export function levelsFromNative(res: NativeVisualQueryResult, levels: DrillLevel[]): LevelQueryResult {
  const keys = res.categoryKeys || [];
  return {
    categories: res.categories.map((c, i) => (keys[i] ? concatLevelLabels(levels, keys[i]) : c)),
    categoryKeys: keys,
    series: res.series,
  };
}

export function matrixFromNative(res: NativeVisualQueryResult, levels: DrillLevel[]): MatrixQueryResult {
  const conv = (n: NativeMatrixNodeDto, depth: number): MatrixNode => ({
    key: n.key,
    label: levelValueLabel(levels[depth] ?? levels[levels.length - 1], n.key),
    values: n.values,
    rowTotal: n.rowTotal ?? null,
    hasChildren: n.hasChildren,
    expanded: n.expanded,
    children: (n.children || []).map(c => conv(c, depth + 1)),
  });
  const convCol = (n: NonNullable<NonNullable<NativeVisualQueryResult['matrix']>['columnHeader']>[number], depth: number, columnLevels: DrillLevel[]): MatrixColumnNode => ({
    key: n.key,
    label: levelValueLabel(columnLevels[depth] ?? columnLevels[columnLevels.length - 1], n.key),
    value: n.value,
    hasChildren: n.hasChildren,
    expanded: n.expanded,
    children: (n.children || []).map(c => convCol(c, depth + 1, columnLevels)),
  });
  void convCol;
  return {
    seriesNames: res.series.map(s => s.name),
    rows: (res.matrix?.rows || []).map(n => conv(n, 0)),
    columnTotals: res.matrix?.columnTotals || [],
    grandTotal: res.matrix?.grandTotal ?? null,
    columnHeader: undefined,
  };
}

/** v0.11 Rodada 2 (Frente I): mesma conversão de `matrixFromNative`, mas
 * recebendo também os níveis de COLUNA para rotular `matrix.columnHeader`. */
export function matrixFromNativeWithColumns(res: NativeVisualQueryResult, rowLevels: DrillLevel[], columnLevels: DrillLevel[]): MatrixQueryResult {
  const base = matrixFromNative(res, rowLevels);
  const header = res.matrix?.columnHeader;
  if (!header) return base;
  const convCol = (n: NonNullable<typeof header>[number], depth: number): MatrixColumnNode => ({
    key: n.key,
    label: levelValueLabel(columnLevels[depth] ?? columnLevels[columnLevels.length - 1], n.key),
    value: n.value,
    hasChildren: n.hasChildren,
    expanded: n.expanded,
    children: (n.children || []).map(c => convCol(c, depth + 1)),
  });
  return { ...base, columnHeader: header.map(n => convCol(n, 0)), seriesNames: [] };
}
