import { describe, it, expect } from 'vitest';
import { InMemoryModelAccess } from '../../../../engine/model/modelAccess';
import { Filter, Measure, Table, Visual } from '../../../../types/bi';
import { queryLevelsTs, queryMatrixTs, MatrixNode } from '../engineQuery';
import { DrillLevel, COLLAPSED_MATRIX, expandAllMatrix, toggleMatrixPath } from '../../../../engine/drill';

// Mesmas fixtures dos testes Rust (src-tauri/src/dax/tests_drill.rs); valores derivados à mão.
const stores: Table = {
  id: 'fv',
  name: 'fVendas',
  originalName: 'fVendas',
  columns: ['Regiao', 'Cidade', 'Loja', 'Cliente', 'Valor'].map((n, i) => ({
    id: `c${i}`,
    name: n,
    originalName: n,
    dataType: n === 'Valor' ? 'number' : 'text',
  })),
  rows: [
    ['Sul', 'POA', 'L1', 'c1', 10],
    ['Sul', 'POA', 'L1', 'c2', 20],
    ['Sul', 'POA', 'L2', 'c1', 30],
    ['Sul', 'Curitiba', 'L3', 'c1', 40],
    ['Sul', 'Curitiba', 'L3', 'c3', 50],
    ['Norte', 'Manaus', 'L4', 'c2', 60],
    ['Norte', 'Manaus', 'L4', 'c4', 70],
  ].map(([Regiao, Cidade, Loja, Cliente, Valor]) => ({ Regiao, Cidade, Loja, Cliente, Valor })),
} as Table;

const pedidos: Table = {
  id: 'fp',
  name: 'fPedidos',
  originalName: 'fPedidos',
  columns: [
    { id: 'd', name: 'Data', originalName: 'Data', dataType: 'date' },
    { id: 'v', name: 'Valor', originalName: 'Valor', dataType: 'number' },
  ],
  rows: [
    { Data: '2023-11-05', Valor: 10 },
    { Data: '2024-01-10', Valor: 5 },
    { Data: '2024-02-20', Valor: 7 },
    { Data: '2024-04-01', Valor: 3 },
    { Data: '2024-12-31', Valor: 1 },
  ],
} as Table;

const measures: Measure[] = [
  { id: 'm1', name: 'Clientes', formula: 'DISTINCTCOUNT(fVendas[Cliente])', tableName: 'fVendas' } as Measure,
  { id: 'm2', name: 'Media', formula: 'AVERAGE(fVendas[Valor])', tableName: 'fVendas' } as Measure,
  { id: 'm3', name: 'Total', formula: 'SUM(fPedidos[Valor])', tableName: 'fPedidos' } as Measure,
];

const model = new InMemoryModelAccess([stores, pedidos], [], measures);

const visual = (over: Partial<Visual['dataConfig']>, type: Visual['type'] = 'column'): Visual => ({
  id: 'v1',
  type,
  title: 'V',
  x: 0,
  y: 0,
  w: 400,
  h: 300,
  dataConfig: { tableId: 'fv', ...over },
  formatConfig: { showTitle: true },
});

const plain = (field: string): DrillLevel => ({ field, caption: field });
const datePart = (part: DrillLevel['datePart']): DrillLevel => ({ field: 'Data', datePart: part, caption: String(part) });
const find = (nodes: MatrixNode[], label: string) => {
  const n = nodes.find(x => x.label === label);
  if (!n) throw new Error(`nó ${label} ausente: ${nodes.map(x => x.label).join(',')}`);
  return n;
};

