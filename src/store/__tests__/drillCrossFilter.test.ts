// v0.11 Frente D: cross-filter a partir de um ponto drillado de data (ano/trimestre/
// mês/dia) ou de um caminho concatenado (modo expandido da matriz/gráfico) — um
// `Filter` por `DrillFilterSpec`, clicar de novo no mesmo ponto limpa o filtro.
import { describe, it, expect, beforeEach } from 'vitest';
import { useBiStore } from '../useBiStore';
import { Table } from '../../types/bi';

const table: Table = {
  id: 't1',
  name: 'fVendas',
  originalName: 'fVendas',
  columns: [
    { id: 'c1', name: 'Data', originalName: 'Data', dataType: 'date' },
    { id: 'c2', name: 'Regiao', originalName: 'Regiao', dataType: 'text' },
  ],
  rows: [],
} as Table;

beforeEach(() => {
  const st = useBiStore.getState();
  st.newProject('Drill cross-filter teste');
  st.setTables([table]);
  useBiStore.setState({ filters: [] });
});

describe('setDrillCrossFilter', () => {
  it('cria um Filter por spec, com datePart e label legível', () => {
    useBiStore.getState().setDrillCrossFilter([
      { field: 'Data', datePart: 'quarter', value: 2, label: 'Trimestre: Trim 2' },
    ]);
    const filters = useBiStore.getState().filters;
    expect(filters).toHaveLength(1);
    expect(filters[0]).toMatchObject({ tableId: 't1', column: 'Data', datePart: 'quarter', value: 2, label: 'Trimestre: Trim 2' });
  });

  it('caminho concatenado (ano + trimestre) cria dois Filters', () => {
    useBiStore.getState().setDrillCrossFilter([
      { field: 'Data', datePart: 'year', value: 2024, label: 'Ano: 2024' },
      { field: 'Data', datePart: 'quarter', value: 2, label: 'Trimestre: Trim 2' },
    ]);
    const filters = useBiStore.getState().filters;
    expect(filters).toHaveLength(2);
    expect(filters.map(f => f.datePart).sort()).toEqual(['quarter', 'year']);
  });

  it('clicar de novo no mesmo ponto (mesmos specs/valores) limpa o filtro', () => {
    const specs = [{ field: 'Data', datePart: 'quarter' as const, value: 2, label: 'Trimestre: Trim 2' }];
    useBiStore.getState().setDrillCrossFilter(specs);
    expect(useBiStore.getState().filters).toHaveLength(1);
    useBiStore.getState().setDrillCrossFilter(specs);
    expect(useBiStore.getState().filters).toHaveLength(0);
  });

  it('clicar num ponto diferente substitui o filtro de drill anterior (mesmo campo+parte)', () => {
    useBiStore.getState().setDrillCrossFilter([{ field: 'Data', datePart: 'quarter', value: 2, label: 'Trimestre: Trim 2' }]);
    useBiStore.getState().setDrillCrossFilter([{ field: 'Data', datePart: 'quarter', value: 3, label: 'Trimestre: Trim 3' }]);
    const filters = useBiStore.getState().filters;
    expect(filters).toHaveLength(1);
    expect(filters[0].value).toBe(3);
  });

  it('resolve campo qualificado "Tabela.Coluna" para o tableId correto', () => {
    useBiStore.getState().setDrillCrossFilter([{ field: 'fVendas.Regiao', value: 'Sul', label: 'Regiao: Sul' }]);
    const filters = useBiStore.getState().filters;
    expect(filters[0]).toMatchObject({ tableId: 't1', column: 'Regiao', value: 'Sul' });
  });
});
