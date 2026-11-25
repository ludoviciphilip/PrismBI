// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent, within, cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

// ECharts needs a real canvas; the sort UI under test doesn't render charts in table/slicer mode,
// and for chart visuals we only interact with the header menu.
vi.mock('echarts-for-react', () => ({ default: () => <div data-testid="echarts" /> }));

import { useBiStore } from '../../../store/useBiStore';
import { VisualCard } from '../VisualCard';
import { VisualSortMenu } from '../VisualSortMenu';
import { FiltersPane } from '../../panels/FiltersPane';
import { Table, Visual } from '../../../types/bi';

const table: Table = {
  id: 'tv',
  name: 'Vendas',
  originalName: 'Vendas',
  columns: [
    { id: 'c1', name: 'Produto', originalName: 'Produto', dataType: 'text' },
    { id: 'c2', name: 'Valor', originalName: 'Valor', dataType: 'number' },
    { id: 'c3', name: 'Canal', originalName: 'Canal', dataType: 'text' },
  ],
  rows: [
    { Produto: 'Caneta', Valor: 10, Canal: 'Loja' },
    { Produto: 'Abacaxi', Valor: 300, Canal: 'Online' },
    { Produto: 'Borracha', Valor: 25, Canal: 'Online' },
    { Produto: 'Ábaco', Valor: 2, Canal: 'Atacado' },
    { Produto: 'Dado', Valor: 99, Canal: 'Online' },
  ],
} as Table;

function seed(visual: Visual) {
  const st = useBiStore.getState();
  st.newProject('Sort UI');
  st.setTables([table]);
  const pageId = useBiStore.getState().activePageId;
  useBiStore.setState(s => ({
    pages: s.pages.map(p => (p.id === pageId ? { ...p, visuals: [visual] } : p)),
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
  type: 'column',
  title: 'Visual',
  x: 0,
  y: 0,
  w: 400,
  h: 300,
  dataConfig: { tableId: 'tv', xAxisField: 'Produto', yAxisFields: ['Valor'], aggregation: 'SUM' },
  formatConfig: { showTitle: true },
  ...over,
});

afterEach(() => cleanup());

describe('VisualSortMenu (header "..." menu)', () => {
  it('lists every field under "Classificar por" with a check on the current choice', () => {
    const onField = vi.fn();
    const onOrder = vi.fn();
    render(
      <VisualSortMenu
        fields={[
          { field: 'Produto', label: 'Produto' },
          { field: 'Valor', label: 'Valor' },
        ]}
        activeField="Valor"
        activeOrder="desc"
        onSelectField={onField}
        onSelectOrder={onOrder}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: 'Mais opções' }));
    const desc = screen.getByRole('menuitemradio', { name: /Ordem decrescente/ });
    const asc = screen.getByRole('menuitemradio', { name: /Ordem crescente/ });
    expect(desc.getAttribute('aria-checked')).toBe('true');
    expect(asc.getAttribute('aria-checked')).toBe('false');

    fireEvent.click(screen.getByRole('menuitem', { name: /Classificar por/ }));
    const sub = screen.getByRole('menu', { name: 'Classificar por' });
    const items = within(sub).getAllByRole('menuitemradio');
    expect(items.map(i => i.textContent)).toEqual(['Produto', 'Valor']);
    expect(items[1].getAttribute('aria-checked')).toBe('true');
    expect(items[0].getAttribute('aria-checked')).toBe('false');

    fireEvent.click(items[0]);
    expect(onField).toHaveBeenCalledWith('Produto');
  });

  it('shows only the direction items when no fields are given (slicer)', () => {
    render(
      <VisualSortMenu fields={[]} activeOrder="asc" onSelectField={() => {}} onSelectOrder={() => {}} />
    );
    fireEvent.click(screen.getByRole('button', { name: 'Mais opções' }));
    expect(screen.queryByRole('menuitem', { name: /Classificar por/ })).toBeNull();
    expect(screen.getAllByRole('menuitemradio')).toHaveLength(2);
  });
});

