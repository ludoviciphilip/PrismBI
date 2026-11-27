import React, { useMemo } from 'react';
import { ArrowDown, ArrowUp, ArrowUpDown, Filter as FilterIcon, SquareMinus, SquarePlus } from 'lucide-react';
import { Visual } from '../../../types/bi';
import { useBiStore } from '../../../store/useBiStore';
import { useTranslation } from '../../../i18n';
import { globalDataEngine, formatValue, formatCurrency, formatNumber, formatPercent } from '../../../engine/dataEngine';
import { applyVisualSort, fieldMatches, VisualSortSpec } from '../../../engine/visualSort';
import {
  BLANK_LABEL,
  DrillLevel,
  MatrixExpansion,
  buildPathFilters,
  describeDrillFilterSpecs,
  isMatrixPathExpanded,
  toggleMatrixPath,
} from '../../../engine/drill';
import { patchForHeaderClick } from '../visualSortOptions';
import { getFieldFormat, activeFormats, styleForTotalCell } from '../conditionalFormat/apply';
import { FormattedCell } from '../conditionalFormat/FormattedCell';
import { MatrixColumnNode, MatrixNode, MatrixQueryResult } from './engineQuery';

export interface HierarchicalMatrixProps {
  visual: Visual;
  levels: DrillLevel[];
  data: MatrixQueryResult | null;
  error: string | null;
  expansion: MatrixExpansion;
  onExpansionChange: (next: MatrixExpansion) => void;
  isCurrencyField: (field: string) => boolean;
  /** v0.10 Rodada 2 (seams F↔G): dica de página — recebe o caminho INTEIRO da
   * linha sob o mouse (todos os níveis acima, não só o primeiro). */
  onRowHover?: (e: React.MouseEvent, path: unknown[]) => void;
  onRowLeave?: () => void;
  /** v0.11 Rodada 2 (Frente I): N níveis nas COLUNAS. Com 2+ níveis a matriz
   * ganha um cabeçalho de colunas multinível (células mescladas por grupo,
   * +/− por grupo) em vez da linha única de `seriesNames`. */
  columnLevels?: DrillLevel[];
  columnExpansion?: MatrixExpansion;
  onColumnExpansionChange?: (next: MatrixExpansion) => void;
  /** Dica de página numa célula: caminho de LINHA + caminho de COLUNA (só quando
   * a matriz tem níveis nas colunas; para o caso de 1 nível o `onRowHover` basta). */
  onCellHover?: (e: React.MouseEvent, rowPath: unknown[], columnPath: unknown[]) => void;
}

interface FlatRow {
  node: MatrixNode;
  depth: number;
  path: unknown[];
}

interface ColumnLeaf {
  node: MatrixColumnNode;
  path: unknown[];
}

/** Ordena os irmãos com as mesmas regras de classificação do visual (cabeçalho/menu "Classificar por"). */
function sortSiblings(nodes: MatrixNode[], seriesNames: string[], spec: VisualSortSpec): MatrixNode[] {
  if (nodes.length < 2) return nodes;
  const byLabel = new Map(nodes.map(n => [n.label, n]));
  const sorted = applyVisualSort(
    { categories: nodes.map(n => n.label), series: seriesNames.map((name, i) => ({ name, data: nodes.map(n => n.values[i] ?? 0) })) },
    spec
  );
  return sorted.categories.map(c => byLabel.get(c)!).filter(Boolean);
}

/** Achata (DFS) as colunas VISÍVEIS (folha ou subtotal recolhido) — mesma ordem usada pelo motor para alinhar `values`. */
function flattenVisibleColumns(nodes: MatrixColumnNode[], path: unknown[] = []): ColumnLeaf[] {
  const out: ColumnLeaf[] = [];
  for (const n of nodes) {
    const p = [...path, n.key];
    if (n.expanded) out.push(...flattenVisibleColumns(n.children, p));
    else out.push({ node: n, path: p });
  }
  return out;
}

