// Testes de integração: dirty tracking + aplicar todas as consultas sujas +
// verificação de que os dados do visual (globalDataEngine) realmente mudam
// depois do "Fechar e Aplicar" — a reclamação original era "apliquei e os
// visuais não mudaram". etlApply é simulado (como os demais testes de
// store), mas o que importa aqui é o fio voltando de updateTable() até o
// globalDataEngine que o relatório consulta — não a lógica do pipeline M.
import { describe, it, expect, beforeEach, vi } from 'vitest';

const apply = vi.fn();
vi.mock('../../services/etl/etlClient', async importOriginal => {
  const actual = await importOriginal<typeof import('../../services/etl/etlClient')>();
  return { ...actual, etlApply: (...a: unknown[]) => apply(...a) };
});

const { useBiStore } = await import('../useBiStore');
const { globalDataEngine } = await import('../../engine/dataEngine');
import type { Visual } from '../../types/bi';

const TABLE_ID = 't_vendas';
const ALL_ROWS = [
  { Categoria: 'Eletrônicos', Valor: 100 },
  { Categoria: 'Eletrônicos', Valor: 50 },
  { Categoria: 'Mobiliário', Valor: 200 },
];
const ELETRO_ROWS = ALL_ROWS.filter(r => r.Categoria === 'Eletrônicos');

function resetStore() {
  useBiStore.setState({ tables: [], appliedSteps: {}, appliedQuerySnapshot: {}, selectedStepByTable: {}, measures: [], relationships: [] } as any);
  useBiStore.getState().addTable({
    id: TABLE_ID,
    name: 'f_Vendas',
    originalName: 'f_Vendas',
    columns: [
      { id: 'c_cat', name: 'Categoria', originalName: 'Categoria', dataType: 'text' },
      { id: 'c_valor', name: 'Valor', originalName: 'Valor', dataType: 'number' },
    ],
    rows: ALL_ROWS,
  });
}

function barVisual(): Visual {
  return {
    id: 'v1',
    type: 'bar',
    title: 'Faturamento por Categoria',
    x: 0,
    y: 0,
    w: 200,
    h: 200,
    dataConfig: {
      tableId: TABLE_ID,
      xAxisField: 'Categoria',
      yAxisFields: ['Valor'],
      aggregation: 'SUM',
    },
    formatConfig: {},
  } as any;
}

