import React, { useMemo } from 'react';
import { ArrowDown, ArrowUp, ArrowUpDown, Filter as FilterIcon, SquareMinus, SquarePlus } from 'lucide-react';
import { Visual } from '../../../types/bi';
import { useBiStore } from '../../../store/useBiStore';
import { globalDataEngine, formatValue, formatCurrency, formatNumber, formatPercent } from '../../../engine/dataEngine';
import { applyVisualSort, fieldMatches, VisualSortSpec } from '../../../engine/visualSort';
import { DrillLevel, MatrixExpansion, toggleMatrixPath } from '../../../engine/drill';
import { patchForHeaderClick } from '../visualSortOptions';
import { getFieldFormat, activeFormats } from '../conditionalFormat/apply';
import { FormattedCell } from '../conditionalFormat/FormattedCell';
import { MatrixNode, MatrixQueryResult } from './engineQuery';

export interface HierarchicalMatrixProps {
  visual: Visual;
  levels: DrillLevel[];
  data: MatrixQueryResult | null;
  error: string | null;
  expansion: MatrixExpansion;
  onExpansionChange: (next: MatrixExpansion) => void;
  isCurrencyField: (field: string) => boolean;
}

interface FlatRow {
  node: MatrixNode;
  depth: number;
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

/**
 * Matriz com N níveis nas linhas (v0.10 Frente G). Cada linha, subtotal e total
 * vem do motor (avaliado no contexto de filtro do nível); o React só desenha.
 */
export const HierarchicalMatrix: React.FC<HierarchicalMatrixProps> = ({
  visual,
  levels,
  data,
  error,
  expansion,
  onExpansionChange,
  isCurrencyField,
}) => {
  const measures = useBiStore(s => s.measures);
  const updateVisual = useBiStore(s => s.updateVisual);
  const setCrossFilter = useBiStore(s => s.setCrossFilter);
  const dc = visual.dataConfig;
  const seriesNames = data?.seriesNames ?? [];
  const primaryY = dc.yAxisFields?.[0] || '';
  const useCurr = isCurrencyField(primaryY);
  const hasLegend = Boolean(dc.legendField);
  const formats = activeFormats(visual);

  const fmtCell = (val: number, colName?: string) => {
    if (colName) {
      const m = measures.find(ms => ms.name.toLowerCase() === colName.toLowerCase());
      if (m?.formatString) return formatValue(val, m.formatString);
      if (m?.format === 'percentage' || colName.includes('%')) return formatPercent(val);
      if (m?.format === 'currency' || isCurrencyField(colName)) return formatCurrency(val);
    }
    return useCurr ? formatCurrency(val) : formatNumber(val);
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
  const showTotalCol = seriesNames.length > 1;
  // Com legenda o total da linha vem do motor (medida sem o filtro da coluna).
  // Sem legenda e com várias medidas, mantém o comportamento antigo (soma das medidas da linha).
  const rowTotalOf = (n: MatrixNode) => (hasLegend ? n.rowTotal ?? 0 : n.values.reduce((a, b) => a + (Number(b) || 0), 0));
  const grandTotal = hasLegend ? data?.grandTotal ?? 0 : (data?.columnTotals ?? []).reduce((a, b) => a + (Number(b) || 0), 0);
  // Valores visíveis por série (gradiente/regras da formatação condicional), separados por
  // nível: subtotais não entram no mínimo/máximo das folhas e vice-versa.
  const visibleAt = (depth: number) =>
    seriesNames.map((_, i) => rows.filter(r => r.depth === depth).map(r => r.node.values[i] ?? 0));
  const visibleByDepth = levels.map((_, d) => visibleAt(d));

  return (
    <div className="w-full h-full overflow-auto text-xs" data-testid="hierarchical-matrix">
      <table className="min-w-full border-separate border-spacing-0 text-left">
        <thead className="bg-slate-100 dark:bg-slate-800">
          <tr>
            <th
              aria-sort={rowField ? ariaSort(rowField) : 'none'}
              onClick={() => rowField && sortBy(rowField)}
              title="Clique para classificar por esta coluna"
              className="group/th sticky top-0 z-10 bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 font-bold text-slate-800 dark:text-slate-100 border-b border-r border-slate-200 dark:border-slate-700 cursor-pointer hover:text-amber-600 select-none"
            >
              <span className="inline-flex items-center gap-1" title={levels.map(l => l.caption).join(' › ')}>
                {levels[0]?.datePart ? levels[0].caption : dc.xAxisField || levels[0]?.caption || 'Linhas'}
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
                Total
              </th>
            )}
          </tr>
        </thead>
        <tbody className="divide-y divide-slate-100 dark:divide-slate-800 bg-white dark:bg-slate-900">
          {rows.map(({ node, depth, path }) => {
            const isTop = depth === 0;
            const level = levels[depth];
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
                    if (level && !level.datePart) setCrossFilter(dc.tableId, level.field, node.key);
                  }}
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
                    <span>{node.label}</span>
                  </div>
                </td>
                {seriesNames.map((name, sIdx) => {
                  const val = Number(node.values[sIdx]) || 0;
                  const fmt = getFieldFormat(formats, name, val, visibleBySeries[sIdx] ?? [], {
                    value: f => {
                      const i = seriesNames.indexOf(f);
                      return i >= 0 ? node.values[i] : undefined;
                    },
                    visible: f => {
                      const i = seriesNames.indexOf(f);
                      return i >= 0 ? visibleBySeries[i] ?? [] : [];
                    },
                  });
                  return (
                    <FormattedCell
                      key={name}
                      format={fmt}
                      className={
                        isTop
                          ? 'px-2.5 py-1 text-right font-mono text-slate-600 dark:text-slate-300 whitespace-nowrap'
                          : 'px-2.5 py-1 text-right font-mono text-[11px] text-slate-500 dark:text-slate-400 whitespace-nowrap'
                      }
                    >
                      {fmtCell(val, name)}
                    </FormattedCell>
                  );
                })}
                {showTotalCol && (
                  <td
                    className={
                      isTop
                        ? 'px-2.5 py-1 text-right font-mono font-bold text-slate-900 dark:text-slate-100 border-l border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-800/40 whitespace-nowrap'
                        : 'px-2.5 py-1 text-right font-mono text-[11px] font-semibold text-slate-700 dark:text-slate-200 border-l border-slate-200 dark:border-slate-800 whitespace-nowrap'
                    }
                  >
                    {fmtCell(rowTotalOf(node))}
                  </td>
                )}
              </tr>
            );
          })}
        </tbody>
        {/* Integração com a Frente H (feat/v0.10-condfmt2): quando `styleForTotalCell` existir em
            conditionalFormat/apply.ts, aplicá-lo nas células de subtotal (linhas com depth < último nível
            e coluna Total) e nos totais abaixo, como a Frente H fez na matriz antiga do VisualCard. */}
        <tfoot className="bg-slate-100 dark:bg-slate-800 font-bold">
          <tr>
            <td className="bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 text-slate-900 dark:text-amber-300 border-t-2 border-r border-slate-300 dark:border-slate-700">
              Total Geral
            </td>
            {seriesNames.map((name, cIdx) => (
              <td
                key={name}
                data-testid={`matrix-total-${name}`}
                className="bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 text-right font-mono text-slate-900 dark:text-amber-300 border-t-2 border-slate-300 dark:border-slate-700 whitespace-nowrap"
              >
                {fmtCell(data?.columnTotals[cIdx] ?? 0, name)}
              </td>
            ))}
            {showTotalCol && (
              <td className="bg-slate-100 dark:bg-slate-800 px-2.5 py-1.5 text-right font-mono text-slate-950 dark:text-amber-400 border-t-2 border-l border-slate-300 dark:border-slate-700 whitespace-nowrap">
                {fmtCell(grandTotal)}
              </td>
            )}
          </tr>
        </tfoot>
      </table>
    </div>
  );
};
