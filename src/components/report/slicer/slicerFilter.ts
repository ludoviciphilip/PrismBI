import { Page, Table, Visual, VisualDataConfig } from '../../../types/bi';
import { resolveRelativeDate } from '../../../engine/relativeDate';

export type SlicerFilterValue = {
  tableId: string;
  column: string;
  operator: 'equals' | 'in' | 'between';
  value: any;
} | null;

/** Resolves the (tableId, column) a slicer visual targets, honoring the "table.column" prefix form. */
export function resolveSlicerField(visual: Visual, tables: Table[]): { tableId: string; column: string } {
  const raw = visual.dataConfig.xAxisField || '';
  const [prefixTbl, prefixCol] = raw.includes('.') ? raw.split('.', 2) : [visual.dataConfig.tableId, raw];
  const table = tables.find(
    t => t.id === prefixTbl || t.name === prefixTbl || t.id === visual.dataConfig.tableId || t.name === visual.dataConfig.tableId
  );
  return { tableId: table?.id || prefixTbl, column: prefixCol };
}

/**
 * Builds the `Filter` a slicer's current selection represents, or `null`
 * when the slicer has no active selection (clears any previous filter).
 * 'relative-date' is resolved live against `today` so a reopened project
 * recalculates the window instead of replaying a stale saved range.
 */
export function buildSlicerFilterValue(
  slicerConfig: NonNullable<VisualDataConfig['slicerConfig']>,
  today: Date = new Date()
): { operator: 'equals' | 'in' | 'between'; value: any } | null {
  if (slicerConfig.mode === 'relative-date') {
    if (!slicerConfig.relative) return null;
    const { start, end } = resolveRelativeDate(slicerConfig.relative, today);
    return { operator: 'between', value: [start, end] };
  }
  if (slicerConfig.mode === 'between') {
    const { start, end } = slicerConfig.between || {};
    if (start === undefined && end === undefined) return null;
    return { operator: 'between', value: [start ?? end, end ?? start] };
  }
  const selected = slicerConfig.selectedValues;
  if (!selected || selected.length === 0) return null;
  return selected.length === 1 ? { operator: 'equals', value: selected[0] } : { operator: 'in', value: selected };
}

export function slicerFilterId(visualId: string): string {
  return `slicer_${visualId}`;
}

function formatBr(iso: string): string {
  const [y, m, d] = iso.split('-');
  return `${d}/${m}/${y}`;
}

const ANCHOR_LABEL: Record<string, string> = { last: 'últimos', this: 'este(a)', next: 'próximos' };
const UNIT_LABEL: Record<string, string> = { days: 'dias', weeks: 'semanas', months: 'meses', quarters: 'trimestres', years: 'anos' };

/**
 * Human-readable label for a Filter produced by a 'between' or 'relative-date'
 * slicer, e.g. "Data: últimos 3 meses (01/09/2026–30/11/2026)" — used by
 * FiltersPane so these filters read like Power BI's filter pane instead of
 * showing a raw [start, end] array.
 */
export function describeSlicerDateFilter(
  pages: Page[],
  visualId: string | undefined
): string | null {
  if (!visualId) return null;
  let visual: Visual | undefined;
  for (const page of pages) {
    visual = page.visuals.find(v => v.id === visualId);
    if (visual) break;
  }
  const slicerConfig = visual?.dataConfig.slicerConfig;
  if (!slicerConfig) return null;

  if (slicerConfig.mode === 'relative-date' && slicerConfig.relative) {
    const { start, end } = resolveRelativeDate(slicerConfig.relative, new Date());
    const { anchor, count, unit } = slicerConfig.relative;
    const countPart = anchor === 'this' ? '' : `${count} `;
    return `Data: ${ANCHOR_LABEL[anchor]} ${countPart}${UNIT_LABEL[unit]} (${formatBr(start)}–${formatBr(end)})`;
  }
  if (slicerConfig.mode === 'between' && slicerConfig.between) {
    const { start, end } = slicerConfig.between;
    if (start === undefined && end === undefined) return null;
    const isDate = typeof start === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(String(start));
    if (isDate) return `Data: ${formatBr(String(start))}–${formatBr(String(end))}`;
    return `${visual!.dataConfig.xAxisField || ''}: ${start}–${end}`;
  }
  return null;
}

/** Every slicer visual across all pages that shares `syncGroup`, paired with its page id. */
export function findSyncSiblings(
  pages: Page[],
  syncGroup: string,
  excludeVisualId: string
): Array<{ pageId: string; visual: Visual }> {
  const result: Array<{ pageId: string; visual: Visual }> = [];
  for (const page of pages) {
    for (const visual of page.visuals) {
      if (visual.id === excludeVisualId) continue;
      if (visual.type !== 'slicer') continue;
      if (visual.dataConfig.slicerConfig?.syncGroup === syncGroup) {
        result.push({ pageId: page.id, visual });
      }
    }
  }
  return result;
}

/** Whether `pageId` is included in a sync scope (empty/absent syncPages = every page). */
export function pageInSyncScope(syncPages: string[] | undefined, pageId: string): boolean {
  return !syncPages || syncPages.length === 0 || syncPages.includes(pageId);
}
