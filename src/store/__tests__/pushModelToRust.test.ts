import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('../../bridge/tauriBridge', async () => {
  const actual = await vi.importActual<any>('../../bridge/tauriBridge');
  return {
    ...actual,
    isTauriRuntime: vi.fn(() => true),
    syncModelToRust: vi.fn(async () => 0),
  };
});

import { useBiStore } from '../useBiStore';
import { syncModelToRust } from '../../bridge/tauriBridge';

describe('pushModelToRustAsync payload shape (duckdbTable vs rows)', () => {
  beforeEach(() => {
    vi.mocked(syncModelToRust).mockClear();
    useBiStore.setState({ tables: [], relationships: [], measures: [] } as any);
  });

  it('sends duckdbTable (Rust reads DuckDB) plus the preview rows as fallback; full rows for the rest', async () => {
    useBiStore.getState().setTables([
      {
        id: 'fVendas',
        name: 'fVendas',
        originalName: 'fVendas',
        columns: [{ id: 'c1', name: 'Valor', originalName: 'Valor', dataType: 'number' }],
        rows: [{ Valor: 1 }, { Valor: 2 }], // UI preview only, capped at 5000 upstream
        duckdbTable: 'fVendas',
        rowsTruncated: true,
        totalRows: 20000,
      } as any,
      {
        id: 'sampleManual',
        name: 'sampleManual',
        originalName: 'sampleManual',
        columns: [{ id: 'c2', name: 'Nome', originalName: 'Nome', dataType: 'text' }],
        rows: [{ Nome: 'a' }, { Nome: 'b' }],
        // no duckdbTable: manual/sample table, rows ARE the whole table
      } as any,
    ]);

    await vi.waitFor(() => expect(syncModelToRust).toHaveBeenCalled());

    const payload = vi.mocked(syncModelToRust).mock.calls.at(-1)![0] as any;
    const fVendas = payload.tables.find((t: any) => t.name === 'fVendas');
    const sampleManual = payload.tables.find((t: any) => t.name === 'sampleManual');

    expect(fVendas.duckdbTable).toBe('fVendas');
    // Rust reads all rows from DuckDB; the preview only matters if the DuckDB
    // table is missing (reopened project). [] would leave the table empty.
    expect(fVendas.rows).toEqual([{ Valor: 1 }, { Valor: 2 }]);

    expect(sampleManual.duckdbTable).toBeUndefined();
    expect(sampleManual.rows).toEqual([{ Nome: 'a' }, { Nome: 'b' }]);
  });
});

describe('reopening a project with truncated DuckDB-backed tables', () => {
  it('reloads the sources in the background so the model gets every row again', async () => {
    const refreshAllData = vi.fn(async () => ({ refreshedTables: [], warnings: [] }));
    useBiStore.setState({ refreshAllData } as any);
    const state = useBiStore.getState().getProjectState();
    useBiStore.getState().loadStateFromProject({
      ...state,
      tables: [
        {
          id: 't_big',
          name: 'fVendas',
          originalName: 'fVendas',
          columns: [{ id: 'c1', name: 'Valor', originalName: 'Valor', dataType: 'number' }],
          rows: [{ Valor: 1 }],
          duckdbTable: 'fVendas',
          rowsTruncated: true,
        } as any,
      ],
    });
    await new Promise(r => setTimeout(r, 0));
    expect(refreshAllData).toHaveBeenCalledTimes(1);
  });
});

// v0.11 Rodada 2 Frente F (bugfix): the real sync path -- the payload
// pushModelToRustAsync actually builds from the store after a .pbip import
// (Column.format / Measure.formatString, as PowerBiImportModal.tsx sets
// them) -- must carry those TMDL format strings to Rust. The Rust fixture
// (`DaxTableInput::column_format_strings`) only exercised a hand-built
// payload, which is why this gap (the frontend never wired it) went
// unnoticed: that test's fixture already had the field filled in.
describe('pushModelToRustAsync carries .pbip-imported formatStrings (real sync path)', () => {
  beforeEach(() => {
    vi.mocked(syncModelToRust).mockClear();
    useBiStore.setState({ tables: [], relationships: [], measures: [] } as any);
  });

  it('sends Column.format as columnFormatStrings, keyed by column name', async () => {
    useBiStore.getState().setTables([
      {
        id: 'dCalendario',
        name: 'dCalendario',
        originalName: 'dCalendario',
        columns: [
          { id: 'c1', name: 'Data', originalName: 'Data', dataType: 'date' },
          { id: 'c2', name: 'Ano', originalName: 'Ano', dataType: 'number', format: '0' },
        ],
        rows: [{ Data: '2024-01-01', Ano: 2024 }],
      } as any,
    ]);

    await vi.waitFor(() => expect(syncModelToRust).toHaveBeenCalled());

    const payload = vi.mocked(syncModelToRust).mock.calls.at(-1)![0] as any;
    const dCalendario = payload.tables.find((t: any) => t.name === 'dCalendario');
    expect(dCalendario.columnFormatStrings).toEqual({ Ano: '0' });
  });

  it('sends Measure.formatString, falling back to a pattern derived from Measure.format', async () => {
    useBiStore.getState().addMeasure({
      id: 'm1',
      name: 'Faturamento',
      tableName: 'fFato',
      formula: 'SUM(fFato[Valor])',
      format: 'currency',
      formatString: '#,##0.00',
    } as any);
    useBiStore.getState().addMeasure({
      id: 'm2',
      name: 'Quantidade',
      tableName: 'fFato',
      formula: 'COUNTROWS(fFato)',
      format: 'integer',
      // no TMDL formatString -- manually created measure
    } as any);

    await vi.waitFor(() => expect(syncModelToRust).toHaveBeenCalled());

    const payload = vi.mocked(syncModelToRust).mock.calls.at(-1)![0] as any;
    const faturamento = payload.measures.find((m: any) => m.name === 'Faturamento');
    const quantidade = payload.measures.find((m: any) => m.name === 'Quantidade');
    expect(faturamento.formatString).toBe('#,##0.00');
    expect(quantidade.formatString).toBe('#,##0');
  });
});
