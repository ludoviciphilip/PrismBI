import { describe, it, expect } from 'vitest';
import {
  applyVisualSort,
  buildSortByLookup,
  compareSortValues,
  fieldMatches,
  sortCategoryLabels,
  sortRows,
  sortValueList,
  VisualSortSpec,
} from '../visualSort';
import { DataEngine } from '../dataEngine';
import { Visual, Table } from '../../types/bi';

const baseSpec = (over: Partial<VisualSortSpec> = {}): VisualSortSpec => ({
  categoryFields: ['Categoria'],
  valueFields: ['Valor'],
  ...over,
});

describe('visualSort — compareSortValues', () => {
  it('sorts numbers numerically, also when they arrive as strings', () => {
    expect(sortCategoryLabels(['10', '9', '100', '2'])).toEqual(['2', '9', '10', '100']);
    expect([10, 9, 100, 2].sort((a, b) => compareSortValues(a, b))).toEqual([2, 9, 10, 100]);
    expect(sortCategoryLabels(['-1.5', '3', '0'], 'desc')).toEqual(['3', '0', '-1.5']);
  });

  it('sorts dates chronologically (ISO, dd/mm/yyyy and Date objects)', () => {
    expect(sortCategoryLabels(['2025-02-01', '2024-12-31', '2025-01-15'])).toEqual([
      '2024-12-31',
      '2025-01-15',
      '2025-02-01',
    ]);
    // As strings "01/02/2025" < "31/12/2024" — chronologically it's the opposite.
    expect(sortCategoryLabels(['01/02/2025', '31/12/2024'])).toEqual(['31/12/2024', '01/02/2025']);
    const d = [new Date('2025-03-01'), new Date('2024-01-01')];
    expect(d.sort((a, b) => compareSortValues(a, b)).map(x => x.getUTCFullYear())).toEqual([2024, 2025]);
  });

  it('sorts text with pt-BR collation (accents next to their base letter)', () => {
    expect(sortCategoryLabels(['Ótica', 'Zebra', 'avião', 'Água', 'Banana', 'Ovo'])).toEqual([
      'Água',
      'avião',
      'Banana',
      'Ótica',
      'Ovo',
      'Zebra',
    ].sort((a, b) => a.localeCompare(b, 'pt-BR')));
    // Accented capital must not end up after "Z" (plain code-point sort would do that).
    const sorted = sortCategoryLabels(['Zé', 'Érica', 'Ana']);
    expect(sorted).toEqual(['Ana', 'Érica', 'Zé']);
  });

  it('puts blanks last in ascending AND descending order', () => {
    const vals = [3, null, 1, '', undefined, 2, NaN];
    const asc = [...vals].sort((a, b) => compareSortValues(a, b, 'asc'));
    expect(asc.slice(0, 3)).toEqual([1, 2, 3]);
    const desc = [...vals].sort((a, b) => compareSortValues(a, b, 'desc'));
    expect(desc.slice(0, 3)).toEqual([3, 2, 1]);
    expect(desc.slice(3).every(v => v === null || v === undefined || v === '' || Number.isNaN(v))).toBe(true);
  });

  it('matches qualified / Σ-prefixed field names', () => {
    expect(fieldMatches('f_Vendas.Categoria', 'Categoria')).toBe(true);
    expect(fieldMatches('Σ Valor', 'valor')).toBe(true);
    expect(fieldMatches('Vendas[Valor]', 'Valor')).toBe(true);
    expect(fieldMatches('Valor', 'Categoria')).toBe(false);
  });
});

