/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import { captureBookmark, applyBookmark } from '../capture';
import { Filter, Page, Visual } from '../../../../types/bi';

const visualA: Visual = {
  id: 'vA',
  type: 'column',
  title: 'A',
  x: 0, y: 0, w: 100, h: 100,
  dataConfig: { tableId: 't1', sortField: 'Mes', sortOrder: 'asc', currentDrillLevel: 0 },
  formatConfig: { showTitle: true, hidden: false },
};

const visualSlicer: Visual = {
  id: 'vSlicer',
  type: 'slicer',
  title: 'Segmentador',
  x: 0, y: 0, w: 100, h: 100,
  dataConfig: { tableId: 't1', xAxisField: 'Categoria', slicerConfig: { mode: 'list', selectedValues: ['Eletrônicos'] } },
  formatConfig: { showTitle: true },
};

const reportFilter: Filter = { id: 'f1', level: 'report', tableId: 't1', column: 'Ano', operator: 'equals', value: 2024 };
const visualFilterA: Filter = { id: 'f2', level: 'visual', visualId: 'vA', tableId: 't1', column: 'Mes', operator: 'equals', value: 'Jan' };

function page(visuals: Visual[]): Page {
  return { id: 'p1', name: 'Página 1', order: 0, visuals };
}

describe('captureBookmark / applyBookmark (puras)', () => {
  it('captura dados+exibição+página (default) e aplica tudo', () => {
    const captured = captureBookmark({
      activePageId: 'p1',
      filters: [reportFilter, visualFilterA],
      visuals: [visualA, visualSlicer],
    });
    expect(captured.capture).toEqual({ data: true, display: true, currentPage: true });
    expect(captured.filters).toHaveLength(2);
    expect(captured.visualState?.vA).toEqual({ hidden: false, drillLevel: 0, sortField: 'Mes', sortOrder: 'asc' });
    expect(captured.slicerSelections?.vSlicer).toEqual({ selectedValues: ['Eletrônicos'], between: undefined, relative: undefined });

    const bookmark = { id: 'b1', name: 'Teste', ...captured } as any;
    const patch = applyBookmark(bookmark, { pages: [page([visualA, visualSlicer])], filters: [], activePageId: 'other' });
    expect(patch.activePageId).toBe('p1');
    expect(patch.filters).toHaveLength(2);
    expect(patch.pages?.[0].visuals[0].dataConfig.sortField).toBe('Mes');
  });

  it('capture.data=false não captura nem aplica filtros/seleção de segmentador', () => {
    const captured = captureBookmark({
      activePageId: 'p1',
      filters: [reportFilter],
      visuals: [visualSlicer],
      capture: { data: false, display: true, currentPage: true },
    });
    expect(captured.filters).toHaveLength(0);
    expect(captured.slicerSelections).toBeUndefined();

    const bookmark = { id: 'b2', name: 'Teste', ...captured } as any;
    const ctxFilters = [reportFilter];
    const patch = applyBookmark(bookmark, { pages: [page([visualSlicer])], filters: ctxFilters, activePageId: 'p1' });
    expect(patch.filters).toBeUndefined();
  });

  it('capture.display=false não captura nem aplica visibilidade/drill/ordenação', () => {
    const captured = captureBookmark({
      activePageId: 'p1',
      filters: [],
      visuals: [visualA],
      capture: { data: true, display: false, currentPage: true },
    });
    expect(captured.visualState).toBeUndefined();

    const bookmark = { id: 'b3', name: 'Teste', ...captured } as any;
    const hiddenVisualA = { ...visualA, formatConfig: { ...visualA.formatConfig, hidden: true } };
    const patch = applyBookmark(bookmark, { pages: [page([hiddenVisualA])], filters: [], activePageId: 'p1' });
    // Sem patch.pages (nada de exibição/segmentador capturado para aplicar)
    expect(patch.pages).toBeUndefined();
  });

  it('capture.currentPage=false não muda a página ativa', () => {
    const captured = captureBookmark({
      activePageId: 'p1',
      filters: [],
      visuals: [],
      capture: { data: true, display: true, currentPage: false },
    });
    const bookmark = { id: 'b4', name: 'Teste', ...captured } as any;
    const patch = applyBookmark(bookmark, { pages: [page([])], filters: [], activePageId: 'p2' });
    expect(patch.activePageId).toBeUndefined();
  });

  it('scope com visuais selecionados: só os visuais do escopo são capturados/alterados', () => {
    const captured = captureBookmark({
      activePageId: 'p1',
      filters: [visualFilterA, { ...visualFilterA, id: 'f3', visualId: 'vSlicer' }],
      visuals: [visualA, visualSlicer],
      scope: ['vA'],
    });
    expect(captured.scope).toEqual(['vA']);
    expect(captured.filters).toHaveLength(1);
    expect(captured.filters[0].visualId).toBe('vA');
    expect(captured.visualState).toHaveProperty('vA');
    expect(captured.visualState).not.toHaveProperty('vSlicer');

    const bookmark = { id: 'b5', name: 'Teste', ...captured } as any;
    const otherVisualFilter: Filter = { id: 'fOther', level: 'visual', visualId: 'vSlicer', tableId: 't1', column: 'x', operator: 'equals', value: 1 };
    const patch = applyBookmark(bookmark, {
      pages: [page([visualA, { ...visualSlicer, formatConfig: { ...visualSlicer.formatConfig, hidden: true } }])],
      filters: [otherVisualFilter],
      activePageId: 'p1',
    });
    // O filtro do visual fora do escopo (vSlicer) é preservado.
    expect(patch.filters).toContainEqual(otherVisualFilter);
    // O visual fora do escopo não é tocado pelo patch de páginas.
    const slicerAfter = patch.pages?.[0].visuals.find(v => v.id === 'vSlicer');
    expect(slicerAfter?.formatConfig.hidden).toBe(true);
  });

  it('seams E↔G: captura e restaura drillPath/drillMode (gráfico) e a expansão da matriz', () => {
    const chartVisual: Visual = {
      id: 'vChart',
      type: 'column',
      title: 'Vendas',
      x: 0, y: 0, w: 100, h: 100,
      dataConfig: { tableId: 't1', xAxisField: 'Data', currentDrillLevel: 1, drillMode: 'drill', drillPath: [2024] },
      formatConfig: { showTitle: true },
    };
    const matrixVisualEntry: Visual = {
      id: 'vMatrix',
      type: 'matrix',
      title: 'Matriz',
      x: 0, y: 0, w: 100, h: 100,
      dataConfig: {
        tableId: 't1',
        matrixRowFields: ['Regiao', 'Cidade', 'Loja'],
        // Estado de expansão local (ver `MatrixExpansion` em engine/drill.ts), opcional e novo.
        matrixExpansion: { depth: 0, expanded: ['["Sul"]'], collapsed: [] },
      },
      formatConfig: { showTitle: true },
    };

    const captured = captureBookmark({
      activePageId: 'p1',
      filters: [],
      visuals: [chartVisual, matrixVisualEntry],
    });
    expect(captured.visualState?.vChart).toMatchObject({ drillLevel: 1, drillMode: 'drill', drillPath: [2024] });
    expect(captured.visualState?.vMatrix).toMatchObject({ matrixExpansion: { depth: 0, expanded: ['["Sul"]'], collapsed: [] } });

    const bookmark = { id: 'b6', name: 'Teste', ...captured } as any;
    const freshChart = { ...chartVisual, dataConfig: { ...chartVisual.dataConfig, currentDrillLevel: 0, drillMode: undefined, drillPath: undefined } };
    const freshMatrix = { ...matrixVisualEntry, dataConfig: { ...matrixVisualEntry.dataConfig, matrixExpansion: undefined } };
    const patch = applyBookmark(bookmark, { pages: [page([freshChart, freshMatrix])], filters: [], activePageId: 'p1' });
    const restoredChart = patch.pages?.[0].visuals.find(v => v.id === 'vChart');
    expect(restoredChart?.dataConfig.currentDrillLevel).toBe(1);
    expect(restoredChart?.dataConfig.drillMode).toBe('drill');
    expect(restoredChart?.dataConfig.drillPath).toEqual([2024]);
    const restoredMatrix = patch.pages?.[0].visuals.find(v => v.id === 'vMatrix');
    expect(restoredMatrix?.dataConfig.matrixExpansion).toEqual({ depth: 0, expanded: ['["Sul"]'], collapsed: [] });
  });

  it('seams E↔G: indicador antigo (sem drillPath/drillMode/matrixExpansion) continua válido', () => {
    const legacyVisualState = { hidden: false, drillLevel: 2, sortField: 'Mes', sortOrder: 'asc' as const };
    const bookmark = {
      id: 'b7',
      name: 'Antigo',
      activePageId: 'p1',
      filters: [],
      visualState: { vA: legacyVisualState },
      capture: { data: true, display: true, currentPage: true },
    } as any;
    const patch = applyBookmark(bookmark, { pages: [page([visualA])], filters: [], activePageId: 'p1' });
    const restored = patch.pages?.[0].visuals.find(v => v.id === 'vA');
    expect(restored?.dataConfig.currentDrillLevel).toBe(2);
    expect(restored?.dataConfig.drillMode).toBe(visualA.dataConfig.drillMode);
    expect(restored?.dataConfig.drillPath).toBe(visualA.dataConfig.drillPath);
  });
});
