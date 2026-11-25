// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

// A tela de relatório usa ECharts (precisa de canvas real) e ResizeObserver —
// não é o que este teste cobre (a barra de alterações pendentes), então mocka os dois.
vi.mock('echarts-for-react', () => ({ default: () => <div data-testid="echarts" /> }));
(global as any).ResizeObserver =
  (global as any).ResizeObserver ||
  class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };

import { useBiStore } from '../../../store/useBiStore';
import { ReportView } from '../ReportView';
import { Table } from '../../../types/bi';

const table: Table = {
  id: 't1',
  name: 'Vendas',
  originalName: 'Vendas',
  columns: [{ id: 'c1', name: 'Categoria', originalName: 'Categoria', dataType: 'text' }],
  rows: [{ Categoria: 'A' }],
} as Table;

function seed() {
  const st = useBiStore.getState();
  st.newProject('Pending Bar Test');
  st.setTables([table]);
  useBiStore.setState({ appliedSteps: {}, appliedQuerySnapshot: {} });
}

describe('<ReportView /> pending Power Query changes bar', () => {
  beforeEach(() => {
    seed();
  });

  it('is hidden when there are no dirty queries', () => {
    render(<ReportView />);
    expect(screen.queryByTestId('pending-query-changes-bar')).toBeNull();
  });

  it('shows "Há alterações pendentes…" with an Apply button once a query is dirty', () => {
    useBiStore.getState().addQueryStep('t1', { id: 's1', name: 'Filtrado', op: 'filterRows', condition: { column: 'Categoria', op: 'eq', value: 'A' } });
    render(<ReportView />);
    expect(screen.getByTestId('pending-query-changes-bar')).toBeTruthy();
    expect(screen.getByText(/Há alterações pendentes nas suas consultas/)).toBeTruthy();
    expect(screen.getByTestId('apply-pending-changes-btn')).toBeTruthy();
  });

  it('clicking "Aplicar alterações" applies the dirty query and the bar disappears', async () => {
    useBiStore.getState().addQueryStep('t1', { id: 's1', name: 'Filtrado', op: 'filterRows', condition: { column: 'Categoria', op: 'eq', value: 'A' } });
    render(<ReportView />);
    fireEvent.click(screen.getByTestId('apply-pending-changes-btn'));
    await waitFor(() => expect(screen.queryByTestId('pending-query-changes-bar')).toBeNull());
    expect(useBiStore.getState().isQueryDirty('t1')).toBe(false);
  });
});