describe('visualSort — applyVisualSort', () => {
  const data = {
    categories: ['B', 'C', 'A', 'D'],
    series: [
      { name: 'Valor', data: [20, 5, 50, 10] },
      { name: 'Qtd', data: [1, 4, 2, 3] },
    ],
  };

  it('sorts by category ascending/descending and keeps series aligned', () => {
    const asc = applyVisualSort(data, baseSpec({ sortField: 'Categoria', sortOrder: 'asc' }));
    expect(asc.categories).toEqual(['A', 'B', 'C', 'D']);
    expect(asc.series[0].data).toEqual([50, 20, 5, 10]);
    const desc = applyVisualSort(data, baseSpec({ sortField: 'Categoria', sortOrder: 'desc' }));
    expect(desc.categories).toEqual(['D', 'C', 'B', 'A']);
    expect(desc.series[1].data).toEqual([3, 4, 1, 2]);
  });

  it('sorts by a measure / value series', () => {
    const byValor = applyVisualSort(data, baseSpec({ sortField: 'Valor', sortOrder: 'desc' }));
    expect(byValor.categories).toEqual(['A', 'B', 'D', 'C']);
    const byQtd = applyVisualSort(data, baseSpec({ sortField: 'Qtd', sortOrder: 'asc' }));
    expect(byQtd.categories).toEqual(['B', 'A', 'D', 'C']);
  });

  it('sorts by row total when the value field is split by a legend', () => {
    const legendData = {
      categories: ['X', 'Y'],
      series: [
        { name: 'Norte', data: [1, 10] },
        { name: 'Sul', data: [5, 1] },
      ],
    };
    const res = applyVisualSort(legendData, baseSpec({ sortField: 'Valor', sortOrder: 'desc' }));
    expect(res.categories).toEqual(['Y', 'X']);
  });

  it('honours sortByColumn indirection for the category axis', () => {
    const rows = [
      { NomeMes: 'Janeiro', Mes: 1 },
      { NomeMes: 'Fevereiro', Mes: 2 },
      { NomeMes: 'Março', Mes: 3 },
      { NomeMes: 'Dezembro', Mes: 12 },
    ];
    const lookup = buildSortByLookup(rows, 'NomeMes', 'Mes');
    const monthData = {
      categories: ['Março', 'Dezembro', 'Janeiro', 'Fevereiro'],
      series: [{ name: 'Valor', data: [3, 12, 1, 2] }],
    };
    const spec = baseSpec({ categoryFields: ['NomeMes'], categorySortKeys: lookup });
    expect(applyVisualSort(monthData, { ...spec, sortField: 'NomeMes', sortOrder: 'asc' }).categories).toEqual([
      'Janeiro',
      'Fevereiro',
      'Março',
      'Dezembro',
    ]);
    expect(applyVisualSort(monthData, { ...spec, sortField: 'NomeMes', sortOrder: 'desc' }).categories[0]).toBe('Dezembro');
    // No sortField -> default axis order still honours sortByColumn.
    expect(applyVisualSort(monthData, spec).categories).toEqual(['Janeiro', 'Fevereiro', 'Março', 'Dezembro']);
    // sortOrder without sortField sorts the axis in that direction.
    expect(applyVisualSort(monthData, { ...spec, sortOrder: 'desc' }).categories[0]).toBe('Dezembro');
  });

  it('applies the TopN limit AFTER sorting', () => {
    const res = applyVisualSort(data, baseSpec({ sortField: 'Categoria', sortOrder: 'asc', limit: 2 }));
    expect(res.categories).toEqual(['A', 'B']);
    const byVal = applyVisualSort(data, baseSpec({ sortField: 'Valor', sortOrder: 'asc', limit: 2 }));
    expect(byVal.categories).toEqual(['C', 'D']);
    // Limit without sortField keeps the largest values (TopN by first series).
    const topN = applyVisualSort(data, baseSpec({ limit: 2 }));
    expect(topN.categories).toEqual(['A', 'B']);
    expect(topN.series[0].data).toEqual([50, 20]);
  });
});

