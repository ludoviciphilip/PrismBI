/**
 * v0.10 Frente G: drill-down hierárquico (lógica pura, sem React e sem motor).
 *
 * Conceitos (paridade com o Power BI Desktop):
 * - Hierarquia ("níveis"): para gráficos, `[xAxisField, ...drillFields]`; se o eixo
 *   for uma coluna de data e `drillFields` estiver ausente, a hierarquia automática
 *   Ano → Trimestre → Mês → Dia sobre a mesma coluna. `drillFields: []` explícito
 *   desliga a hierarquia (é assim que projetos antigos são migrados, para abrirem
 *   iguais). Na matriz, `matrixRowFields` (N níveis).
 * - Estado: `currentDrillLevel` (índice do nível), `drillMode` ('drill' | 'expand')
 *   e `drillPath` (valor escolhido em cada nível acima do atual; `DRILL_ALL` = "ir
 *   para o próximo nível" sem filtrar por um ponto).
 * - O caminho vira filtros (`DrillFilterSpec`) que o motor aplica no contexto de
 *   filtro (Rust: `build_filter_context`; TS: `FilterContext`), nunca no React.
 */
import { DataType, VisualDataConfig } from '../types/bi';
import { parseDate } from './shared/valueParser';

export type DatePart = 'year' | 'quarter' | 'month' | 'day';
export const DATE_PARTS: DatePart[] = ['year', 'quarter', 'month', 'day'];
export const DATE_PART_CAPTIONS: Record<DatePart, string> = {
  year: 'Ano',
  quarter: 'Trimestre',
  month: 'Mês',
  day: 'Dia',
};

export interface DrillLevel {
  /** Campo como no dataConfig (pode ser qualificado: "Tabela.Coluna"). */
  field: string;
  /** Parte da data quando o nível vem da hierarquia automática de data. */
  datePart?: DatePart;
  /** Rótulo do nível (cabeçalho/tooltip). */
  caption: string;
}

export interface DrillFilterSpec {
  field: string;
  datePart?: DatePart;
  value: unknown;
}

export interface DrillState {
  level: number;
  mode: 'drill' | 'expand';
  path: unknown[];
}

export const DRILL_ALL: { $all: true } = Object.freeze({ $all: true }) as { $all: true };

export function isDrillAll(v: unknown): boolean {
  return typeof v === 'object' && v !== null && (v as { $all?: unknown }).$all === true;
}

/** Tipos de visual que ganham os controles de drill no cabeçalho. */
export const DRILLABLE_VISUAL_TYPES = new Set([
  'column',
  'stacked-column',
  'bar',
  'stacked-bar',
  'line',
  'area',
  'pie',
  'donut',
]);

export type ColumnTypeLookup = (field: string) => DataType | undefined;

function plainLevel(field: string): DrillLevel {
  return { field, caption: field };
}

function dateHierarchy(field: string): DrillLevel[] {
  return DATE_PARTS.map(part => ({ field, datePart: part, caption: DATE_PART_CAPTIONS[part] }));
}

/** Níveis da hierarquia do eixo de um gráfico. */
export function resolveDrillLevels(dc: VisualDataConfig, typeOf: ColumnTypeLookup): DrillLevel[] {
  const x = dc.xAxisField;
  if (!x) return [];
  if (dc.drillFields && dc.drillFields.length > 0) {
    return [plainLevel(x), ...dc.drillFields.filter(Boolean).map(plainLevel)];
  }
  if (dc.drillFields === undefined && typeOf(x) === 'date') return dateHierarchy(x);
  return [plainLevel(x)];
}

/** Níveis das linhas da matriz (N níveis). */
export function resolveMatrixRowLevels(dc: VisualDataConfig, typeOf: ColumnTypeLookup): DrillLevel[] {
  const rows = (dc.matrixRowFields || []).filter(Boolean);
  if (rows.length > 1) return rows.map(plainLevel);
  if (rows.length === 1) return resolveDrillLevels({ ...dc, xAxisField: rows[0] }, typeOf);
  return resolveDrillLevels(dc, typeOf);
}