describe('queryMatrixTs: N níveis com subtotal avaliado pelo motor', () => {
  const rows = [plain('Regiao'), plain('Cidade'), plain('Loja')];

  it('DISTINCTCOUNT: subtotal de cada nível ≠ soma dos filhos', () => {
    const res = queryMatrixTs(model, {
      visual: visual({ yAxisFields: ['Clientes'] }, 'matrix'),
      filters: [],
      levels: rows,
      expansion: expandAllMatrix(3),
    });
    const sul = find(res.rows, 'Sul');
    const poa = find(sul.children, 'POA');
    expect(find(poa.children, 'L1').values).toEqual([2]);
    expect(find(poa.children, 'L2').values).toEqual([1]);
    expect(poa.values).toEqual([2]); // {c1,c2}; soma das lojas seria 3
    expect(sul.values).toEqual([3]); // {c1,c2,c3}; POA+Curitiba seria 4
    expect(find(res.rows, 'Norte').values).toEqual([2]);
    expect(res.columnTotals).toEqual([4]); // {c1..c4}; Sul+Norte seria 5
  });

  it('AVERAGE: POA = 60/3 = 20 (não média das médias 22,5)', () => {
    const res = queryMatrixTs(model, {
      visual: visual({ yAxisFields: ['Media'] }, 'matrix'),
      filters: [],
      levels: rows,
      expansion: expandAllMatrix(3),
    });
    expect(find(find(res.rows, 'Sul').children, 'POA').values).toEqual([20]);
    expect(res.columnTotals).toEqual([40]);
  });

  it('expansão: recolhido não calcula filhos; toggle expande só a linha', () => {
    const v = visual({ yAxisFields: ['Clientes'] }, 'matrix');
    const collapsed = queryMatrixTs(model, { visual: v, filters: [], levels: rows, expansion: COLLAPSED_MATRIX });
    expect(collapsed.rows.map(r => r.label)).toEqual(['Norte', 'Sul']);
    expect(collapsed.rows.every(r => r.hasChildren && !r.expanded && r.children.length === 0)).toBe(true);
    const one = queryMatrixTs(model, {
      visual: v,
      filters: [],
      levels: rows,
      expansion: toggleMatrixPath(COLLAPSED_MATRIX, ['Sul']),
    });
    expect(find(one.rows, 'Sul').children.map(c => c.label)).toEqual(['Curitiba', 'POA']);
    expect(find(one.rows, 'Norte').children).toEqual([]);
  });

  it('legenda (colunas): total da linha avaliado sem o filtro da coluna', () => {
    const res = queryMatrixTs(model, {
      visual: visual({ yAxisFields: ['Clientes'], legendField: 'Loja' }, 'matrix'),
      filters: [],
      levels: [plain('Regiao'), plain('Cidade')],
      expansion: COLLAPSED_MATRIX,
    });
    expect(res.seriesNames).toEqual(['L1', 'L2', 'L3', 'L4']);
    const sul = find(res.rows, 'Sul');
    expect(sul.values).toEqual([2, 1, 2, null]);
    expect(sul.rowTotal).toBe(3);
    expect(res.columnTotals).toEqual([2, 1, 2, 2]);
    expect(res.grandTotal).toBe(4);
  });

  it('respeita filtros do relatório (filtro em Cliente)', () => {
    const f: Filter = { id: 'f', level: 'report', tableId: 'fv', column: 'Cliente', operator: 'in', value: ['c1'] };
    const res = queryMatrixTs(model, {
      visual: visual({ yAxisFields: ['Clientes'] }, 'matrix'),
      filters: [f],
      levels: rows,
      expansion: COLLAPSED_MATRIX,
    });
    // Só Sul tem c1; Norte some (BLANK).
    expect(res.rows.map(r => [r.label, r.values[0]])).toEqual([['Sul', 1]]);
  });

  it('colunas e linhas da matriz respeitam sortByColumn e omitir linhas só com BLANK / SUMX vazio', () => {
    const dCal: Table = {
      id: 'dc',
      name: 'dCal',
      originalName: 'dCal',
      columns: [
        { id: 'c1', name: 'Data', originalName: 'Data', dataType: 'date' },
        { id: 'c2', name: 'MesNome', originalName: 'MesNome', dataType: 'text', sortByColumn: 'MesNum' },
        { id: 'c3', name: 'MesNum', originalName: 'MesNum', dataType: 'number' },
      ],
      rows: [
        { Data: '2026-01-10', MesNome: 'jan', MesNum: 1 },
        { Data: '2026-04-10', MesNome: 'abr', MesNum: 4 },
        { Data: '2026-11-10', MesNome: 'nov', MesNum: 11 },
      ],
    } as Table;
    const dEst: Table = {
      id: 'de',
      name: 'dEstrutura',
      originalName: 'dEstrutura',
      columns: [
        { id: 'e1', name: 'Linha', originalName: 'Linha', dataType: 'text', sortByColumn: 'Ordem' },
        { id: 'e2', name: 'Ordem', originalName: 'Ordem', dataType: 'number' },
      ],
      rows: [
        { Linha: 'Receita', Ordem: 1 },
        { Linha: 'ZeroReal', Ordem: 2 },
        { Linha: '—', Ordem: 3 },
      ],
    } as Table;
    const fLanc: Table = {
      id: 'fl',
      name: 'fLanc',
      originalName: 'fLanc',
      columns: [
        { id: 'l1', name: 'Data', originalName: 'Data', dataType: 'date' },
        { id: 'l2', name: 'Linha', originalName: 'Linha', dataType: 'text' },
        { id: 'l3', name: 'Valor', originalName: 'Valor', dataType: 'number' },
      ],
      rows: [
        { Data: '2026-01-10', Linha: 'Receita', Valor: 1000 },
        { Data: '2026-11-10', Linha: 'Receita', Valor: 2500 },
        { Data: '2026-04-10', Linha: 'ZeroReal', Valor: 0 },
      ],
    } as Table;
    const mDre: Measure[] = [
      { id: 'md', name: 'DRE Valor', formula: 'SUMX(FILTER(fLanc, fLanc[Valor] >= 0), fLanc[Valor])', tableName: 'fLanc' } as Measure,
    ];
    const rels = [
      { id: 'r1', fromTable: 'fLanc', fromColumn: 'Data', toTable: 'dCal', toColumn: 'Data', cardinality: 'N:1' as const, crossFilterDirection: 'single' as const, isActive: true },
      { id: 'r2', fromTable: 'fLanc', fromColumn: 'Linha', toTable: 'dEstrutura', toColumn: 'Linha', cardinality: 'N:1' as const, crossFilterDirection: 'single' as const, isActive: true },
    ];
    const dreModel = new InMemoryModelAccess([dCal, dEst, fLanc], rels, mDre);
    const res = queryMatrixTs(dreModel, {
      visual: visual({ tableId: 'fl', xAxisField: 'dEstrutura.Linha', legendField: 'dCal.MesNome', yAxisFields: ['DRE Valor'] }, 'matrix'),
      filters: [],
      levels: [plain('dEstrutura.Linha')],
      expansion: COLLAPSED_MATRIX,
    });
    expect(res.seriesNames).toEqual(['jan', 'abr', 'nov']);
    expect(res.rows.map(r => r.label)).toEqual(['Receita', 'ZeroReal']);
    expect(res.rows[0].values).toEqual([1000, null, 2500]);
    expect(res.rows[1].values).toEqual([null, 0, null]);
  });
});