describe('visualSort — rows and value lists', () => {
  it('sortRows sorts detail rows by column, numbers numerically, blanks last', () => {
    const rows = [
      { Nome: 'b', Valor: 10 },
      { Nome: 'a', Valor: null },
      { Nome: 'c', Valor: 9 },
    ];
    expect(sortRows(rows, 'Valor', 'asc').map(r => r.Nome)).toEqual(['c', 'b', 'a']);
    expect(sortRows(rows, 'Valor', 'desc').map(r => r.Nome)).toEqual(['b', 'c', 'a']);
    expect(sortRows(rows, 'Nome', 'desc').map(r => r.Nome)).toEqual(['c', 'b', 'a']);
  });

  it('sortRows honours sortByColumn', () => {
    const rows = [
      { NomeMes: 'Março', Mes: 3 },
      { NomeMes: 'Abril', Mes: 4 },
      { NomeMes: 'Janeiro', Mes: 1 },
    ];
    expect(sortRows(rows, 'NomeMes', 'asc', { sortByColumn: 'Mes' }).map(r => r.NomeMes)).toEqual([
      'Janeiro',
      'Março',
      'Abril',
    ]);
  });

  it('sortValueList supports A-Z, Z-A and by count', () => {
    const items = [
      { value: 'Loja', count: 5 },
      { value: 'Online', count: 12 },
      { value: 'Atacado', count: 5 },
      { value: null, count: 1 },
    ];
    expect(sortValueList(items, 'asc').map(i => i.value)).toEqual(['Atacado', 'Loja', 'Online', null]);
    expect(sortValueList(items, 'desc').map(i => i.value)).toEqual(['Online', 'Loja', 'Atacado', null]);
    expect(sortValueList(items, 'count').map(i => i.value)).toEqual(['Online', 'Atacado', 'Loja', null]);
  });
});

