import { describe, it, expect, beforeEach } from 'vitest';
import { useBiStore } from '../useBiStore';

describe('Table Management & Data Refresh (v0.5.0)', () => {
  beforeEach(() => {
    useBiStore.getState().newProject('Teste Gestão de Tabelas');
    useBiStore.getState().addTables([
      {
        id: 'fLancamentos',
        name: 'fLancamentos',
        originalName: 'Fato',
        sourceConfig: {
          type: 'xlsx',
          filePath: '/tmp/base_inexistente.xlsx',
          sheetName: 'Fato',
        },
        columns: [
          { id: 'c1', name: 'Categoria', originalName: 'Categoria', dataType: 'text' },
          { id: 'c2', name: 'Valor', originalName: 'Valor', dataType: 'currency' },
        ],
        rows: [
          { Categoria: 'Serviços', Valor: 100 },
          { Categoria: 'Produtos', Valor: 250 },
        ],
      },
      {
        id: 'dCategorias',
        name: 'dCategorias',
        originalName: 'dCategorias',
        columns: [{ id: 'c3', name: 'Categoria', originalName: 'Categoria', dataType: 'text' }],
        rows: [{ Categoria: 'Serviços' }, { Categoria: 'Produtos' }],
      },
    ]);

    useBiStore.getState().addRelationship({
      id: 'rel_1',
      fromTable: 'fLancamentos',
      fromColumn: 'Categoria',
      toTable: 'dCategorias',
      toColumn: 'Categoria',
      cardinality: 'N:1',
      crossFilterDirection: 'single',
      isActive: true,
    });

    useBiStore.getState().addMeasure({
      id: 'm_total',
      name: 'Total Geral',
      tableName: 'fLancamentos',
      tableId: 'fLancamentos',
      formula: 'SUM(fLancamentos[Valor])',
      format: 'currency',
    });

    useBiStore.getState().addVisual('column', 'fLancamentos');
    useBiStore.getState().addFilter({
      id: 'flt_1',
      level: 'report',
      tableId: 'fLancamentos',
      column: 'Categoria',
      operator: 'equals',
      value: 'Serviços',
    });
  });

  it('renameTable atualiza tabela, relacionamentos, medidas DAX, visuais e filtros', () => {
    const res = useBiStore.getState().renameTable('fLancamentos', 'fVendasReal');
    expect(res.ok).toBe(true);

    const state = useBiStore.getState();
    expect(state.tables.some(t => t.name === 'fVendasReal' && t.id === 'fVendasReal')).toBe(true);
    expect(state.tables.some(t => t.name === 'fLancamentos')).toBe(false);

    expect(state.relationships[0].fromTable).toBe('fVendasReal');
    expect(state.measures[0].tableName).toBe('fVendasReal');
    expect(state.measures[0].tableId).toBe('fVendasReal');
    expect(state.measures[0].formula).toBe('SUM(fVendasReal[Valor])');

    const visual = state.pages[0].visuals[0];
    expect(visual.dataConfig.tableId).toBe('fVendasReal');
    expect(state.filters[0].tableId).toBe('fVendasReal');
  });

  it('replaceTableData substitui linhas e sourceConfig preservando visuais, medidas e relacionamentos', () => {
    const prevVersion = useBiStore.getState().dataVersion;
    const ok = useBiStore.getState().replaceTableData(
      'fLancamentos',
      [
        { id: 'new_1', name: 'Categoria', originalName: 'Categoria', dataType: 'text' },
        { id: 'new_2', name: 'Valor', originalName: 'Valor', dataType: 'currency' },
      ],
      [{ Categoria: 'Assinatura', Valor: 999 }],
      {
        type: 'xlsx',
        filePath: '/dados/planilha_v2.xlsx',
        sheetName: 'Aba_Fato_Nova',
      }
    );

    expect(ok).toBe(true);
    const state = useBiStore.getState();
    const updated = state.tables.find(t => t.id === 'fLancamentos')!;
    expect(updated.rows).toHaveLength(1);
    expect(updated.rows[0].Valor).toBe(999);
    expect(updated.sourceConfig?.sheetName).toBe('Aba_Fato_Nova');
    expect(updated.sourceConfig?.filePath).toBe('/dados/planilha_v2.xlsx');
    expect(state.measures).toHaveLength(1);
    expect(state.relationships).toHaveLength(1);
    expect(state.pages[0].visuals).toHaveLength(1);
    expect(state.dataVersion).toBeGreaterThan(prevVersion);
  });

  it('refreshAllData nunca apaga tabelas importadas para carregar amostra padrão', async () => {
    const prevVersion = useBiStore.getState().dataVersion;
    await useBiStore.getState().refreshAllData();

    const state = useBiStore.getState();
    expect(state.tables.some(t => t.name === 'fLancamentos')).toBe(true);
    expect(state.tables.some(t => t.name === 'f_Vendas')).toBe(false);
    expect(state.dataVersion).toBeGreaterThan(prevVersion);
  });
});