export function getDrillState(dc: VisualDataConfig, levels: DrillLevel[]): DrillState {
  const max = Math.max(0, levels.length - 1);
  const raw = Number.isFinite(dc.currentDrillLevel) ? Math.trunc(dc.currentDrillLevel as number) : 0;
  const level = Math.min(max, Math.max(0, raw));
  const mode = dc.drillMode === 'expand' && level > 0 ? 'expand' : 'drill';
  const path = Array.isArray(dc.drillPath) ? dc.drillPath.slice(0, level) : [];
  return { level, mode, path };
}

export function canDrillUp(state: DrillState): boolean {
  return state.level > 0;
}

export function canDrillDown(state: DrillState, levels: DrillLevel[]): boolean {
  return state.level < levels.length - 1;
}

type DrillPatch = Pick<VisualDataConfig, 'currentDrillLevel' | 'drillPath' | 'drillMode'>;

export function drillUpPatch(dc: VisualDataConfig, levels: DrillLevel[]): DrillPatch | null {
  const st = getDrillState(dc, levels);
  if (!canDrillUp(st)) return null;
  const level = st.level - 1;
  if (st.mode === 'expand') {
    return { currentDrillLevel: level, drillPath: [], drillMode: level === 0 ? 'drill' : 'expand' };
  }
  return { currentDrillLevel: level, drillPath: st.path.slice(0, level), drillMode: 'drill' };
}

/**
 * Desce filtrando pelo ponto clicado. `pointKeys` = valores do ponto em cada nível
 * agrupado (1 valor no modo drill; a tupla inteira no modo expand).
 */
export function drillDownOnPointPatch(
  dc: VisualDataConfig,
  levels: DrillLevel[],
  pointKeys: unknown[]
): DrillPatch | null {
  const st = getDrillState(dc, levels);
  if (!canDrillDown(st, levels) || pointKeys.length === 0) return null;
  const path =
    st.mode === 'expand'
      ? pointKeys.slice(0, st.level + 1)
      : [...padPath(st.path, st.level), pointKeys[pointKeys.length - 1]];
  return { currentDrillLevel: st.level + 1, drillPath: path, drillMode: 'drill' };
}

/** ⇊ Ir para o próximo nível: todos os valores, sem filtrar por um ponto. */
export function goToNextLevelPatch(dc: VisualDataConfig, levels: DrillLevel[]): DrillPatch | null {
  const st = getDrillState(dc, levels);
  if (!canDrillDown(st, levels)) return null;
  const base = st.mode === 'expand' ? [] : padPath(st.path, st.level);
  return { currentDrillLevel: st.level + 1, drillPath: [...padPath(base, st.level), DRILL_ALL], drillMode: 'drill' };
}

/** Expandir todos os níveis um nível abaixo (rótulos concatenados, ex. "2024 Trim 1"). */
export function expandAllLevelsPatch(dc: VisualDataConfig, levels: DrillLevel[]): DrillPatch | null {
  const st = getDrillState(dc, levels);
  if (!canDrillDown(st, levels)) return null;
  return { currentDrillLevel: st.level + 1, drillPath: [], drillMode: 'expand' };
}

function padPath(path: unknown[], len: number): unknown[] {
  const out = path.slice(0, len);
  while (out.length < len) out.push(DRILL_ALL);
  return out;
}

/** Níveis pelos quais o visual agrupa no estado atual. */
export function groupingLevels(state: DrillState, levels: DrillLevel[]): DrillLevel[] {
  if (levels.length === 0) return [];
  if (state.mode === 'expand') return levels.slice(0, state.level + 1);
  return [levels[state.level]];
}

