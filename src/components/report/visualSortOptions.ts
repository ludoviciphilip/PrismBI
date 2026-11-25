import { Visual, VisualDataConfig } from '../../types/bi';
import { fieldMatches } from '../../engine/visualSort';
import type { SortableField } from './VisualSortMenu';

/** Visual types that expose "Classificar por" (fields + direction). */
const FIELD_SORTABLE_TYPES = new Set<Visual['type']>([
  'column',
  'stacked-column',
  'bar',
  'stacked-bar',
  'line',
  'area',
  'pie',
  'donut',
  'treemap',
  'funnel',
  'waterfall',
  'combo',
  'table',
  'matrix',
]);

export type SortMenuKind = 'fields' | 'direction-only' | 'none';

export function sortMenuKind(visual: Visual): SortMenuKind {
  if (visual.type === 'slicer') return 'direction-only';
  if (FIELD_SORTABLE_TYPES.has(visual.type)) return 'fields';
  return 'none';
}

const displayName = (f: string) => f.replace(/^Σ\s*/, '');

export function effectiveCategoryField(dc: VisualDataConfig): string | undefined {
  if (dc.drillFields?.length && dc.currentDrillLevel && dc.currentDrillLevel > 0) {
    return dc.drillFields[dc.currentDrillLevel - 1] ?? dc.xAxisField;
  }
  return dc.xAxisField;
}

/**
 * Every field used by the visual that can drive its sort, in Power BI order:
 * category/row field first, then values/measures. For tables, the displayed columns.
 */
export function getSortableFields(
  visual: Visual,
  result: { series: { name: string }[]; rawRows: Record<string, any>[] }
): SortableField[] {
  const out: SortableField[] = [];
  const push = (field: string | undefined) => {
    if (!field) return;
    if (out.some(o => o.field === field || fieldMatches(o.field, field))) return;
    out.push({ field, label: displayName(field) });
  };
  if (visual.type === 'table') {
    const cols = result.rawRows[0] ? Object.keys(result.rawRows[0]).slice(0, 6) : [];
    cols.forEach(push);
    return out;
  }
  push(effectiveCategoryField(visual.dataConfig));
  (visual.dataConfig.yAxisFields || []).forEach(push);
  if (visual.type === 'matrix') result.series.forEach(s => push(s.name));
  return out;
}

export interface EffectiveSort {
  field?: string;
  order: 'asc' | 'desc';
}

/** The sort the user currently sees (explicit config, or the engine default). */
export function getEffectiveSort(visual: Visual, fields: SortableField[]): EffectiveSort {
  const dc = visual.dataConfig;
  if (dc.sortField) {
    const match = fields.find(f => f.field === dc.sortField) ?? fields.find(f => fieldMatches(f.field, dc.sortField));
    return { field: match?.field ?? dc.sortField, order: dc.sortOrder ?? 'asc' };
  }
  if (visual.type === 'table' || visual.type === 'slicer') return { order: dc.sortOrder ?? 'asc' };
  if (dc.limit && dc.limit > 0 && dc.yAxisFields?.[0]) return { field: dc.yAxisFields[0], order: 'desc' };
  return { field: effectiveCategoryField(dc), order: 'asc' };
}

/** dataConfig patch for choosing a sort field from the menu. */
export function patchForField(visual: Visual, field: string): Partial<VisualDataConfig> {
  const isValue = (visual.dataConfig.yAxisFields || []).some(y => fieldMatches(y, field));
  return { sortField: field, sortOrder: visual.dataConfig.sortOrder ?? (isValue ? 'desc' : 'asc') };
}

/** dataConfig patch for choosing a direction from the menu (pins the effective field). */
export function patchForOrder(visual: Visual, effective: EffectiveSort, order: 'asc' | 'desc'): Partial<VisualDataConfig> {
  if (visual.type === 'slicer') return { sortOrder: order };
  return { sortOrder: order, sortField: visual.dataConfig.sortField ?? effective.field };
}

/** Table/matrix header click: first click sorts ascending by that column, later clicks toggle. */
export function patchForHeaderClick(visual: Visual, field: string): Partial<VisualDataConfig> {
  const dc = visual.dataConfig;
  if (dc.sortField && (dc.sortField === field || fieldMatches(dc.sortField, field))) {
    return { sortField: dc.sortField, sortOrder: (dc.sortOrder ?? 'asc') === 'asc' ? 'desc' : 'asc' };
  }
  return { sortField: field, sortOrder: 'asc' };
}