interface HeaderCell {
  node: MatrixColumnNode;
  path: unknown[];
  colSpan: number;
  /** Quantas linhas de cabeçalho esta célula ocupa para baixo (nó recolhido = até o fim). */
  rowSpan: number;
}

function countVisibleLeaves(node: MatrixColumnNode): number {
  if (!node.expanded || node.children.length === 0) return 1;
  return node.children.reduce((s, c) => s + countVisibleLeaves(c), 0) || 1;
}

/** Monta, por profundidade, as células de cabeçalho (com colSpan/rowSpan) — mesma técnica de célula mesclada do Power BI Desktop. */
function buildHeaderRows(nodes: MatrixColumnNode[], depth: number, maxDepth: number, path: unknown[], rows: HeaderCell[][]): void {
  rows[depth] = rows[depth] || [];
  for (const n of nodes) {
    const p = [...path, n.key];
    rows[depth].push({ node: n, path: p, colSpan: countVisibleLeaves(n), rowSpan: n.expanded ? 1 : maxDepth - depth });
    if (n.expanded) buildHeaderRows(n.children, depth + 1, maxDepth, p, rows);
  }
}

/**
 * Matriz com N níveis nas linhas (v0.10 Frente G) e, desde a v0.11 Rodada 2
 * (Frente I), também nas COLUNAS: cabeçalho multinível com células mescladas,
 * +/− por grupo. Cada linha, subtotal, coluna, subtotal de coluna e a
 * interseção de ambos vem do motor (avaliado no contexto de filtro do nó); o
 * React só desenha.
 */