describe('VisualCard sort integration', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('column chart: choosing "Classificar por > Valor" then "Ordem crescente" updates sortField/sortOrder', () => {
    seed(baseVisual({}));
    render(<LiveCard id="v1" />);
    fireEvent.click(screen.getByRole('button', { name: 'Mais opções' }));
    fireEvent.click(screen.getByRole('menuitem', { name: /Classificar por/ }));
    const sub = screen.getByRole('menu', { name: 'Classificar por' });
    // Default sort is by axis ascending → "Produto" checked.
    expect(within(sub).getByRole('menuitemradio', { name: 'Produto' }).getAttribute('aria-checked')).toBe('true');
    fireEvent.click(within(sub).getByRole('menuitemradio', { name: 'Valor' }));
    expect(getVisual('v1').dataConfig.sortField).toBe('Valor');
    expect(getVisual('v1').dataConfig.sortOrder).toBe('desc');

    fireEvent.click(screen.getByRole('button', { name: 'Mais opções' }));
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Ordem crescente/ }));
    expect(getVisual('v1').dataConfig.sortOrder).toBe('asc');
    expect(getVisual('v1').dataConfig.sortField).toBe('Valor');
  });

  it('table: clicking a column header sorts asc, then toggles desc, with an arrow indicator', () => {
    seed(baseVisual({ type: 'table', dataConfig: { tableId: 'tv', xAxisField: 'Produto', yAxisFields: ['Valor'] } }));
    render(<LiveCard id="v1" />);
    const firstColValues = () =>
      screen
        .getAllByRole('row')
        .slice(1)
        .map(r => Number((within(r).getAllByRole('cell')[1].textContent || '').replace(/[^\d,]/g, '').replace(',', '.')));

    const valorHeader = () => screen.getByRole('columnheader', { name: /Valor/ });
    expect(valorHeader().getAttribute('aria-sort')).toBe('none');

    fireEvent.click(valorHeader());
    expect(getVisual('v1').dataConfig).toMatchObject({ sortField: 'Valor', sortOrder: 'asc' });
    expect(valorHeader().getAttribute('aria-sort')).toBe('ascending');
    expect(within(valorHeader()).getByLabelText('crescente')).toBeTruthy();
    expect(firstColValues()).toEqual([2, 10, 25, 99, 300]);

    fireEvent.click(valorHeader());
    expect(getVisual('v1').dataConfig.sortOrder).toBe('desc');
    expect(valorHeader().getAttribute('aria-sort')).toBe('descending');
    expect(within(valorHeader()).getByLabelText('decrescente')).toBeTruthy();
    expect(firstColValues()).toEqual([300, 99, 25, 10, 2]);

    // Switching to another column starts ascending (pt-BR collation).
    const prodHeader = screen.getByRole('columnheader', { name: /Produto/ });
    fireEvent.click(prodHeader);
    expect(getVisual('v1').dataConfig).toMatchObject({ sortField: 'Produto', sortOrder: 'asc' });
    expect(valorHeader().getAttribute('aria-sort')).toBe('none');
    const names = screen
      .getAllByRole('row')
      .slice(1)
      .map(r => within(r).getAllByRole('cell')[0].textContent);
    expect(names).toEqual([...names].sort((a, b) => a!.localeCompare(b!, 'pt-BR')));
    expect(names[names.length - 1]).toBe('Dado');
  });

  it('matrix: clicking a value header sorts rows by that measure', () => {
    seed(baseVisual({ type: 'matrix' }));
    render(<LiveCard id="v1" />);
    fireEvent.click(screen.getByRole('columnheader', { name: /Valor/ }));
    fireEvent.click(screen.getByRole('columnheader', { name: /Valor/ }));
    expect(getVisual('v1').dataConfig).toMatchObject({ sortField: 'Valor', sortOrder: 'desc' });
    const rows = screen.getAllByRole('row').slice(1, -1);
    expect(rows.map(r => within(r).getAllByRole('cell')[0].textContent)).toEqual([
      'Abacaxi',
      'Dado',
      'Borracha',
      'Caneta',
      'Ábaco',
    ]);
  });

  it('slicer: "..." menu sorts its items descending', () => {
    seed(baseVisual({ type: 'slicer', dataConfig: { tableId: 'tv', xAxisField: 'Canal' } }));
    render(<LiveCard id="v1" />);
    const options = () => within(screen.getByRole('combobox')).getAllByRole('option').slice(1).map(o => o.textContent);
    expect(options()).toEqual(['Atacado', 'Loja', 'Online']);
    fireEvent.click(screen.getByRole('button', { name: 'Mais opções' }));
    expect(screen.queryByRole('menuitem', { name: /Classificar por/ })).toBeNull();
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Ordem decrescente/ }));
    expect(getVisual('v1').dataConfig.sortOrder).toBe('desc');
    expect(options()).toEqual(['Online', 'Loja', 'Atacado']);
  });
});

describe('FiltersPane basic filter card value list', () => {
  it('toggles A-Z / Z-A / by count', () => {
    seed(baseVisual({}));
    useBiStore.setState({
      filters: [{ id: 'f1', level: 'report', tableId: 'tv', column: 'Canal', operator: 'equals', value: 'Loja' }],
    });
    render(<FiltersPane />);
    const list = () =>
      within(screen.getByRole('list', { name: 'Canal' }))
        .getAllByRole('listitem')
        .map(li => li.textContent);
    expect(list()).toEqual(['Atacado1', 'Loja1', 'Online3']);
    fireEvent.click(screen.getByRole('button', { name: /Z–A/ }));
    expect(list()).toEqual(['Online3', 'Loja1', 'Atacado1']);
    fireEvent.click(screen.getByRole('button', { name: /#/ }));
    expect(list()[0]).toBe('Online3');
    expect(screen.getByRole('button', { name: /#/ }).getAttribute('aria-pressed')).toBe('true');

    // Checking another value turns the card into an "in" filter, in place.
    fireEvent.click(within(screen.getByRole('list', { name: 'Canal' })).getAllByRole('checkbox')[0]);
    expect(useBiStore.getState().filters[0]).toMatchObject({ operator: 'in', value: ['Loja', 'Online'] });
  });
});