describe('queryLevelsTs: drill e hierarquia de data', () => {
  const v = visual({ tableId: 'fp', xAxisField: 'Data', yAxisFields: ['Total'] });

  it('nível Ano', () => {
    const res = queryLevelsTs(model, { visual: v, filters: [], levels: [datePart('year')], drillFilters: [] });
    expect(res.categoryKeys).toEqual([[2023], [2024]]);
    expect(res.categories).toEqual(['2023', '2024']);
    expect(res.series[0].data).toEqual([10, 16]);
  });

  it('drill em 2024 → trimestres (T3 sem dados some)', () => {
    const res = queryLevelsTs(model, {
      visual: v,
      filters: [],
      levels: [datePart('quarter')],
      drillFilters: [{ field: 'Data', datePart: 'year', value: 2024 }],
    });
    expect(res.categories).toEqual(['Trim 1', 'Trim 2', 'Trim 4']);
    expect(res.series[0].data).toEqual([12, 3, 1]);
  });

  it('próximo nível sem filtro agrega todos os anos', () => {
    const res = queryLevelsTs(model, { visual: v, filters: [], levels: [datePart('quarter')], drillFilters: [] });
    expect(res.series[0].data).toEqual([12, 3, 11]);
  });

  it('expandir todos os níveis: rótulos concatenados', () => {
    const res = queryLevelsTs(model, {
      visual: v,
      filters: [],
      levels: [datePart('year'), datePart('quarter')],
      drillFilters: [],
    });
    expect(res.categories).toEqual(['2023 Trim 4', '2024 Trim 1', '2024 Trim 2', '2024 Trim 4']);
    expect(res.series[0].data).toEqual([10, 12, 3, 1]);
  });

  it('filtro do drill se intersecta com o segmentador na mesma coluna', () => {
    const slicer: Filter = {
      id: 's',
      level: 'page',
      tableId: 'fp',
      column: 'Data',
      operator: 'in',
      value: ['2023-11-05', '2024-01-10'],
    };
    const res = queryLevelsTs(model, {
      visual: v,
      filters: [slicer],
      levels: [datePart('quarter')],
      drillFilters: [{ field: 'Data', datePart: 'year', value: 2024 }],
    });
    expect(res.categories).toEqual(['Trim 1']);
    expect(res.series[0].data).toEqual([5]);
  });

  it('campo simples: drill em Regiao=Sul → cidades', () => {
    const res = queryLevelsTs(model, {
      visual: visual({ xAxisField: 'Regiao', yAxisFields: ['Clientes'] }),
      filters: [],
      levels: [plain('Cidade')],
      drillFilters: [{ field: 'Regiao', value: 'Sul' }],
    });
    expect(res.categories).toEqual(['Curitiba', 'POA']);
    expect(res.series[0].data).toEqual([2, 2]);
  });
});