describe('visualSort — engine integration', () => {
  const table: Table = {
    id: 't1',
    name: 'Vendas',
    originalName: 'Vendas',
    columns: [
      { id: 'c1', name: 'NomeMes', originalName: 'NomeMes', dataType: 'text', sortByColumn: 'Mes' },
      { id: 'c2', name: 'Mes', originalName: 'Mes', dataType: 'number' },
      { id: 'c3', name: 'Valor', originalName: 'Valor', dataType: 'number' },
    ],
    rows: [
      { NomeMes: 'Março', Mes: 3, Valor: 30 },
      { NomeMes: 'Janeiro', Mes: 1, Valor: 100 },
      { NomeMes: 'Fevereiro', Mes: 2, Valor: 5 },
      { NomeMes: 'Janeiro', Mes: 1, Valor: 1 },
    ],
  } as Table;

  const mkVisual = (dc: Partial<Visual['dataConfig']>, type: Visual['type'] = 'column'): Visual => ({
    id: `v_${type}_${JSON.stringify(dc)}`,
    type,
    title: 'v',
    x: 0,
    y: 0,
    w: 100,
    h: 100,
    dataConfig: { tableId: 't1', xAxisField: 'NomeMes', yAxisFields: ['Valor'], aggregation: 'SUM', ...dc },
    formatConfig: { showTitle: true },
  });

  it('JS planner and a native-shaped result sort identically through the shared spec', () => {
    const engine = new DataEngine();
    engine.setSchema([table], [], []);
    for (const dc of [
      { sortField: 'Valor', sortOrder: 'desc' as const },
      { sortField: 'NomeMes', sortOrder: 'asc' as const },
      { sortField: 'NomeMes', sortOrder: 'desc' as const, limit: 2 },
    ]) {
      const v = mkVisual(dc);
      const js = engine.queryVisualData(v, []);
      // Native result arrives in arbitrary (e.g. lexicographic) order and without limit.
      const native = {
        categories: ['Fevereiro', 'Janeiro', 'Março'],
        series: [{ name: 'Valor', data: [5, 101, 30] }],
      };
      const spec = engine.getVisualSortSpec(v)!;
      const nativeSorted = applyVisualSort(native, spec);
      expect(nativeSorted.categories).toEqual(js.categories);
      expect(nativeSorted.series[0].data).toEqual(js.series[0].data);
    }
    expect(engine.queryVisualData(mkVisual({ sortField: 'NomeMes', sortOrder: 'asc' }), []).categories).toEqual([
      'Janeiro',
      'Fevereiro',
      'Março',
    ]);
  });

  it('sorts slicer items and table rows', () => {
    const engine = new DataEngine();
    engine.setSchema([table], [], []);
    const slicer = engine.queryVisualData(mkVisual({ sortOrder: 'desc' }, 'slicer'), []);
    expect(slicer.categories).toEqual(['Março', 'Fevereiro', 'Janeiro']);
    const tbl = engine.queryVisualData(mkVisual({ sortField: 'Valor', sortOrder: 'desc' }, 'table'), []);
    expect(tbl.rawRows.map(r => r.Valor)).toEqual([100, 30, 5, 1]);
  });

  it('orders matrix/legend columns by sortByColumn and drops all-blank SUMX rows', () => {
    const cal: Table = {
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
    const est: Table = {
      id: 'de',
      name: 'dEstrutura',
      originalName: 'dEstrutura',
      columns: [
        { id: 'e1', name: 'Linha', originalName: 'Linha', dataType: 'text', sortByColumn: 'Ordem' },
        { id: 'e2', name: 'Ordem', originalName: 'Ordem', dataType: 'number' },
      ],
      rows: [
        { Linha: 'Receita', Ordem: 1 },
        { Linha: '—', Ordem: 2 },
      ],
    } as Table;
    const fact: Table = {
      id: 'fl',
      name: 'fLanc',
      originalName: 'fLanc',
      columns: [
        { id: 'l1', name: 'Data', originalName: 'Data', dataType: 'date' },
        { id: 'l2', name: 'Linha', originalName: 'Linha', dataType: 'text' },
        { id: 'l3', name: 'Valor', originalName: 'Valor', dataType: 'number' },
      ],
      rows: [
        { Data: '2026-01-10', Linha: 'Receita', Valor: 100 },
        { Data: '2026-11-10', Linha: 'Receita', Valor: 250 },
      ],
    } as Table;
    const engine = new DataEngine();
    engine.setSchema(
      [cal, est, fact],
      [
        { id: 'r1', fromTable: 'fLanc', fromColumn: 'Data', toTable: 'dCal', toColumn: 'Data', cardinality: 'N:1', crossFilterDirection: 'single', isActive: true },
        { id: 'r2', fromTable: 'fLanc', fromColumn: 'Linha', toTable: 'dEstrutura', toColumn: 'Linha', cardinality: 'N:1', crossFilterDirection: 'single', isActive: true },
      ],
      [{ id: 'm1', name: 'DRE Valor', formula: '-SUMX(FILTER(fLanc, fLanc[Valor] >= 0), fLanc[Valor])', tableName: 'fLanc' } as any]
    );
    const v = mkVisual({ tableId: 'fl', xAxisField: 'dEstrutura.Linha', legendField: 'dCal.MesNome', yAxisFields: ['DRE Valor'], aggregation: 'NONE' }, 'matrix');
    const res = engine.queryVisualData(v, []);
    expect(res.categories).toEqual(['Receita']); // "—" row omitted completely
    expect(res.series.map(s => s.name)).toEqual(['jan', 'nov']); // sorted by MesNum, abr had only BLANK on Receita
    expect(res.series[0].data).toEqual([-100]);
    expect(res.series[1].data).toEqual([-250]);

    // Simulated native-unordered response also gets legend columns sorted via buildSortSpec
    const spec = engine.getVisualSortSpec(v)!;
    const sortedNative = applyVisualSort(
      {
        categories: ['Receita'],
        series: [
          { name: 'nov', data: [-250] },
          { name: 'abr', data: [null] },
          { name: 'jan', data: [-100] },
        ],
      },
      spec
    );
    expect(sortedNative.series.map(s => s.name)).toEqual(['jan', 'abr', 'nov']);
    expect(sortedNative.series.map(s => s.data)).toEqual([[-100], [null], [-250]]);
  });
});
