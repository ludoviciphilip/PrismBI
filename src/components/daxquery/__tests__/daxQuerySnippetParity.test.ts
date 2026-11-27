/**
 * v0.11 Rodada 2 (bugfix): every snippet the "Snippets DAX" button offers
 * must come out as a VALID, RUNNABLE DAX query against the CURRENT model,
 * like Power BI's own "quick measure" insertion -- not a static string
 * referencing example tables ('d_Calendario', [Medida], 'd_Produtos'...)
 * that don't exist in a real project (see `src/engine/shared/snippetResolver.ts`).
 *
 * This is the TS-engine side of the contract: build a small model with a
 * real fact table, calendar dimension and product dimension, resolve every
 * `DAX_SNIPPETS` entry against it with `resolveDaxSnippet`, and execute the
 * resulting query through `DaxEngine.executeDaxQuery` (same engine
 * `DaxQueryView`'s browser-mode fallback uses). The Rust side (same
 * resolved queries) is covered by `src-tauri/src/dax/query_view/tests_snippets.rs`.
 */
import { describe, it, expect } from 'vitest';
import { DaxEngine } from '../../../engine/daxEngine';
import { InMemoryModelAccess } from '../../../engine/model/modelAccess';
import { DAX_SNIPPETS } from '../../../engine/shared/intellisense';
import { buildModelParts, resolveDaxSnippet } from '../../../engine/shared/snippetResolver';
import type { Table, Relationship, Measure } from '../../../types/bi';

/** Same daily calendar generator used by `dax_corpus.test.ts`, for Ano. */
function generateCalendarRows(start: string, end: string): Record<string, unknown>[] {
  const rows: Record<string, unknown>[] = [];
  const d = new Date(`${start}T00:00:00Z`);
  const last = new Date(`${end}T00:00:00Z`);
  while (d <= last) {
    rows.push({ Data: d.toISOString().slice(0, 10), Ano: d.getUTCFullYear() });
    d.setUTCDate(d.getUTCDate() + 1);
  }
  return rows;
}

const vendas: Table = {
  id: 'f_Vendas',
  name: 'f_Vendas',
  originalName: 'f_Vendas',
  columns: [
    { id: 'f_Vendas.Data', name: 'Data', originalName: 'Data', dataType: 'date' },
    { id: 'f_Vendas.Produto', name: 'Produto', originalName: 'Produto', dataType: 'text' },
    { id: 'f_Vendas.Valor', name: 'Valor', originalName: 'Valor', dataType: 'number' },
  ],
  rows: [
    { Data: '2023-01-10', Produto: 'A', Valor: 100 },
    { Data: '2023-06-01', Produto: 'B', Valor: 50 },
    { Data: '2024-01-10', Produto: 'A', Valor: 200 },
    { Data: '2024-02-01', Produto: 'B', Valor: 20 },
    { Data: '2024-03-01', Produto: 'C', Valor: 5 },
  ],
} as Table;

const calendario: Table = {
  id: 'd_Calendario',
  name: 'd_Calendario',
  originalName: 'd_Calendario',
  columns: [
    { id: 'd_Calendario.Data', name: 'Data', originalName: 'Data', dataType: 'date' },
    { id: 'd_Calendario.Ano', name: 'Ano', originalName: 'Ano', dataType: 'number' },
  ],
  rows: generateCalendarRows('2023-01-01', '2024-12-31'),
} as Table;

const produtos: Table = {
  id: 'd_Produtos',
  name: 'd_Produtos',
  originalName: 'd_Produtos',
  columns: [{ id: 'd_Produtos.Produto', name: 'Produto', originalName: 'Produto', dataType: 'text' }],
  rows: [{ Produto: 'A' }, { Produto: 'B' }, { Produto: 'C' }],
} as Table;

