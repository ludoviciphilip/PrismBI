import { Filter as BiFilter } from '../../../types/bi';

export interface TooltipPoint {
  categoryField?: string;
  categoryValue?: unknown;
  legendField?: string;
  legendValue?: unknown;
}

function isUsableValue(v: unknown): boolean {
  return v !== undefined && v !== null && v !== '';
}

/**
 * Monta filtros TEMPORÁRIOS (nunca gravados no store) a partir do ponto sob o
 * mouse: o valor da categoria (eixo X) e, quando existir, o valor da legenda.
 * São filtros de nível 'visual' passados só para a consulta da miniatura —
 * ver `build_filter_context` no motor (mesmo contrato usado pelo drill-through).
 */
export function buildTemporaryFilters(tableId: string, point: TooltipPoint): BiFilter[] {
  const filters: BiFilter[] = [];

  if (point.categoryField && isUsableValue(point.categoryValue)) {
    filters.push({
      id: `__tooltip_cat_${point.categoryField}`,
      level: 'visual',
      tableId,
      column: point.categoryField,
      operator: 'equals',
      value: point.categoryValue,
    });
  }

  if (point.legendField && isUsableValue(point.legendValue) && point.legendField !== point.categoryField) {
    filters.push({
      id: `__tooltip_legend_${point.legendField}`,
      level: 'visual',
      tableId,
      column: point.legendField,
      operator: 'equals',
      value: point.legendValue,
    });
  }

  return filters;
}

/** Chave estável (página + filtros) usada para cache e para detectar hover em um ponto já visto. */
export function tooltipCacheKey(pageId: string, filters: BiFilter[]): string {
  const sorted = [...filters].sort((a, b) => a.column.localeCompare(b.column));
  return `${pageId}|${JSON.stringify(sorted.map(f => [f.tableId, f.column, f.value]))}`;
}
