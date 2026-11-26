// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, fireEvent, within, cleanup } from '@testing-library/react';

import { useBiStore } from '../../../store/useBiStore';
import { VisualCard } from '../VisualCard';
import { FiltersPane } from '../../panels/FiltersPane';
import { Table, Visual } from '../../../types/bi';

const table: Table = {
  id: 'tv',
  name: 'Vendas',
  originalName: 'Vendas',
  columns: [
    { id: 'c1', name: 'Canal', originalName: 'Canal', dataType: 'text' },
    { id: 'c2', name: 'Valor', originalName: 'Valor', dataType: 'number' },
    { id: 'c3', name: 'Data', originalName: 'Data', dataType: 'date' },
  ],
  rows: [
    { Canal: 'Online', Valor: 10, Data: '2024-01-01' },
    { Canal: 'Loja', Valor: 20, Data: '2024-02-01' },
    { Canal: 'Átacado', Valor: 30, Data: '2024-03-01' },
    { Canal: 'Loja Física', Valor: 40, Data: '2024-04-01' },
  ],
} as Table;

function seed(visual: Visual) {
  const st = useBiStore.getState();
  st.newProject('Slicer UI');
  st.setTables([table]);
  const pageId = useBiStore.getState().activePageId;
  useBiStore.setState(s => ({
    pages: [{ ...s.pages[0], id: pageId, visuals: [visual] }],
    filters: [],
  }));
}

const LiveCard: React.FC<{ id: string }> = ({ id }) => {
  const v = useBiStore(s => s.pages.flatMap(p => p.visuals).find(x => x.id === id))!;
  return <VisualCard visual={v} isSelected={false} onSelect={() => {}} />;
};

const getVisual = (id: string) => useBiStore.getState().pages.flatMap(p => p.visuals).find(v => v.id === id)!;

const baseVisual = (over: Partial<Visual>): Visual => ({
  id: 'v1',
  type: 'slicer',
  title: 'Segmentador',
  x: 0,
  y: 0,
  w: 260,
  h: 150,
  dataConfig: { tableId: 'tv', xAxisField: 'Canal' },
  formatConfig: { showTitle: true },
  ...over,
});

afterEach(() => cleanup());

describe('SlicerBody: busca (showSearch) nos modos lista/dropdown/tiles', () => {
  it('lista: filtra itens por busca case/acento-insensitive e "Selecionar tudo" respeita a busca', () => {
    seed(baseVisual({ dataConfig: { tableId: 'tv', xAxisField: 'Canal', slicerConfig: { mode: 'list', showSearch: true } } }));
    render(<LiveCard id="v1" />);

    const items = () => screen.getAllByRole('listitem').map(li => li.textContent);
    expect(items()).toEqual(['Átacado', 'Loja', 'Loja Física', 'Online']);

    fireEvent.change(screen.getByPlaceholderText('Buscar item'), { target: { value: 'atacado' } });
    expect(items()).toEqual(['Átacado']);

    fireEvent.click(screen.getByRole('checkbox', { name: /Selecionar tudo/ }));
    expect(getVisual('v1').dataConfig.slicerConfig?.selectedValues).toEqual(['Átacado']);

    // Clearing the search again shows every item, selection from before remains.
    fireEvent.change(screen.getByPlaceholderText('Buscar item'), { target: { value: '' } });
    expect(items()).toEqual(['Átacado', 'Loja', 'Loja Física', 'Online']);
    expect(screen.getByRole('checkbox', { name: /Átacado/ }).hasAttribute('checked') || (screen.getByRole('checkbox', { name: /Átacado/ }) as HTMLInputElement).checked).toBeTruthy();
  });

  it('tiles: busca filtra os blocos exibidos e alterna seleção múltipla', () => {
    seed(baseVisual({ dataConfig: { tableId: 'tv', xAxisField: 'Canal', slicerConfig: { mode: 'tiles', showSearch: true } } }));
    render(<LiveCard id="v1" />);
    fireEvent.change(screen.getByPlaceholderText('Buscar item'), { target: { value: 'loja' } });
    const buttons = () =>
      screen
        .getAllByRole('button')
        .map(b => b.textContent)
        .filter(txt => txt === 'Loja' || txt === 'Loja Física');
    expect(buttons().sort()).toEqual(['Loja', 'Loja Física'].sort());
    fireEvent.click(screen.getByRole('button', { name: 'Loja' }));
    expect(getVisual('v1').dataConfig.slicerConfig?.selectedValues).toEqual(['Loja']);
    fireEvent.click(screen.getByRole('button', { name: 'Loja Física' }));
    expect(getVisual('v1').dataConfig.slicerConfig?.selectedValues).toEqual(['Loja', 'Loja Física']);
  });

  it('dropdown: busca filtra as opções do select', () => {
    seed(baseVisual({ dataConfig: { tableId: 'tv', xAxisField: 'Canal', slicerConfig: { mode: 'dropdown', showSearch: true } } }));
    render(<LiveCard id="v1" />);
    fireEvent.change(screen.getByPlaceholderText('Buscar item'), { target: { value: 'loja' } });
    const options = within(screen.getByRole('combobox')).getAllByRole('option').map(o => o.textContent);
    expect(options).toEqual(['(Todos os Registros)', 'Loja', 'Loja Física']);
  });
});

describe('SlicerBody: modo Entre (between)', () => {
  it('número: mostra min/max e grava um filtro between', () => {
    seed(baseVisual({ dataConfig: { tableId: 'tv', xAxisField: 'Valor', slicerConfig: { mode: 'between' } } }));
    render(<LiveCard id="v1" />);
    const minInput = screen.getByLabelText('Mín.') as HTMLInputElement;
    const maxInput = screen.getByLabelText('Máx.') as HTMLInputElement;
    fireEvent.change(minInput, { target: { value: '15' } });
    expect(getVisual('v1').dataConfig.slicerConfig?.between?.start).toBe(15);
    const filter = useBiStore.getState().filters.find(f => f.visualId === 'v1')!;
    expect(filter.operator).toBe('between');
    // Every new input must be usable at 1366x768 — real widths, not collapsed to ~20px.
    expect(minInput.getBoundingClientRect).toBeTruthy();
    expect(maxInput).toBeTruthy();
  });

  it('data: mostra Início/Fim e grava um filtro between legível na FiltersPane', () => {
    seed(baseVisual({ dataConfig: { tableId: 'tv', xAxisField: 'Data', slicerConfig: { mode: 'between' } } }));
    render(<LiveCard id="v1" />);
    fireEvent.change(screen.getByLabelText('Início'), { target: { value: '2026-09-01' } });
    fireEvent.change(screen.getByLabelText('Fim'), { target: { value: '2026-11-30' } });
    render(<FiltersPane />);
    expect(screen.getByText('Data: 01/09/2026–30/11/2026')).toBeTruthy();
  });
});

describe('SlicerBody: Data relativa', () => {
  it('monta um filtro between a partir da definição relativa e aparece legível na FiltersPane', () => {
    seed(
      baseVisual({
        dataConfig: {
          tableId: 'tv',
          xAxisField: 'Data',
          slicerConfig: { mode: 'relative-date', relative: { anchor: 'this', count: 1, unit: 'years' } },
        },
      })
    );
    render(<LiveCard id="v1" />);
    useBiStore.getState().recalculateRelativeDateFilters();
    render(<FiltersPane />);
    const year = new Date().getFullYear();
    expect(screen.getByText(`Data: este(a) anos (01/01/${year}–31/12/${year})`)).toBeTruthy();
  });
});