const tables: Table[] = [vendas, calendario, produtos];
const relationships: Relationship[] = [
  { id: 'rel_1', fromTable: 'f_Vendas', fromColumn: 'Data', toTable: 'd_Calendario', toColumn: 'Data', cardinality: 'N:1', crossFilterDirection: 'single', isActive: true },
  { id: 'rel_2', fromTable: 'f_Vendas', fromColumn: 'Produto', toTable: 'd_Produtos', toColumn: 'Produto', cardinality: 'N:1', crossFilterDirection: 'single', isActive: true },
];
const measures: Measure[] = [{ id: 'm_receita', name: 'Receita', tableName: 'f_Vendas', tableId: 'f_Vendas', formula: 'SUM(f_Vendas[Valor])', format: 'number' }];

function buildModel(): InMemoryModelAccess {
  return new InMemoryModelAccess(tables, relationships, measures);
}

describe('DAX_SNIPPETS resolve against a real model and are runnable queries', () => {
  const parts = buildModelParts(tables, measures);

  it('resolves every snippet id as enabled against a model with fact + calendar + product dimension + 1 measure', () => {
    for (const snip of DAX_SNIPPETS) {
      const resolved = resolveDaxSnippet(snip.id, parts);
      expect(resolved.enabled, `${snip.id} disabled: ${resolved.disabledReason}`).toBe(true);
      expect(resolved.query).toMatch(/\bDEFINE\b/);
      expect(resolved.query).toMatch(/\bEVALUATE\b/);
    }
  });

  it.each(DAX_SNIPPETS.map(s => s.id))('runs %s through the TS engine without error and returns rows', (id) => {
    const resolved = resolveDaxSnippet(id, parts);
    const model = buildModel();
    const res = DaxEngine.executeDaxQuery(resolved.query, model);
    expect(res.success, `query failed: ${res.error}\n${resolved.query}`).toBe(true);
    expect(res.error).toBeUndefined();
    expect(res.results.length).toBe(1);
    expect(res.results[0]?.rows?.length ?? 0).toBeGreaterThan(0);
  });

  it('YoY growth matches the hand-derived value (2024 over 2023, Produto A+B+C)', () => {
    const resolved = resolveDaxSnippet('snippet_yoy', parts);
    const model = buildModel();
    const res = DaxEngine.executeDaxQuery(resolved.query, model);
    expect(res.success).toBe(true);
    const rows = res.results[0].rows ?? [];
    const byYear = new Map(rows.map((r: any) => [Number(r['d_Calendario[Ano]'] ?? r.Ano), r[resolved.measureName]]));
    // 2023 Receita = 100 + 50 = 150; 2024 Receita = 200 + 20 + 5 = 225.
    // YoY 2024 = (225 - 150) / 150 = 0.5. 2023 has no prior year -> SAMEPERIODLASTYEAR is blank -> DIVIDE's 0 default.
    expect(byYear.get(2024)).toBeCloseTo(0.5, 6);
  });

  it('YTD equals the running SUM within each year (same as the plain measure here, no partial-year filter)', () => {
    const resolved = resolveDaxSnippet('snippet_ytd', parts);
    const model = buildModel();
    const res = DaxEngine.executeDaxQuery(resolved.query, model);
    expect(res.success).toBe(true);
    const rows = res.results[0].rows ?? [];
    const byYear = new Map(rows.map((r: any) => [Number(r['d_Calendario[Ano]'] ?? r.Ano), r[resolved.measureName]]));
    expect(byYear.get(2023)).toBe(150);
    expect(byYear.get(2024)).toBe(225);
  });

  it('Pareto accumulated % sums to 100% at the top-ranked product', () => {
    const resolved = resolveDaxSnippet('snippet_pareto', parts);
    const model = buildModel();
    const res = DaxEngine.executeDaxQuery(resolved.query, model);
    expect(res.success).toBe(true);
    const rows = res.results[0].rows ?? [];
    // Produto A = 300 (biggest), total = 300+70+5 = 375 -> A alone already >= A's own value,
    // so Acumulado for A is just A's 300 / 375.
    const byProduto = new Map(rows.map((r: any) => [r['d_Produtos[Produto]'] ?? r.Produto, r[resolved.measureName]]));
    expect(byProduto.get('A')).toBeCloseTo(300 / 375, 6);
    // The overall max accumulated % (lowest-ranked product) must reach 100%.
    expect(Math.max(...Array.from(byProduto.values() as Iterable<number>))).toBeCloseTo(1, 6);
  });
});