describe('conversões do caminho nativo (contrato com query_planner.rs)', () => {
  it('requisição: níveis e filtros de drill com datePart explícito (null para campo simples)', async () => {
    const { toNativeLevels, toNativeDrillFilters, baseNativeRequest } = await import('../engineQuery');
    expect(toNativeLevels([plain('Regiao'), datePart('year')])).toEqual([
      { field: 'Regiao', datePart: null },
      { field: 'Data', datePart: 'year' },
    ]);
    expect(toNativeDrillFilters([{ field: 'Data', datePart: 'year', value: 2024 }, { field: 'Regiao', value: 'Sul' }])).toEqual([
      { field: 'Data', datePart: 'year', value: 2024 },
      { field: 'Regiao', datePart: null, value: 'Sul' },
    ]);
    const f: Filter = { id: 'f', level: 'report', tableId: 'fv', column: 'Cliente', operator: 'in', value: ['c1'] };
    expect(baseNativeRequest(visual({ xAxisField: 'Regiao', yAxisFields: ['Clientes'] }), [f]).filters).toEqual([
      { tableId: 'fv', column: 'Cliente', values: ['c1'] },
    ]);
  });

  it('resposta: rótulos refeitos a partir das chaves (partes de data em PT-BR)', async () => {
    const { levelsFromNative, matrixFromNative } = await import('../engineQuery');
    const lv = levelsFromNative(
      { categories: ['2024 1'], categoryKeys: [[2024, 1]], series: [{ name: 'Total', data: [12] }], queryTimeMs: 1, engine: 'rust-native' },
      [datePart('year'), datePart('quarter')]
    );
    expect(lv.categories).toEqual(['2024 Trim 1']);
    const mx = matrixFromNative(
      {
        categories: ['2024'],
        series: [{ name: 'Total', data: [16] }],
        queryTimeMs: 1,
        engine: 'rust-native',
        matrix: {
          rows: [
            {
              key: 2024,
              label: '2024',
              values: [16],
              rowTotal: null,
              hasChildren: true,
              expanded: true,
              children: [{ key: 3, label: '3', values: [16], rowTotal: null, hasChildren: false, expanded: false, children: [] }],
            },
          ],
          columnTotals: [26],
          grandTotal: null,
        },
      },
      [datePart('year'), datePart('month')]
    );
    expect(mx.rows[0].label).toBe('2024');
    expect(mx.rows[0].children[0].label).toBe('março');
    expect(mx.columnTotals).toEqual([26]);
    expect(mx.seriesNames).toEqual(['Total']);
  });
});