describe('Power Query dirty tracking + applyAllDirtyQueries', () => {
  beforeEach(() => {
    apply.mockReset();
    resetStore();
  });

  it('a query with no steps is never dirty', () => {
    expect(useBiStore.getState().isQueryDirty(TABLE_ID)).toBe(false);
  });

  it('becomes dirty after adding a step, and clean again after applying it', async () => {
    apply.mockResolvedValue({
      columns: [{ name: 'Categoria', originalName: 'Categoria', dataType: 'text' }, { name: 'Valor', originalName: 'Valor', dataType: 'number' }],
      rows: ELETRO_ROWS,
      totalRows: 2,
      truncated: false,
    });
    const { addQueryStep, isQueryDirty, closeAndApplyQuery } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 's1', name: 'Filtrado', op: 'filterRows', condition: { column: 'Categoria', op: 'eq', value: 'Eletrônicos' } });
    expect(isQueryDirty(TABLE_ID)).toBe(true);

    await closeAndApplyQuery(TABLE_ID);
    expect(useBiStore.getState().isQueryDirty(TABLE_ID)).toBe(false);
  });

  it('editing again after apply makes it dirty once more (compares against the applied snapshot)', async () => {
    apply.mockResolvedValue({ columns: [], rows: ELETRO_ROWS, totalRows: 2, truncated: false });
    const { addQueryStep, closeAndApplyQuery, isQueryDirty, updateQueryStep } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 's1', name: 'Filtrado', op: 'filterRows', condition: { column: 'Categoria', op: 'eq', value: 'Eletrônicos' } });
    await closeAndApplyQuery(TABLE_ID);
    expect(isQueryDirty(TABLE_ID)).toBe(false);

    updateQueryStep(TABLE_ID, 's1', { condition: { column: 'Categoria', op: 'eq', value: 'Mobiliário' } } as any);
    expect(isQueryDirty(TABLE_ID)).toBe(true);
  });

  it('hasPendingQueryChanges reflects any dirty table', () => {
    const { addQueryStep, hasPendingQueryChanges } = useBiStore.getState();
    expect(hasPendingQueryChanges()).toBe(false);
    addQueryStep(TABLE_ID, { id: 's1', name: 'Filtrado', op: 'filterRows', condition: { column: 'Categoria', op: 'eq', value: 'Eletrônicos' } });
    expect(hasPendingQueryChanges()).toBe(true);
  });

  it('applyAllDirtyQueries applies the dirty table and reports its row count + truncation', async () => {
    apply.mockResolvedValue({
      columns: [{ name: 'Categoria', originalName: 'Categoria', dataType: 'text' }, { name: 'Valor', originalName: 'Valor', dataType: 'number' }],
      rows: ELETRO_ROWS,
      totalRows: 7000,
      truncated: true,
    });
    const { addQueryStep, applyAllDirtyQueries } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 's1', name: 'Filtrado', op: 'filterRows', condition: { column: 'Categoria', op: 'eq', value: 'Eletrônicos' } });

    const progressCalls: any[] = [];
    const result = await applyAllDirtyQueries(info => progressCalls.push(info));

    expect(result.errors).toBeNull();
    expect(result.applied).toHaveLength(1);
    expect(result.applied[0]).toMatchObject({ tableId: TABLE_ID, name: 'f_Vendas', rowCount: 7000, truncated: true });
    expect(progressCalls.length).toBeGreaterThan(0);

    const table = useBiStore.getState().tables.find(t => t.id === TABLE_ID)!;
    expect(table.rows).toEqual(ELETRO_ROWS);
    expect(useBiStore.getState().isQueryDirty(TABLE_ID)).toBe(false);
  });

  it('applying with no dirty queries is a no-op', async () => {
    const result = await useBiStore.getState().applyAllDirtyQueries();
    expect(result).toEqual({ applied: [], errors: null });
    expect(apply).not.toHaveBeenCalled();
  });

  // ROOT CAUSE CHECK: "Fechar e Aplicar" must make the report's visual data change.
  // This exercises the actual query engine the report reads from (globalDataEngine),
  // not just the store's table rows, to catch cache-key/schemaVersion regressions.
  it('end-to-end: after applying, the visual query (globalDataEngine) returns the new data', async () => {
    globalDataEngine.setSchema(useBiStore.getState().tables, [], []);
    const visual = barVisual();

    const before = globalDataEngine.queryVisualData(visual, []);
    expect([...before.categories].sort()).toEqual(['Eletrônicos', 'Mobiliário']);
    expect(before.series[0].data.reduce<number>((a, b) => a + (b ?? 0), 0)).toBe(350);

    apply.mockResolvedValue({
      columns: [{ name: 'Categoria', originalName: 'Categoria', dataType: 'text' }, { name: 'Valor', originalName: 'Valor', dataType: 'number' }],
      rows: ELETRO_ROWS,
      totalRows: 2,
      truncated: false,
    });
    const { addQueryStep, applyAllDirtyQueries } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 's1', name: 'Filtrado', op: 'filterRows', condition: { column: 'Categoria', op: 'eq', value: 'Eletrônicos' } });
    const result = await applyAllDirtyQueries();
    expect(result.errors).toBeNull();

    // A tabela foi atualizada via updateTable(), que já chama globalDataEngine.setSchema() —
    // não deveria ser preciso fazer mais nada para o visual "ver" o novo dado.
    const after = globalDataEngine.queryVisualData(visual, []);
    expect(after.categories).toEqual(['Eletrônicos']);
    expect(after.series[0].data.reduce<number>((a, b) => a + (b ?? 0), 0)).toBe(150);
  });

  it('stops at the first failing query, reports which one/step failed, and keeps the query dirty', async () => {
    apply.mockRejectedValue(new Error('Coluna "ColunaQueNaoExiste" não existe'));
    const { addQueryStep, applyAllDirtyQueries } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 'bad', name: 'Quebrado', op: 'groupBy', keys: ['ColunaQueNaoExiste'], aggregations: [{ name: 'Total', op: 'sum', column: 'Valor' }] });

    const result = await applyAllDirtyQueries();
    expect(result.errors).not.toBeNull();
    expect(result.errors?.tableId).toBe(TABLE_ID);
    expect(result.errors?.name).toBe('f_Vendas');
    expect(result.errors?.message).toMatch(/ColunaQueNaoExiste/);
    // A consulta continua suja (não aplicada) — o editor deve continuar aberto.
    expect(useBiStore.getState().isQueryDirty(TABLE_ID)).toBe(true);
  });

  it('applies dependent queries (mergeQueries) before the one that references them', async () => {
    useBiStore.getState().addTable({
      id: 't_cal',
      name: 'd_Calendario',
      originalName: 'd_Calendario',
      columns: [{ id: 'c_data', name: 'Data', originalName: 'Data', dataType: 'date' }],
      rows: [{ Data: '2026-01-05' }],
    });
    const callOrder: string[] = [];
    apply.mockImplementation(async (args: any) => {
      callOrder.push(args.targetTable);
      return { columns: [], rows: [], totalRows: 0, truncated: false };
    });

    const { addQueryStep, applyAllDirtyQueries } = useBiStore.getState();
    // f_Vendas (adicionada primeiro) referencia d_Calendario via mergeQueries.
    addQueryStep(TABLE_ID, {
      id: 'm1',
      name: 'Mesclado',
      op: 'mergeQueries',
      rightQuery: 'd_Calendario',
      leftKeys: ['Categoria'],
      rightKeys: ['Data'],
      joinKind: 'leftOuter',
      expand: [],
    });
    addQueryStep('t_cal', { id: 's1', name: 'Filtrado', op: 'filterRows', condition: { column: 'Data', op: 'eq', value: '2026-01-05' } });

    const result = await applyAllDirtyQueries();
    expect(result.errors).toBeNull();
    expect(callOrder).toEqual(['d_Calendario', 'f_Vendas']);
  });
});
