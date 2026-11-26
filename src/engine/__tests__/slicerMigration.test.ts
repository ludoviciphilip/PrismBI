import { describe, it, expect } from 'vitest';
import { migrateSlicerConfigs } from '../migrations/slicers';
import { Page } from '../../types/bi';

function page(visuals: Page['visuals']): Page {
  return { id: 'p1', name: 'Pagina', visuals } as Page;
}

describe('migrateSlicerConfigs (legacy dateRange.relativePreset -> relative)', () => {
  it('migrates last7days to a relative definition and switches the mode', () => {
    const pages = [
      page([
        {
          id: 'v1',
          type: 'slicer',
          title: 'Data',
          x: 0,
          y: 0,
          w: 200,
          h: 100,
          dataConfig: {
            tableId: 't1',
            xAxisField: 'Data',
            slicerConfig: { mode: 'relative-date', dateRange: { relativePreset: 'last7days' } },
          },
          formatConfig: { showTitle: true },
        },
      ]),
    ] as Page[];

    const migrated = migrateSlicerConfigs(pages);
    const slicer = migrated[0].visuals[0].dataConfig.slicerConfig!;
    expect(slicer.mode).toBe('relative-date');
    expect(slicer.relative).toEqual({ anchor: 'last', count: 7, unit: 'days', includeToday: true });
    expect(slicer.dateRange).toBeUndefined();
  });

  it('migrates thisMonth/thisYear presets', () => {
    const pages = [
      page([
        {
          id: 'v1', type: 'slicer', title: 'Data', x: 0, y: 0, w: 200, h: 100,
          dataConfig: { tableId: 't1', slicerConfig: { mode: 'relative-date', dateRange: { relativePreset: 'thisMonth' } } },
          formatConfig: { showTitle: true },
        },
        {
          id: 'v2', type: 'slicer', title: 'Data', x: 0, y: 0, w: 200, h: 100,
          dataConfig: { tableId: 't1', slicerConfig: { mode: 'relative-date', dateRange: { relativePreset: 'thisYear' } } },
          formatConfig: { showTitle: true },
        },
      ]),
    ] as Page[];
    const migrated = migrateSlicerConfigs(pages);
    expect(migrated[0].visuals[0].dataConfig.slicerConfig!.relative).toEqual({ anchor: 'this', count: 1, unit: 'months' });
    expect(migrated[0].visuals[1].dataConfig.slicerConfig!.relative).toEqual({ anchor: 'this', count: 1, unit: 'years' });
  });

  it('"all" preset with no explicit range drops to an empty between, dropping mode relative-date', () => {
    const pages = [
      page([
        {
          id: 'v1', type: 'slicer', title: 'Data', x: 0, y: 0, w: 200, h: 100,
          dataConfig: { tableId: 't1', slicerConfig: { mode: 'relative-date', dateRange: { relativePreset: 'all' } } },
          formatConfig: { showTitle: true },
        },
      ]),
    ] as Page[];
    const migrated = migrateSlicerConfigs(pages);
    const slicer = migrated[0].visuals[0].dataConfig.slicerConfig!;
    expect(slicer.mode).toBe('between');
    expect(slicer.relative).toBeUndefined();
  });

  it('"all" preset with an explicit manual start/end preserves it as between', () => {
    const pages = [
      page([
        {
          id: 'v1', type: 'slicer', title: 'Data', x: 0, y: 0, w: 200, h: 100,
          dataConfig: {
            tableId: 't1',
            slicerConfig: { mode: 'relative-date', dateRange: { relativePreset: 'all', start: '2024-01-01', end: '2024-01-31' } },
          },
          formatConfig: { showTitle: true },
        },
      ]),
    ] as Page[];
    const migrated = migrateSlicerConfigs(pages);
    const slicer = migrated[0].visuals[0].dataConfig.slicerConfig!;
    expect(slicer.mode).toBe('between');
    expect(slicer.between).toEqual({ start: '2024-01-01', end: '2024-01-31' });
  });

  it('leaves slicers without a legacy preset untouched', () => {
    const pages = [
      page([
        {
          id: 'v1', type: 'slicer', title: 'Canal', x: 0, y: 0, w: 200, h: 100,
          dataConfig: { tableId: 't1', slicerConfig: { mode: 'dropdown' } },
          formatConfig: { showTitle: true },
        },
      ]),
    ] as Page[];
    const migrated = migrateSlicerConfigs(pages);
    expect(migrated[0].visuals[0].dataConfig.slicerConfig).toEqual({ mode: 'dropdown' });
  });
});