/** Filtros do caminho de drill (níveis acima do atual), para o contexto de filtro do motor. */
export function buildDrillPathFilters(state: DrillState, levels: DrillLevel[]): DrillFilterSpec[] {
  if (state.mode === 'expand') return [];
  const out: DrillFilterSpec[] = [];
  for (let i = 0; i < state.level && i < levels.length; i++) {
    if (i >= state.path.length) continue;
    const value = state.path[i];
    if (isDrillAll(value)) continue;
    const lv = levels[i];
    out.push(lv.datePart ? { field: lv.field, datePart: lv.datePart, value } : { field: lv.field, value });
  }
  return out;
}

/** Valor numérico de uma parte da data (ano, trimestre 1-4, mês 1-12, dia 1-31). */
export function datePartValue(raw: unknown, part: DatePart): number | null {
  const { date } = parseDate(raw);
  if (!date) return null;
  const m = date.getUTCMonth() + 1;
  switch (part) {
    case 'year':
      return date.getUTCFullYear();
    case 'quarter':
      return Math.floor((m - 1) / 3) + 1;
    case 'month':
      return m;
    case 'day':
      return date.getUTCDate();
  }
}

const MONTHS_PT = [
  'janeiro',
  'fevereiro',
  'março',
  'abril',
  'maio',
  'junho',
  'julho',
  'agosto',
  'setembro',
  'outubro',
  'novembro',
  'dezembro',
];

export function formatDatePartLabel(part: DatePart, value: unknown): string {
  const n = Number(value);
  if (!Number.isFinite(n)) return String(value ?? '');
  switch (part) {
    case 'quarter':
      return `Trim ${n}`;
    case 'month':
      return MONTHS_PT[n - 1] ?? String(n);
    default:
      return String(n);
  }
}

export const BLANK_LABEL = '(Em branco)';

export function levelValueLabel(level: DrillLevel, value: unknown): string {
  if (value === null || value === undefined || value === '') return BLANK_LABEL;
  if (level.datePart) return formatDatePartLabel(level.datePart, value);
  return String(value);
}

/** Rótulo concatenado do "expandir todos os níveis" (ex. "2024 Trim 1"). */
export function concatLevelLabels(levels: DrillLevel[], keys: unknown[]): string {
  return levels.map((lv, i) => levelValueLabel(lv, keys[i])).join(' ');
}

// --- Matriz: estado de expansão das linhas ---

export interface MatrixExpansion {
  /** Todas as linhas com profundidade < depth ficam expandidas ("expandir um nível"/"expandir tudo"). */
  depth: number;
  /** Ids (matrixPathId) expandidos individualmente (+). */
  expanded: string[];
  /** Ids recolhidos individualmente (−) apesar de `depth`. */
  collapsed: string[];
}

export const COLLAPSED_MATRIX: MatrixExpansion = { depth: 0, expanded: [], collapsed: [] };

export function matrixPathId(path: unknown[]): string {
  return JSON.stringify(path);
}

export function isMatrixPathExpanded(exp: MatrixExpansion, path: unknown[]): boolean {
  const id = matrixPathId(path);
  if (exp.collapsed.includes(id)) return false;
  if (exp.expanded.includes(id)) return true;
  return path.length - 1 < exp.depth;
}

export function toggleMatrixPath(exp: MatrixExpansion, path: unknown[]): MatrixExpansion {
  const id = matrixPathId(path);
  const open = isMatrixPathExpanded(exp, path);
  const expanded = exp.expanded.filter(x => x !== id);
  const collapsed = exp.collapsed.filter(x => x !== id);
  return open
    ? { ...exp, expanded, collapsed: [...collapsed, id] }
    : { ...exp, expanded: [...expanded, id], collapsed };
}

export function expandOneLevel(exp: MatrixExpansion, levelCount: number): MatrixExpansion {
  return { depth: Math.min(exp.depth + 1, Math.max(0, levelCount - 1)), expanded: exp.expanded, collapsed: [] };
}

export function expandAllMatrix(levelCount: number): MatrixExpansion {
  return { depth: Math.max(0, levelCount - 1), expanded: [], collapsed: [] };
}
