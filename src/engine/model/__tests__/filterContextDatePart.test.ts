// v0.11 Frente D: filtro de cross-filter por parte de data (ano/trimestre/mês/dia),
// o mesmo mecanismo usado pelo clique num ponto drillado de data (ver drill.ts /
// useVisualDrill / HierarchicalMatrix). Garante que `filterContextFromUIFilters`
// filtra pela PARTE da data (via `datePartValue`), não pelo valor cru da coluna.
import { describe, it, expect } from 'vitest';
import { filterContextFromUIFilters } from '../filterContext';
import { InMemoryModelAccess } from '../modelAccess';
import { Filter, Table } from '../../../types/bi';

const fVendas: Table = {
  id: 'fv',
  name: 'fVendas',
  originalName: 'fVendas',
  columns: [
    { id: '1', name: 'Data', originalName: 'Data', dataType: 'date' },
    { id: '2', name: 'Valor', originalName: 'Valor', dataType: 'number' },
  ],
  rows: [
    { Data: '2023-01-10', Valor: 10 }, // Q1 2023
    { Data: '2023-04-10', Valor: 20 }, // Q2 2023
    { Data: '2024-04-20', Valor: 30 }, // Q2 2024
    { Data: '2024-07-05', Valor: 40 }, // Q3 2024
  ],
};

const model = new InMemoryModelAccess([fVendas], [], []);

describe('filterContextFromUIFilters: filtro por datePart', () => {
  it('filtra pelo trimestre (2), não pelo valor cru da coluna Data', () => {
    const filter: Filter = { id: 'f1', level: 'visual', tableId: 'fv', column: 'Data', operator: 'equals', value: 2, datePart: 'quarter' };
    const ctx = filterContextFromUIFilters([filter], model);
    const rows = ctx.getFilteredRows('fv', model);
    expect(rows.map(r => r.Valor).sort((a: any, b: any) => a - b)).toEqual([20, 30]);
  });

  it('filtra pelo ano (2024)', () => {
    const filter: Filter = { id: 'f2', level: 'visual', tableId: 'fv', column: 'Data', operator: 'equals', value: 2024, datePart: 'year' };
    const ctx = filterContextFromUIFilters([filter], model);
    const rows = ctx.getFilteredRows('fv', model);
    expect(rows.map(r => r.Valor).sort((a: any, b: any) => a - b)).toEqual([30, 40]);
  });

  it('ano + trimestre juntos (dois filtros de visual) restringem à interseção', () => {
    const filters: Filter[] = [
      { id: 'f3', level: 'visual', tableId: 'fv', column: 'Data', operator: 'equals', value: 2024, datePart: 'year' },
      { id: 'f4', level: 'visual', tableId: 'fv', column: 'Data', operator: 'equals', value: 2, datePart: 'quarter' },
    ];
    const ctx = filterContextFromUIFilters(filters, model);
    const rows = ctx.getFilteredRows('fv', model);
    expect(rows.map(r => r.Valor)).toEqual([30]);
  });
});
