import { describe, it, expect } from 'vitest';
import { DataEngine } from '../dataEngine';
import { Table, Visual, Filter } from '../../types/bi';

describe('VetroBI Visual Planner - Slicer, SortByColumn & Multi-Card', () => {
  const buildModel = () => {
    const engine = new DataEngine();
    const tables: Table[] = [
      {
        id: 'dEstrutura',
        name: 'dEstrutura',
        originalName: 'dEstrutura',
        columns: [
          { id: 'c1', name: 'Linha', originalName: 'Linha', dataType: 'text', sortByColumn: 'Ordem' },
          { id: 'c2', name: 'Ordem', originalName: 'Ordem', dataType: 'number' },
        ],
        rows: [
          { Linha: 'Resultado Operacional', Ordem: 3 },
          { Linha: 'Receita Bruta', Ordem: 1 },
          { Linha: '(-) Imposto', Ordem: 2 },
        ],
      },
      {
        id: 'fVendas',
        name: 'fVendas',
        originalName: 'fVendas',
        columns: [
          { id: 'v1', name: 'Canal', originalName: 'Canal', dataType: 'text' },
          { id: 'v2', name: 'Valor', originalName: 'Valor', dataType: 'currency' },
        ],
        rows: [
          { Canal: 'Direto', Valor: 1000 },
          { Canal: 'Parceiro', Valor: 2000 },
          { Canal: 'Online', Valor: 3000 },
        ],
      },
    ];
    engine.setSchema(tables, [], [
      { id: 'm1', name: 'Receita', tableName: 'fVendas', formula: 'SUM(fVendas[Valor])', format: 'currency' },
      { id: 'm2', name: 'Qtd', tableName: 'fVendas', formula: 'COUNTROWS(fVendas)', format: 'integer' },
    ]);
    return engine;
  };

  it('keeps all slicer options available after selecting a value on the same column', () => {
    const engine = buildModel();
    const slicerVisual: Visual = {
      id: 'vis_slicer',
      type: 'slicer',
      title: 'Canal',
      x: 0, y: 0, w: 200, h: 100,
      dataConfig: { tableId: 'fVendas', xAxisField: 'Canal' },
      formatConfig: { showTitle: true },
    };
    const activeFilters: Filter[] = [
      { id: 'f1', level: 'report', tableId: 'fVendas', column: 'Canal', operator: 'equals', value: 'Direto' },
    ];
    const res = engine.queryVisualData(slicerVisual, activeFilters);
    expect(res.categories).toEqual(['Direto', 'Online', 'Parceiro']);
  });

  it('sorts categories by sortByColumn when present', () => {
    const engine = buildModel();
    const matrixVisual: Visual = {
      id: 'vis_mat',
      type: 'matrix',
      title: 'Estrutura',
      x: 0, y: 0, w: 400, h: 300,
      dataConfig: { tableId: 'dEstrutura', xAxisField: 'Linha', yAxisFields: ['Ordem'], aggregation: 'SUM' },
      formatConfig: { showTitle: true },
    };
    const res = engine.queryVisualData(matrixVisual, []);
    expect(res.categories).toEqual(['Receita Bruta', '(-) Imposto', 'Resultado Operacional']);
  });

  it('evaluates multi-card across multiple DAX measures', () => {
    const engine = buildModel();
    const multiCard: Visual = {
      id: 'vis_mc',
      type: 'multi-card',
      title: 'Resumo',
      x: 0, y: 0, w: 400, h: 150,
      dataConfig: { tableId: 'fVendas', yAxisFields: ['Receita', 'Qtd'] },
      formatConfig: { showTitle: true },
    };
    const res = engine.queryVisualData(multiCard, []);
    expect(res.categories).toEqual(['Receita', 'Qtd']);
    expect(res.series[0].data).toEqual([6000, 3]);
  });

  it('supports bare boolean ColumnRef filter inside CALCULATE (DFC Saída de Caixa)', () => {
    const engine = new DataEngine();
    const tables: Table[] = [
      {
        id: 'dCategorias',
        name: 'dCategorias',
        originalName: 'dCategorias',
        columns: [
          { id: 'c1', name: 'Categoria', originalName: 'Categoria', dataType: 'text' },
          { id: 'c2', name: 'Saída de Caixa', originalName: 'Saída de Caixa', dataType: 'boolean' },
        ],
        rows: [
          { Categoria: 'Entrada', 'Saída de Caixa': false },
          { Categoria: 'Combustível', 'Saída de Caixa': true },
        ],
      },
      {
        id: 'fLanc',
        name: 'fLanc',
        originalName: 'fLanc',
        columns: [
          { id: 'f1', name: 'Categoria', originalName: 'Categoria', dataType: 'text' },
          { id: 'f2', name: 'Valor', originalName: 'Valor', dataType: 'currency' },
        ],
        rows: [
          { Categoria: 'Entrada', Valor: 1000 },
          { Categoria: 'Combustível', Valor: -250 },
        ],
      },
    ];
    engine.setSchema(
      tables,
      [
        {
          id: 'r1',
          fromTable: 'fLanc',
          fromColumn: 'Categoria',
          toTable: 'dCategorias',
          toColumn: 'Categoria',
          cardinality: 'N:1',
          crossFilterDirection: 'single',
          isActive: true,
        },
      ],
      [
        { id: 'm1', name: 'Resultado DFC', tableName: 'fLanc', formula: 'SUM(fLanc[Valor])', format: 'currency' },
        { id: 'm2', name: 'Saídas', tableName: 'fLanc', formula: 'CALCULATE([Resultado DFC], dCategorias[Saída de Caixa])', format: 'currency' },
      ]
    );
    const card: Visual = {
      id: 'vis_saidas',
      type: 'card',
      title: 'Saídas',
      x: 0, y: 0, w: 200, h: 100,
      dataConfig: { tableId: 'fLanc', yAxisFields: ['Saídas'] },
      formatConfig: { showTitle: true },
    };
    const res = engine.queryVisualData(card, []);
    expect(res.summaryValue).toBe(-250);
  });
});
