// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, within } from '@testing-library/react';

import { useBiStore } from '../../../store/useBiStore';
import { VisualizationsPane } from '../VisualizationsPane';
import { Table, Visual } from '../../../types/bi';

const table: Table = {
  id: 'tv',
  name: 'Vendas',
  originalName: 'Vendas',
  columns: [{ id: 'c1', name: 'Canal', originalName: 'Canal', dataType: 'text' }],
  rows: [{ Canal: 'Online' }],
} as Table;

function seedTwoPagesWithSlicer(): string {
  const st = useBiStore.getState();
  st.newProject('Sync Panel Test');
  st.setTables([table]);
  const page1Id = useBiStore.getState().activePageId;
  const visual: Visual = {
    id: 'v1',
    type: 'slicer',
    title: 'Segmentador',
    x: 0,
    y: 0,
    w: 260,
    h: 150,
    dataConfig: { tableId: 'tv', xAxisField: 'Canal', slicerConfig: { mode: 'dropdown' } },
    formatConfig: { showTitle: true },
  };
  const page1 = useBiStore.getState().pages[0];
  useBiStore.setState(s => ({
    pages: [
      { ...page1, id: page1Id, visuals: [visual] },
      { ...page1, id: 'page_2', name: 'Página 2', visuals: [] },
    ],
    selectedVisualId: 'v1',
    activePageId: page1Id,
  }));
  return page1Id;
}

afterEach(() => cleanup());

describe('VisualizationsPane: seção "Sincronizar segmentações"', () => {
  it('define o grupo de sincronização e marca páginas específicas', () => {
    seedTwoPagesWithSlicer();
    render(<VisualizationsPane />);
    fireEvent.click(screen.getByRole('button', { name: 'Formatar visual' }));

    const panel = screen.getByTestId('slicer-sync-panel');
    fireEvent.change(within(panel).getByPlaceholderText('Ex.: Período'), { target: { value: 'g1' } });
    expect(useBiStore.getState().pages[0].visuals[0].dataConfig.slicerConfig?.syncGroup).toBe('g1');

    const refreshedPanel = screen.getByTestId('slicer-sync-panel');
    // Default is "all pages" (empty syncPages) until a specific page is unchecked by selecting it.
    fireEvent.click(within(refreshedPanel).getByRole('checkbox', { name: 'Página 2' }));
    expect(useBiStore.getState().pages[0].visuals[0].dataConfig.slicerConfig?.syncPages).toEqual(['page_2']);
  });

  it('oferece os modos between e relative-date no seletor de estilo', () => {
    seedTwoPagesWithSlicer();
    render(<VisualizationsPane />);
    fireEvent.click(screen.getByRole('button', { name: 'Formatar visual' }));
    const select = screen.getByDisplayValue('Suspenso (Dropdown)') as HTMLSelectElement;
    const options = within(select).getAllByRole('option').map(o => o.textContent);
    expect(options).toContain('Entre');
    expect(options).toContain('Data relativa');
  });
});
