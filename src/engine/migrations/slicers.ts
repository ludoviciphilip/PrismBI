import { Page, RelativeDateFilter, Visual } from '../../types/bi';

/**
 * Maps the legacy `slicerConfig.dateRange.relativePreset` enum to the new
 * `slicerConfig.relative` definition (resolved live by src/engine/relativeDate.ts).
 * `'all'` has no relative equivalent and is dropped (the slicer reverts to
 * an unfiltered 'between' with no start/end, same as before the migration).
 */
const PRESET_MAP: Record<string, RelativeDateFilter | null> = {
  last7days: { anchor: 'last', count: 7, unit: 'days', includeToday: true },
  last30days: { anchor: 'last', count: 30, unit: 'days', includeToday: true },
  thisMonth: { anchor: 'this', count: 1, unit: 'months' },
  thisYear: { anchor: 'this', count: 1, unit: 'years' },
  all: null,
};

function migrateVisual(visual: Visual): Visual {
  const slicerConfig = visual.dataConfig?.slicerConfig;
  const preset = slicerConfig?.dateRange?.relativePreset;
  if (!slicerConfig || !preset) return visual;

  const relative = PRESET_MAP[preset];
  const { dateRange, ...restSlicerConfig } = slicerConfig;
  // Legacy explicit start/end (when the preset was 'all' and the user picked
  // a manual range) are preserved as a 'between' range.
  const legacyBetween =
    !relative && (dateRange?.start || dateRange?.end) ? { start: dateRange.start, end: dateRange.end } : undefined;

  return {
    ...visual,
    dataConfig: {
      ...visual.dataConfig,
      slicerConfig: {
        ...restSlicerConfig,
        mode: relative ? 'relative-date' : restSlicerConfig.mode === 'relative-date' ? 'between' : restSlicerConfig.mode,
        ...(relative ? { relative } : {}),
        ...(legacyBetween ? { between: legacyBetween } : {}),
      },
    },
  };
}

/** Migrates every slicer visual's legacy `dateRange.relativePreset` across all pages. */
export function migrateSlicerConfigs(pages: Page[]): Page[] {
  if (!Array.isArray(pages)) return pages;
  return pages.map(page => ({
    ...page,
    visuals: Array.isArray(page.visuals) ? page.visuals.map(migrateVisual) : page.visuals,
  }));
}
