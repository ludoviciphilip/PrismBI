import { describe, it, expect } from 'vitest';
import { useBiStore } from '../useBiStore';
import { Table, Visual } from '../../types/bi';

const table: Table = {
  id: 't1',
  name: 'Vendas',
  originalName: 'Vendas',
  columns: [
    { id: 'c1', name: 'Canal', originalName: 'Canal', dataType: 'text' },
    { id: 'c2', name: 'Data', originalName: 'Data', dataType: 'date' },
  ],
  rows: [],
} as Table;

function slicerVisual(id: string, over: Partial<Visual> = {}): Visual {
  return {
    id,
    type: 'slicer',
    title: 'Segmentador',
    x: 0,
    y: 0,
    w: 200,
    h: 100,
    dataConfig: { tableId: 't1', xAxisField: 'Canal', slicerConfig: { mode: 'list' } },
    formatConfig: { showTitle: true },
    ...over,
  };
}

function seedTwoPages(v1: Visual, v2: Visual, v3?: Visual) {
  const st = useBiStore.getState();
  st.newProject('Sync teste');
  st.setTables([table]);
  const page1Id = useBiStore.getState().activePageId;
  const page2Id = 'page_2';
  const page1 = useBiStore.getState().pages[0];
  useBiStore.setState(s => ({
    pages: [
      { ...page1, id: page1Id, visuals: [v1] },
      { ...page1, id: page2Id, name: 'Página 2', visuals: v3 ? [v2, v3] : [v2] },
    ],
    filters: [],
  }));
  return { page1Id, page2Id };
}

describe('useBiStore.setSlicerSelection — sync entre páginas', () => {
  it('propaga a seleção para outro segmentador do mesmo grupo em outra página', () => {
    const v1 = slicerVisual('v1', { dataConfig: { tableId: 't1', xAxisField: 'Canal', slicerConfig: { mode: 'list', syncGroup: 'g1' } } });
    const v2 = slicerVisual('v2', { dataConfig: { tableId: 't1', xAxisField: 'Canal', slicerConfig: { mode: 'list', syncGroup: 'g1' } } });
    const { page2Id } = seedTwoPages(v1, v2);

    useBiStore.getState().setSlicerSelection('v1', { selectedValues: ['Online'] });

    const getVisual = (id: string) => useBiStore.getState().pages.flatMap(p => p.visuals).find(v => v.id === id)!;
    expect(getVisual('v1').dataConfig.slicerConfig?.selectedValues).toEqual(['Online']);
    expect(getVisual('v2').dataConfig.slicerConfig?.selectedValues).toEqual(['Online']);

    const filters = useBiStore.getState().filters;
    expect(filters.find(f => f.visualId === 'v1')).toMatchObject({ operator: 'equals', value: 'Online', tableId: 't1', column: 'Canal' });
    expect(filters.find(f => f.visualId === 'v2')).toMatchObject({ operator: 'equals', value: 'Online', tableId: 't1', column: 'Canal' });
    expect(page2Id).toBe('page_2');
  });

  it('não propaga para um segmentador do mesmo grupo numa página fora de syncPages', () => {
    const v1 = slicerVisual('v1', {
      dataConfig: { tableId: 't1', xAxisField: 'Canal', slicerConfig: { mode: 'list', syncGroup: 'g1', syncPages: ['page_executivo'] } },
    });
    const v2 = slicerVisual('v2', { dataConfig: { tableId: 't1', xAxisField: 'Canal', slicerConfig: { mode: 'list', syncGroup: 'g1' } } });
    seedTwoPages(v1, v2);

    useBiStore.getState().setSlicerSelection('v1', { selectedValues: ['Loja'] });

    const getVisual = (id: string) => useBiStore.getState().pages.flatMap(p => p.visuals).find(v => v.id === id)!;
    expect(getVisual('v1').dataConfig.slicerConfig?.selectedValues).toEqual(['Loja']);
    // v2 is on page_2, which is not in v1's syncPages -> untouched.
    expect(getVisual('v2').dataConfig.slicerConfig?.selectedValues).toBeUndefined();
    expect(useBiStore.getState().filters.find(f => f.visualId === 'v2')).toBeUndefined();
  });

  it('não propaga para segmentadores de outro syncGroup', () => {
    const v1 = slicerVisual('v1', { dataConfig: { tableId: 't1', xAxisField: 'Canal', slicerConfig: { mode: 'list', syncGroup: 'g1' } } });
    const v2 = slicerVisual('v2', { dataConfig: { tableId: 't1', xAxisField: 'Canal', slicerConfig: { mode: 'list', syncGroup: 'g2' } } });
    seedTwoPages(v1, v2);

    useBiStore.getState().setSlicerSelection('v1', { selectedValues: ['Loja'] });
    const getVisual = (id: string) => useBiStore.getState().pages.flatMap(p => p.visuals).find(v => v.id === id)!;
    expect(getVisual('v2').dataConfig.slicerConfig?.selectedValues).toBeUndefined();
  });

  it('clearing the selection (empty array) removes the filter again', () => {
    const v1 = slicerVisual('v1');
    seedTwoPages(v1, slicerVisual('v2'));
    useBiStore.getState().setSlicerSelection('v1', { selectedValues: ['Loja'] });
    expect(useBiStore.getState().filters.find(f => f.visualId === 'v1')).toBeTruthy();
    useBiStore.getState().setSlicerSelection('v1', { selectedValues: [] });
    expect(useBiStore.getState().filters.find(f => f.visualId === 'v1')).toBeUndefined();
  });
});

describe('useBiStore.recalculateRelativeDateFilters', () => {
  it('resolves a relative slicer definition to a concrete between filter', () => {
    const v1 = slicerVisual('v1', {
      dataConfig: {
        tableId: 't1',
        xAxisField: 'Data',
        slicerConfig: { mode: 'relative-date', relative: { anchor: 'this', count: 1, unit: 'years' } },
      },
    });
    const st = useBiStore.getState();
    st.newProject('Relativo');
    st.setTables([table]);
    const pageId = useBiStore.getState().activePageId;
    useBiStore.setState(s => ({ pages: [{ ...s.pages[0], id: pageId, visuals: [v1] }], filters: [] }));

    useBiStore.getState().recalculateRelativeDateFilters();
    const filter = useBiStore.getState().filters.find(f => f.visualId === 'v1')!;
    expect(filter.operator).toBe('between');
    const year = new Date().getFullYear();
    expect(filter.value).toEqual([`${year}-01-01`, `${year}-12-31`]);
  });
});