export const HierarchicalMatrix: React.FC<HierarchicalMatrixProps> = ({
  visual,
  levels,
  data,
  error,
  expansion,
  onExpansionChange,
  isCurrencyField,
  onRowHover,
  onRowLeave,
  columnLevels = [],
  columnExpansion,
  onColumnExpansionChange,
  onCellHover,
}) => {
  const measures = useBiStore(s => s.measures);
  const updateVisual = useBiStore(s => s.updateVisual);
  const setCrossFilter = useBiStore(s => s.setCrossFilter);
  const setDrillCrossFilter = useBiStore(s => s.setDrillCrossFilter);
  const { t } = useTranslation();
  const mt = t.matrix;
  /** Substitui o rótulo padrão "(Em branco)" do motor (`BLANK_LABEL`) pela tradução ativa. */
  const localizeLabel = (label: string): string => (label === BLANK_LABEL ? mt.blank : label);
  const dc = visual.dataConfig;
  // v0.11 Rodada 2 (Frente I): com N níveis nas colunas, `seriesNames` fica
  // vazio e as "colunas" vêm de `data.columnHeader` (achatado em `columnLeaves`).
  const hasPivotColumns = columnLevels.length > 1 && Boolean(data?.columnHeader);
  const seriesNames = data?.seriesNames ?? [];
  const primaryY = dc.yAxisFields?.[0] || '';
  const useCurr = isCurrencyField(primaryY);
  const hasLegend = Boolean(dc.legendField) && !hasPivotColumns;
  const formats = activeFormats(visual);

  const columnLeaves = useMemo<ColumnLeaf[]>(() => (hasPivotColumns ? flattenVisibleColumns(data!.columnHeader!) : []), [hasPivotColumns, data]);
  const headerRows = useMemo<HeaderCell[][]>(() => {
    if (!hasPivotColumns) return [];
    const rows: HeaderCell[][] = [];
    buildHeaderRows(data!.columnHeader!, 0, columnLevels.length, [], rows);
    return rows;
  }, [hasPivotColumns, data, columnLevels.length]);
  // Colunas efetivamente iteradas no corpo/rodapé: folhas/subtotais recolhidos (modo N
  // níveis) ou as séries/legenda de hoje (modo antigo, sem mudança de comportamento).
  const columns: { label: string; idx: number; path: unknown[] }[] = hasPivotColumns
    ? columnLeaves.map((leaf, idx) => ({ label: leaf.node.label, idx, path: leaf.path }))
    : seriesNames.map((name, idx) => ({ label: name, idx, path: [] }));
  // v0.11 Rodada 2 (Frente I): com N níveis nas colunas há só 1 medida (a 1ª de
  // `yAxisFields`, igual ao motor); a formatação condicional (`applyTo`) casa
  // pelo NOME DA MEDIDA, nunca pelo rótulo da coluna (ex. "Norte"/"Sul").
  const condFmtFieldFor = (col: { label: string }) => (hasPivotColumns ? primaryY : col.label);

  const fmtCell = (val: number | null | undefined, colName?: string) => {
    if (val === null || val === undefined || Number.isNaN(val)) return '';
    if (colName) {
      const m = measures.find(ms => ms.name.toLowerCase() === colName.toLowerCase());
      if (m?.formatString) return formatValue(val, m.formatString);
      if (m?.format === 'percentage' || colName.includes('%')) return formatPercent(val);
      if (m?.format === 'currency' || isCurrencyField(colName)) return formatCurrency(val);
    }
    return useCurr ? formatCurrency(val) : formatNumber(val);
  };

  const sumNullable = (vals: (number | null | undefined)[]): number | null => {
    let acc = 0;
    let any = false;
    for (const v of vals) {
      if (v !== null && v !== undefined && Number.isFinite(v)) {
        acc += v;
        any = true;
      }
    }
    return any ? acc : null;
  };

  const rows: FlatRow[] = useMemo(() => {
    if (!data) return [];
    const base = globalDataEngine.getVisualSortSpec(visual);
    const specFor = (depth: number, nodes: MatrixNode[]): VisualSortSpec => {
      const lv = levels[depth];
      const indexKeys = new Map<string, unknown>(nodes.map((n, i) => [n.label, i]));
      return {
        sortField: dc.sortField,
        sortOrder: dc.sortOrder,
        limit: depth === 0 ? dc.limit : undefined,
        categoryFields: [...(base?.categoryFields ?? []), ...levels.map(l => l.field)],
        valueFields: [...(base?.valueFields ?? dc.yAxisFields ?? []), ...seriesNames],
        categorySortKeys: depth === 0 && !lv?.datePart ? base?.categorySortKeys : indexKeys,
      };
    };
    const out: FlatRow[] = [];
    const walk = (nodes: MatrixNode[], depth: number, path: unknown[]) => {
      for (const n of sortSiblings(nodes, seriesNames, specFor(depth, nodes))) {
        const p = [...path, n.key];
        out.push({ node: n, depth, path: p });
        if (n.expanded) walk(n.children, depth + 1, p);
      }
    };
    walk(data.rows, 0, []);
    return out;
  }, [data, visual, levels, seriesNames, dc.sortField, dc.sortOrder, dc.limit]);

  if (error) {
    return (
      <div className="p-3 rounded-lg bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-900 text-red-700 dark:text-red-300 text-[11px] overflow-auto">
        <div className="font-bold mb-1">Erro ao calcular visual</div>
        <div className="font-mono break-words">{error}</div>
      </div>
    );
  }

  const isSortedBy = (field: string) => !!dc.sortField && (dc.sortField === field || fieldMatches(dc.sortField, field));
  const sortArrow = (field: string) =>
    !isSortedBy(field) ? (
      <ArrowUpDown className="w-3 h-3 opacity-0 group-hover/th:opacity-40 shrink-0" aria-hidden />
    ) : dc.sortOrder === 'desc' ? (
      <ArrowDown className="w-3 h-3 text-amber-600 dark:text-amber-400 shrink-0" aria-label="decrescente" />
    ) : (
      <ArrowUp className="w-3 h-3 text-amber-600 dark:text-amber-400 shrink-0" aria-label="crescente" />
    );
  const ariaSort = (field: string): 'ascending' | 'descending' | 'none' =>
    isSortedBy(field) ? (dc.sortOrder === 'desc' ? 'descending' : 'ascending') : 'none';
  const sortBy = (field: string) => updateVisual(visual.id, { dataConfig: { ...dc, ...patchForHeaderClick(visual, field) } });

  const rowField = levels[0]?.field;
  const showTotalCol = hasPivotColumns ? true : seriesNames.length > 1;
  // Com legenda (ou N níveis nas colunas) o total da linha vem do motor (medida sem o
  // filtro da coluna). Sem legenda e com várias medidas, mantém o comportamento antigo.
  const rowTotalOf = (n: MatrixNode): number | null =>
    hasLegend || hasPivotColumns ? n.rowTotal ?? null : sumNullable(n.values);
  const grandTotal: number | null = hasLegend || hasPivotColumns
    ? data?.grandTotal ?? null
    : sumNullable(data?.columnTotals ?? []);
  // Valores visíveis por série (gradiente/regras da formatação condicional), separados por
  // nível: subtotais não entram no mínimo/máximo das folhas e vice-versa; células em branco ficam de fora.
  const onlyNums = (arr: (number | null | undefined)[]): number[] =>
    arr.filter((v): v is number => v !== null && v !== undefined && Number.isFinite(v));
  const visibleAt = (depth: number) => columns.map(col => onlyNums(rows.filter(r => r.depth === depth).map(r => r.node.values[col.idx])));
  const visibleByDepth = levels.map((_, d) => visibleAt(d));
  // v0.10 Frente H→G: cada nível (subtotal) tem a própria escala para `styleForTotalCell`,
  // igual ao total da linha, aos totais de coluna e ao total geral abaixo.
  const lastDepth = levels.length - 1;
  const rowTotalsByDepth = levels.map((_, d) => onlyNums(rows.filter(r => r.depth === d).map(r => rowTotalOf(r.node))));
  const colTotalsVisible = onlyNums(data?.columnTotals ?? []);

  const headerRowSpanTotal = hasPivotColumns ? columnLevels.length : 1;
  const toggleColumnPath = (path: unknown[]) => onColumnExpansionChange?.(toggleMatrixPath(columnExpansion ?? { depth: 0, expanded: [], collapsed: [] }, path));

  return (
    <div className="w-full h-full overflow-auto text-xs" data-testid="hierarchical-matrix">
      <table className="min-w-full border-separate border-spacing-0 text-left">
        <thead className="bg-slate-100 dark:bg-slate-800">
          {hasPivotColumns ? (
            headerRows.map((cells, depth) => (
              <tr key={`hdr-${depth}`}>
                {depth === 0 && (
                  <th
                    rowSpan={headerRowSpanTotal}
                    aria-sort={rowField ? ariaSort(rowField) : 'none'}
                    onClick={() => rowField && sortBy(rowField)}
                    title="Clique para classificar por esta coluna"
                    className="group/th sticky top-0 z-10 bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 font-bold text-slate-800 dark:text-slate-100 border-b border-r border-slate-200 dark:border-slate-700 cursor-pointer hover:text-amber-600 select-none align-bottom"
                  >
                    <span className="inline-flex items-center gap-1" title={levels.map(l => l.caption).join(' › ')}>
                      {levels[0]?.datePart ? levels[0].caption : dc.xAxisField || levels[0]?.caption || mt.rowsLabel}
                      {rowField && sortArrow(rowField)}
                    </span>
                  </th>
                )}
                {cells.map(cell => (
                  <th
                    key={`${depth}-${JSON.stringify(cell.path)}`}
                    colSpan={cell.colSpan}
                    rowSpan={cell.rowSpan}
                    className="sticky top-0 z-10 bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 font-semibold text-center text-slate-700 dark:text-slate-200 border-b border-l border-slate-200 dark:border-slate-700 select-none"
                  >
                    <span className="inline-flex items-center justify-center gap-1">
                      {cell.node.hasChildren && (
                        <button
                          type="button"
                          aria-label={`${cell.node.expanded ? 'Recolher' : 'Expandir'} coluna ${cell.node.label}`}
                          aria-expanded={cell.node.expanded}
                          title={cell.node.expanded ? 'Recolher grupo de coluna' : `Expandir sub-nível (${columnLevels[depth + 1]?.caption ?? ''})`}
                          onClick={e => {
                            e.stopPropagation();
                            toggleColumnPath(cell.path);
                          }}
                          className="p-0.5 rounded hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-500 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400"
                        >
                          {cell.node.expanded ? <SquareMinus className="w-3 h-3" /> : <SquarePlus className="w-3 h-3" />}
                        </button>
                      )}
                      <span>{localizeLabel(cell.node.label)}</span>
                    </span>
                  </th>
                ))}
                {depth === 0 && showTotalCol && (
                  <th
                    rowSpan={headerRowSpanTotal}
                    className="sticky top-0 z-10 px-2.5 py-1.5 font-bold text-right text-slate-900 dark:text-amber-300 border-b border-l border-slate-200 dark:border-slate-700 bg-slate-200 dark:bg-slate-800 align-bottom"
                  >
                    {mt.total}
                  </th>
                )}
              </tr>
            ))
          ) : (
            <tr>
              <th
                aria-sort={rowField ? ariaSort(rowField) : 'none'}
                onClick={() => rowField && sortBy(rowField)}
                title="Clique para classificar por esta coluna"
                className="group/th sticky top-0 z-10 bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 font-bold text-slate-800 dark:text-slate-100 border-b border-r border-slate-200 dark:border-slate-700 cursor-pointer hover:text-amber-600 select-none"
              >
                <span className="inline-flex items-center gap-1" title={levels.map(l => l.caption).join(' › ')}>
                  {levels[0]?.datePart ? levels[0].caption : dc.xAxisField || levels[0]?.caption || mt.rowsLabel}
                  {rowField && sortArrow(rowField)}
                </span>
              </th>
              {seriesNames.map(name => (
                <th
                  key={name}
                  aria-sort={ariaSort(name)}
                  onClick={() => sortBy(name)}
                  title="Clique para classificar por esta coluna"
                  className="group/th sticky top-0 z-10 bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 font-semibold text-right text-slate-700 dark:text-slate-200 border-b border-slate-200 dark:border-slate-700 cursor-pointer hover:text-amber-600 select-none"
                >
                  <span className="inline-flex items-center justify-end gap-1 w-full">
                    {dc.legendField && (
                      <button
                        type="button"
                        onClick={e => {
                          e.stopPropagation();
                          setCrossFilter(dc.tableId, dc.legendField!, name);
                        }}
                        className="p-0.5 rounded text-slate-400 hover:text-amber-600 hover:bg-slate-200 dark:hover:bg-slate-700"
                        title={`Filtrar por ${name}`}
                      >
                        <FilterIcon className="w-2.5 h-2.5" />
                      </button>
                    )}
                    {sortArrow(name)}
                    {name}
                  </span>
                </th>
              ))}
              {showTotalCol && (
                <th className="sticky top-0 z-10 px-2.5 py-1.5 font-bold text-right text-slate-900 dark:text-amber-300 border-b border-l border-slate-200 dark:border-slate-700 bg-slate-200 dark:bg-slate-800">
                  {mt.total}
                </th>
              )}
            </tr>
          )}
        </thead>
        <tbody className="divide-y divide-slate-100 dark:divide-slate-800 bg-white dark:bg-slate-900">
          {rows.map(({ node, depth, path }) => {
            const isTop = depth === 0;
            const rowId = JSON.stringify(path);
            const visibleBySeries = visibleByDepth[depth] ?? [];
            return (
              <tr
                key={rowId}
                data-depth={depth}
                className={isTop ? 'hover:bg-amber-50/50 dark:hover:bg-slate-800/60' : 'bg-slate-50/70 dark:bg-slate-800/30'}
              >
                <td
                  onClick={() => {
                    // v0.11 Frente D: cross-filter pelo caminho inteiro da linha (ancestrais +
                    // nível atual) — funciona tanto para campos comuns quanto para níveis de
                    // data (ano/trimestre/mês/dia), sempre calculado pelo motor (`drillFilters`).
                    const specs = buildPathFilters(levels.slice(0, depth + 1), path);
                    if (specs.length === 0) return;
                    if (specs.length === 1 && !specs[0].datePart) {
                      setCrossFilter(dc.tableId, specs[0].field, specs[0].value);
                      return;
                    }
                    setDrillCrossFilter(
                      specs.map(s => ({ field: s.field, datePart: s.datePart, value: s.value, label: describeDrillFilterSpecs(levels, [s]) }))
                    );
                  }}
                  onMouseEnter={e => onRowHover?.(e, path)}
                  onMouseLeave={() => onRowLeave?.()}
                  style={{ paddingLeft: 10 + depth * 18 }}
                  className={
                    isTop
                      ? 'pr-2.5 py-1 font-semibold text-slate-700 dark:text-slate-200 border-r border-slate-200 dark:border-slate-800 cursor-pointer hover:text-amber-600 whitespace-nowrap'
                      : 'pr-2.5 py-1 text-[11px] text-slate-600 dark:text-slate-300 border-r border-slate-200 dark:border-slate-800 whitespace-nowrap'
                  }
                >
                  <div className="flex items-center gap-1">
                    {node.hasChildren && (
                      <button
                        type="button"
                        aria-label={`${node.expanded ? 'Recolher' : 'Expandir'} ${node.label}`}
                        aria-expanded={node.expanded}
                        title={node.expanded ? 'Recolher nível' : `Expandir sub-nível (${levels[depth + 1]?.caption ?? ''})`}
                        onClick={e => {
                          e.stopPropagation();
                          onExpansionChange(toggleMatrixPath(expansion, path));
                        }}
                        className="p-0.5 rounded hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-500 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400"
                      >
                        {node.expanded ? <SquareMinus className="w-3 h-3" /> : <SquarePlus className="w-3 h-3" />}
                      </button>
                    )}
                    <span>{localizeLabel(node.label)}</span>
                  </div>
                </td>
                {columns.map(col => {
                  const rawVal = node.values[col.idx];
                  if (rawVal === null || rawVal === undefined || Number.isNaN(rawVal)) {
                    return (
                      <td
                        key={col.label + col.idx}
                        onMouseEnter={hasPivotColumns ? e => onCellHover?.(e, path, col.path) : undefined}
                        className={
                          isTop
                            ? 'px-2.5 py-1 text-right font-mono text-slate-600 dark:text-slate-300 whitespace-nowrap'
                            : 'px-2.5 py-1 text-right font-mono text-[11px] text-slate-500 dark:text-slate-400 whitespace-nowrap'
                        }
                      />
                    );
                  }
                  const val = Number(rawVal);
                  const isSubtotalRow = depth < lastDepth;
                  if (isSubtotalRow) {
                    // Subtotal (nível intermediário): só formatos com applyToTotals, escala do próprio nível.
                    const totalStyle = styleForTotalCell(formats, condFmtFieldFor(col), val, visibleByDepth[depth]?.[col.idx] ?? []);
                    return (
                      <td
                        key={col.label + col.idx}
                        style={totalStyle}
                        onMouseEnter={hasPivotColumns ? e => onCellHover?.(e, path, col.path) : undefined}
                        className="px-2.5 py-1 text-right font-mono text-[11px] text-slate-500 dark:text-slate-400 whitespace-nowrap"
                      >
                        {fmtCell(val, col.label)}
                      </td>
                    );
                  }
                  const fmt = getFieldFormat(formats, condFmtFieldFor(col), val, visibleBySeries[col.idx] ?? [], {
                    value: f => {
                      const i = columns.findIndex(c => c.label === f);
                      return i >= 0 ? node.values[i] : undefined;
                    },
                    visible: f => {
                      const i = columns.findIndex(c => c.label === f);
                      return i >= 0 ? visibleBySeries[i] ?? [] : [];
                    },
                  });
                  return (
                    <FormattedCell
                      key={col.label + col.idx}
                      format={fmt}
                      onMouseEnter={hasPivotColumns ? e => onCellHover?.(e, path, col.path) : undefined}
                      className={
                        isTop
                          ? 'px-2.5 py-1 text-right font-mono text-slate-600 dark:text-slate-300 whitespace-nowrap'
                          : 'px-2.5 py-1 text-right font-mono text-[11px] text-slate-500 dark:text-slate-400 whitespace-nowrap'
                      }
                    >
                      {fmtCell(val, col.label)}
                    </FormattedCell>
                  );
                })}
                {showTotalCol && (() => {
                  const rowTot = rowTotalOf(node);
                  return (
                    <td
                      className={
                        isTop
                          ? 'px-2.5 py-1 text-right font-mono font-bold text-slate-900 dark:text-slate-100 border-l border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-800/40 whitespace-nowrap'
                          : 'px-2.5 py-1 text-right font-mono text-[11px] font-semibold text-slate-700 dark:text-slate-200 border-l border-slate-200 dark:border-slate-800 whitespace-nowrap'
                      }
                      style={rowTot !== null ? styleForTotalCell(formats, primaryY, rowTot, rowTotalsByDepth[depth] ?? []) : undefined}
                    >
                      {fmtCell(rowTot)}
                    </td>
                  );
                })()}
              </tr>
            );
          })}
        </tbody>
        {/* v0.10 Frente H→G: formatação condicional dos totais/subtotais ligada —
            `styleForTotalCell` (apply.ts) nos subtotais de cada nível, no total da
            linha, nos totais de coluna abaixo (também os subtotais de coluna da
            v0.11 Rodada 2/Frente I) e no total geral, cada grupo com a própria
            escala (ver `rowTotalsByDepth`, `colTotalsVisible`, `[grandTotal]`). */}
        <tfoot className="bg-slate-100 dark:bg-slate-800 font-bold">
          <tr>
            <td className="bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 text-slate-900 dark:text-amber-300 border-t-2 border-r border-slate-300 dark:border-slate-700">
              {mt.grandTotal}
            </td>
            {columns.map(col => {
              const colTot = data?.columnTotals[col.idx] ?? null;
              return (
                <td
                  key={col.label + col.idx}
                  data-testid={`matrix-total-${col.label}`}
                  className="bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 text-right font-mono text-slate-900 dark:text-amber-300 border-t-2 border-slate-300 dark:border-slate-700 whitespace-nowrap"
                  style={colTot !== null ? styleForTotalCell(formats, condFmtFieldFor(col), colTot, colTotalsVisible) : undefined}
                >
                  {fmtCell(colTot, col.label)}
                </td>
              );
            })}
            {showTotalCol && (
              <td
                className="bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 text-right font-mono text-slate-950 dark:text-amber-400 border-t-2 border-l border-slate-300 dark:border-slate-700 whitespace-nowrap"
                style={grandTotal !== null ? styleForTotalCell(formats, primaryY, grandTotal, [grandTotal]) : undefined}
              >
                {fmtCell(grandTotal)}
              </td>
            )}
          </tr>
        </tfoot>
      </table>
    </div>
  );
};
