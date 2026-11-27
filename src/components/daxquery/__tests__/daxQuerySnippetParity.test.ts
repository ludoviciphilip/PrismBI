/**
 * Parity test for DAX Query View × TS engine (v0.11 Frente F, item 5 of the
 * task): runs DaxQueryView's own example — the `DEFINE`/`EVALUATE`/`ORDER BY`
 * snippet from `DAX_SNIPPETS` (`src/engine/shared/intellisense.ts`) that the
 * UI offers to users via "Inserir Snippet" — through the legacy TS engine
 * (`DaxEngine.executeDaxQuery`) against a tiny hand-built model, and checks
 * the result against values derived by hand (not copied from either
 * engine's output), matching `tests/dax-corpus/README.md`'s rule.
 *
 * This is the TS side of the parity check. The Rust side (same construct —
 * `DEFINE MEASURE` + `SUMMARIZECOLUMNS` + `ORDER BY`) is covered by
 * `src-tauri/src/dax/query_view/tests.rs`
 * (`summarizecolumns_groups_by_category_through_relationship`,
 * `define_measure_overrides_model_measure_only_for_this_query`), run against
 * an equally small model with the same arithmetic shape, since there's no
 * way to invoke the real Tauri `dax_query_execute` command from a plain
 * Vitest process (no Tauri runtime here) — see the final report.
 */
import { describe, it, expect } from 'vitest';
import { DaxEngine } from '../../../engine/daxEngine';
import { InMemoryModelAccess } from '../../../engine/model/modelAccess';
import { DAX_SNIPPETS } from '../../../engine/shared/intellisense';
import type { Table, Relationship, Measure } from '../../../types/bi';

/** Same daily calendar generator used by `dax_corpus.test.ts`, for Ano/Mes columns. */
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

function buildModel(): InMemoryModelAccess {
  const vendas: Table = {
    id: 'f_Vendas',
    name: 'f_Vendas',
    originalName: 'f_Vendas',
    columns: [
      { id: 'f_Vendas.Data', name: 'Data', originalName: 'Data', dataType: 'date' },
      { id: 'f_Vendas.Valor', name: 'Valor', originalName: 'Valor', dataType: 'number' },
    ],
    rows: [
      { Data: '2023-01-10', Valor: 100 },
      { Data: '2023-06-01', Valor: 50 },
      { Data: '2024-02-01', Valor: 200 },
    ],
  } as Table;
  const custos: Table = {
    id: 'f_Custos',
    name: 'f_Custos',
    originalName: 'f_Custos',
    columns: [
      { id: 'f_Custos.Data', name: 'Data', originalName: 'Data', dataType: 'date' },
      { id: 'f_Custos.Valor', name: 'Valor', originalName: 'Valor', dataType: 'number' },
    ],
    rows: [
      { Data: '2023-03-01', Valor: 30 },
      { Data: '2024-05-01', Valor: 50 },
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

  const relationships: Relationship[] = [
    { id: 'rel_1', fromTable: 'f_Vendas', fromColumn: 'Data', toTable: 'd_Calendario', toColumn: 'Data', cardinality: 'N:1', crossFilterDirection: 'single', isActive: true },
    { id: 'rel_2', fromTable: 'f_Custos', fromColumn: 'Data', toTable: 'd_Calendario', toColumn: 'Data', cardinality: 'N:1', crossFilterDirection: 'single', isActive: true },
  ];
  const measures: Measure[] = [
    { id: 'm_receita', name: 'Receita', tableName: 'f_Vendas', tableId: 'f_Vendas', formula: 'SUM(f_Vendas[Valor])', format: 'number' },
    { id: 'm_custo', name: 'Custo', tableName: 'f_Custos', tableId: 'f_Custos', formula: 'SUM(f_Custos[Valor])', format: 'number' },
  ];
  return new InMemoryModelAccess([vendas, custos, calendario], relationships, measures);
}

describe('DaxQueryView snippet parity (TS engine)', () => {
  it('DAX_SNIPPETS has exactly one full DEFINE/EVALUATE/ORDER BY query example', () => {
    const fullQueries = DAX_SNIPPETS.filter(s => /\bDEFINE\b/i.test(s.template) && /\bEVALUATE\b/i.test(s.template));
    expect(fullQueries.length).toBe(1);
  });

  it('runs the UI-offered DEFINE MEASURE + SUMMARIZECOLUMNS + ORDER BY snippet', () => {
    const snippet = DAX_SNIPPETS.find(s => /\bDEFINE\b/i.test(s.template) && /\bEVALUATE\b/i.test(s.template));
    expect(snippet).toBeDefined();

    const model = buildModel();
    const res = DaxEngine.executeDaxQuery(snippet!.template, model);

    expect(res.success).toBe(true);
    expect(res.error).toBeUndefined();
    expect(res.results.length).toBe(1);

    // Hand-derived: Receita 2023 = 100+50 = 150, Custo 2023 = 30 -> Margem 2023 = 120.
    // Receita 2024 = 200, Custo 2024 = 50 -> Margem 2024 = 150. ORDER BY Ano ASC.
    const rows = res.results[0].rows ?? [];
    expect(rows.length).toBe(2);
    const byYear = new Map(rows.map(r => [Number((r as any)['d_Calendario[Ano]'] ?? (r as any).Ano), (r as any).Margem]));
    expect(byYear.get(2023)).toBe(120);
    expect(byYear.get(2024)).toBe(150);
  });
});
