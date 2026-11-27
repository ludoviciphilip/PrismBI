import { describe, it, expect } from 'vitest';
import { InMemoryModelAccess } from '../../../../engine/model/modelAccess';
import { Measure, Table, Visual } from '../../../../types/bi';
import { QueryPlanner } from '../../../../engine/query/planner';

/**
 * v0.11 Rodada 2, Frente E: differential (parity) test.
 *
 * Same fixture as `src-tauri/src/dax/query_planner.rs` `sample_model()` and
 * its `test_native_table_query_grouped_sort_total_over_all_rows` /
 * `test_native_scatter_query_points_per_detail_value` tests. Both sides are
 * hand-derived (never copied from either engine's own output):
 *   fVendas: Categoria=A,Valor=100 / Categoria=A,Valor=50 / Categoria=B,Valor=200
 *   Receita = SUM(fVendas[Valor])   Qtd = COUNTROWS(fVendas)
 *   -> group by Categoria: A = 100+50 = 150, B = 200; total = 350
 *
 * This proves the TS legacy path (QueryPlanner, used by the browser-only
 * fallback) and the new Rust native path (query_table_native /
 * query_scatter_native, see query_planner.rs) agree on the shared
 * aggregation + total semantics, even though pagination/sort/Top N and the
 * engine-computed total row are Rust-only additions (no 5.000-row preview
 * cap, no client-side IPC of the whole table) with no TS equivalent to
 * diff against — see docs/V011.md Rodada 2, Frente E item 5.
 *
 * NOTE: this is a static cross-engine fixture (both sides hand-derived from
 * the same model), not a live Tauri round-trip — vitest has no Tauri
 * runtime to invoke `query_table_native` from. The Rust side of the same
 * scenario is asserted independently in query_planner.rs's own test.
 */
const fVendas: Table = {
  id: 'fVendas',
  name: 'fVendas',
  originalName: 'fVendas',
  columns: ['Categoria', 'Valor'].map((n, i) => ({
    id: `c${i}`,
    name: n,
    originalName: n,
    dataType: n === 'Valor' ? 'number' : 'text',
  })),
  rows: [
    { Categoria: 'A', Valor: 100 },
    { Categoria: 'A', Valor: 50 },
    { Categoria: 'B', Valor: 200 },
  ],
} as Table;

const measures: Measure[] = [
  { id: 'm1', name: 'Receita', formula: 'SUM(fVendas[Valor])', tableName: 'fVendas' } as Measure,
  { id: 'm2', name: 'Qtd', formula: 'COUNTROWS(fVendas)', tableName: 'fVendas' } as Measure,
];

const model = new InMemoryModelAccess([fVendas], [], measures);

const tableVisual: Visual = {
  id: 'tbl1',
  type: 'table',
  title: 'Tabela',
  x: 0,
  y: 0,
  w: 400,
  h: 300,
  dataConfig: {
    tableId: 'fVendas',
    xAxisField: 'Categoria',
    yAxisFields: ['Receita'],
    aggregation: 'SUM',
  },
} as Visual;

describe('parity (Frente E): table grouping + total — TS legacy vs Rust fixture', () => {
  it('groups by Categoria and sums Receita exactly like the Rust native table query', () => {
    const planner = new QueryPlanner(model);
    const result = planner.query(tableVisual, []);

    // Hand-derived expectation, identical to the Rust test's assertions.
    const byCategory = Object.fromEntries(
      result.categories.map((c, i) => [c, result.series[0].data[i]])
    );
    expect(byCategory['A']).toBe(150); // 100 + 50
    expect(byCategory['B']).toBe(200);

    // Total is the SAME closed-form value the Rust engine computes in the
    // TOTAL filter context (no group filter) — 350, not a client sum of a
    // paginated page (which the TS path, lacking pagination, can't even
    // produce a wrong answer for here, but Rust's total_row must match).
    const grandTotal = result.series[0].data.reduce((a: number, b) => a + (b ?? 0), 0);
    expect(grandTotal).toBe(350);
  });
});
