import { describe, it, expect, beforeEach } from 'vitest';
import { etlLoadRaw, etlPreview, etlApply, etlToM, setBrowserRawProvider } from '../etlClient';
import { EtlQuery } from '../../../types/etl';

const rawTable = {
  columns: [
    { name: 'Nome', type: 'text' },
    { name: 'Valor', type: 'decimal' },
    { name: 'Regiao', type: 'text' },
  ],
  rows: [
    { Nome: 'A', Valor: 10, Regiao: 'SP' },
    { Nome: 'b', Valor: 20, Regiao: 'RJ' },
    { Nome: 'C', Valor: 30, Regiao: 'SP' },
  ],
};

beforeEach(() => {
  setBrowserRawProvider(name => (name === 'Vendas' ? rawTable : undefined));
});

function baseQuery(steps: EtlQuery['steps']): EtlQuery {
  return { name: 'Vendas', source: { kind: 'table', table: 'Vendas' }, steps };
}

describe('etlClient browser fallback', () => {
  it('etlLoadRaw returns schema and row count from the provider', async () => {
    const result = await etlLoadRaw({ query: 'Vendas', source: { kind: 'table' } });
    expect(result.rawTable).toBe('raw__Vendas');
    expect(result.rowCount).toBe(3);
    expect(result.columns).toHaveLength(3);
  });

  it('applies filterRows (contains) and reports the browser fallback notice', async () => {
    const query = baseQuery([{ id: 's1', name: 'Filtro', op: 'filterRows', condition: { column: 'Regiao', op: 'eq', value: 'SP' } }]);
    const result = await etlPreview({ query, limit: 200 });
    expect(result.rowCount).toBe(2);
    expect(result.rows.every(r => r.Regiao === 'SP')).toBe(true);
    expect(result.warnings.some(w => w.includes('Tauri'))).toBe(true);
  });

  it('applies textTransform upper', async () => {
    const query = baseQuery([{ id: 's1', name: 'Maiusculas', op: 'textTransform', columns: ['Nome'], fn: 'upper' }]);
    const result = await etlPreview({ query, limit: 200 });
    expect(result.rows.map(r => r.Nome)).toEqual(['A', 'B', 'C']);
  });

  it('applies groupBy with sum aggregation', async () => {
    const query = baseQuery([
      { id: 's1', name: 'Agrupado', op: 'groupBy', keys: ['Regiao'], aggregations: [{ name: 'Total', op: 'sum', column: 'Valor' }] },
    ]);
    const result = await etlPreview({ query, limit: 200 });
    const sp = result.rows.find(r => r.Regiao === 'SP');
    expect(sp?.Total).toBe(40);
  });

  it('applies addIndexColumn', async () => {
    const query = baseQuery([{ id: 's1', name: 'Indice', op: 'addIndexColumn', newColumn: 'Idx', start: 1, step: 1 }]);
    const result = await etlPreview({ query, limit: 200 });
    expect(result.rows.map(r => r.Idx)).toEqual([1, 2, 3]);
  });

  it('reports failedStep and truncated data when a step throws', async () => {
    const query = baseQuery([
      { id: 's1', name: 'Boa', op: 'textTransform', columns: ['Nome'], fn: 'upper' },
      { id: 's2', name: 'Custom quebrado', op: 'addCustomColumn', newColumn: 'X', expression: 'each [Inexistente](' },
    ]);
    const result = await etlPreview({ query, limit: 200 });
    expect(result.failedStep?.index).toBe(1);
    expect(result.rows).toHaveLength(3);
  });

  it('etlApply returns an ImportedTablePayload-shaped result', async () => {
    const query = baseQuery([{ id: 's1', name: 'Selecionar', op: 'selectColumns', columns: ['Nome', 'Valor'] }]);
    const payload = await etlApply({ query, targetTable: 'Vendas' });
    expect(payload.tableName).toBe('Vendas');
    expect(payload.columns.map(c => c.name)).toEqual(['Nome', 'Valor']);
    expect(payload.totalRows).toBe(3);
  });

  it('etlToM renders a readable M script for common steps', async () => {
    const query = baseQuery([{ id: 's1', name: 'Removidas Colunas', op: 'removeColumns', columns: ['Regiao'] }]);
    const m = await etlToM({ query });
    expect(m).toContain('let');
    expect(m).toContain('#"Removidas Colunas"');
    expect(m).toContain('Table.RemoveColumns');
  });
});
