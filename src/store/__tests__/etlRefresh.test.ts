import { describe, it, expect, beforeEach, vi } from 'vitest';

// Bug do teste no desktop: depois de editar etapas e aplicar, "Atualizar"
// ficava em "Atualizando..." para sempre e recarregava as tabelas pelo M
// original (descartando as etapas). Aqui o backend é simulado.
const loadRaw = vi.fn();
const apply = vi.fn();
vi.mock('../../services/etl/etlClient', async importOriginal => {
  const actual = await importOriginal<typeof import('../../services/etl/etlClient')>();
  return { ...actual, etlLoadRaw: (...a: unknown[]) => loadRaw(...a), etlApply: (...a: unknown[]) => apply(...a) };
});
const refreshNative = vi.fn();
const refreshPbip = vi.fn();
const syncModel = vi.fn();
vi.mock('../../bridge/tauriBridge', async importOriginal => {
  const actual = await importOriginal<typeof import('../../bridge/tauriBridge')>();
  return {
    ...actual,
    isTauriRuntime: () => true,
    refreshTableNative: (...a: unknown[]) => refreshNative(...a),
    refreshPbipTables: (...a: unknown[]) => refreshPbip(...a),
    syncModelToRust: (...a: unknown[]) => syncModel(...a),
  };
});

const { useBiStore } = await import('../useBiStore');

function setup() {
  useBiStore.setState({
    tables: [
      {
        id: 't_cal',
        name: 'dCalendario',
        originalName: 'dCalendario',
        columns: [{ id: 'c_data', name: 'Data', originalName: 'Data', dataType: 'date' }],
        rows: [{ Data: '2026-04-01' }],
        sourceConfig: { type: 'pbip', mScript: 'let ... in x', pbipRoot: '/proj' },
      },
      {
        id: 't_cat',
        name: 'dCategorias',
        originalName: 'dCategorias',
        columns: [{ id: 'c_cat', name: 'Categoria', originalName: 'Categoria', dataType: 'text' }],
        rows: [{ Categoria: 'A' }],
        sourceConfig: { type: 'xlsx', filePath: '/proj/r.xlsx', sheetName: 'Categorias', mScript: 'let ... in y', pbipRoot: '/proj' },
      },
    ] as any,
    appliedSteps: {
      t_cal: {
        name: 'dCalendario',
        source: { kind: 'listDates', start: '2026-04-01', count: 2, stepDays: 1 },
        steps: [{ id: 's1', name: 'Renamed Columns', op: 'renameColumns', renames: [['Column1', 'Data']] }],
      },
    },
    sourcePbipPath: '/proj/Rhino.pbip',
    isRefreshingData: false,
  } as any);
}

describe('refresh with Power Query queries', () => {
  beforeEach(() => {
    [loadRaw, apply, refreshNative, refreshPbip, syncModel].forEach(f => f.mockReset());
    loadRaw.mockResolvedValue({ rawTable: 'raw__dCalendario', columns: [], rowCount: 2 });
    apply.mockResolvedValue({
      columns: [{ name: 'Data', originalName: 'Data', dataType: 'date' }],
      rows: [{ Data: '2026-04-01' }, { Data: '2026-04-02' }],
    });
    refreshNative.mockResolvedValue({ tableName: 'dCategorias', originalName: 'dCategorias', columns: [{ name: 'Categoria', originalName: 'Categoria', dataType: 'text' }], rows: [{ Categoria: 'B' }], totalRows: 1 });
    // O .pbip devolve a dCalendario ORIGINAL (todas as colunas): não pode sobrescrever a consulta editada.
    refreshPbip.mockResolvedValue({
      warnings: [],
      loadedTables: [{ tableName: 'dCalendario', originalName: 'dCalendario', columns: [{ name: 'Ano', originalName: 'Ano', dataType: 'number' }], rows: [{ Ano: 2026 }], totalRows: 1 }],
    });
    syncModel.mockResolvedValue(2);
    setup();
  });

  it('refreshAllData re-runs ETL queries (raw + steps) and never reloads them from the original M', async () => {
    const res = await useBiStore.getState().refreshAllData();
    expect(loadRaw).toHaveBeenCalledWith({
      query: 'dCalendario',
      source: { kind: 'listDates', start: '2026-04-01', count: 2, stepDays: 1 },
      pbipRoot: '/proj',
    });
    expect(apply).toHaveBeenCalledTimes(1);
    // A tabela sem consulta segue pelo caminho antigo; a com consulta, não.
    expect(refreshNative).toHaveBeenCalledTimes(1);
    expect(refreshNative.mock.calls[0][0]).toMatchObject({ tableName: 'dCategorias' });
    const cal = useBiStore.getState().tables.find(t => t.id === 't_cal')!;
    expect(cal.columns.map(c => c.name)).toEqual(['Data']);
    expect(cal.rows).toHaveLength(2);
    expect(res.refreshedTables).toEqual(expect.arrayContaining(['dCalendario', 'dCategorias']));
    expect(useBiStore.getState().isRefreshingData).toBe(false);
  });

  it('a backend call that never answers ends with a warning instead of "Atualizando..." forever', async () => {
    vi.useFakeTimers();
    try {
      syncModel.mockReturnValue(new Promise(() => {}));
      const p = useBiStore.getState().refreshAllData();
      await vi.advanceTimersByTimeAsync(121_000);
      const res = await p;
      expect(res.error || res.warnings.join(' ')).toMatch(/sem resposta/);
      expect(useBiStore.getState().isRefreshingData).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });
});
