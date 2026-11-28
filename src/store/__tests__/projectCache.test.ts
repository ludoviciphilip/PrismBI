import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('../../bridge/tauriBridge', async () => {
  const actual = await vi.importActual<any>('../../bridge/tauriBridge');
  return {
    ...actual,
    isTauriRuntime: vi.fn(() => true),
    projectCacheSave: vi.fn(),
    projectCacheLoad: vi.fn(),
    syncModelToRust: vi.fn(async () => 0),
  };
});

import { useBiStore } from '../useBiStore';
import { isTauriRuntime, projectCacheLoad } from '../../bridge/tauriBridge';
import type { ProjectState } from '../../engine/storage';

function baseProjectState(overrides: Partial<ProjectState> = {}): ProjectState {
  return {
    version: '2.2.0',
    projectId: 'proj-abc-123',
    projectName: 'Projeto Teste',
    tables: [
      {
        id: 'f_vendas',
        name: 'f_vendas',
        originalName: 'f_vendas',
        columns: [
          { id: 'c_id', name: 'id', originalName: 'id', dataType: 'number' },
          { id: 'c_val', name: 'val', originalName: 'val', dataType: 'number' },
        ],
        rows: [{ id: 1, val: 10 }],
        duckdbTable: 'f_vendas',
        rowsTruncated: true,
        totalRows: 10_000_000,
      } as any,
    ],
    relationships: [],
    measures: [],
    pages: [{ id: 'page_executivo', name: 'Página 1', visuals: [] } as any],
    activePageId: 'page_executivo',
    filters: [],
    bookmarks: [],
    dashboardTiles: [],
    goals: [],
    rlsRoles: [],
    whatIfParameters: [],
    theme: 'powerbi-light',
    updatedAt: new Date().toISOString(),
    ...overrides,
  };
}

describe('useBiStore.loadStateFromProject — cache colunar nativo (v0.11 Frente B)', () => {
  beforeEach(() => {
    useBiStore.setState({ refreshStatus: null } as any);
    vi.mocked(isTauriRuntime).mockReturnValue(true);
    vi.mocked(projectCacheLoad).mockReset();
    // Evita que `refreshAllData` real (que bate em fontes de arquivo/DB)
    // rode durante o teste quando o cache for inválido.
    useBiStore.setState({ refreshAllData: vi.fn(async () => ({ refreshedTables: [], warnings: [] })) } as any);
  });

  it('com cache válido: carrega do cache, NÃO chama refreshAllData, e mostra o aviso', async () => {
    vi.mocked(projectCacheLoad).mockResolvedValue({
      loaded: true,
      reason: null,
      tables: [
        {
          tableName: 'f_vendas',
          originalName: 'f_vendas',
          columns: [],
          rows: [],
          totalRows: 10_000_000,
          truncated: false,
        } as any,
      ],
      totalRows: 10_000_000,
      elapsedMs: 320,
    });

    const state = baseProjectState();
    useBiStore.getState().loadStateFromProject(state);

    // A chamada ao cache é assíncrona (fire-and-forget); espera resolver.
    await new Promise(resolve => setTimeout(resolve, 0));
    await new Promise(resolve => setTimeout(resolve, 0));

    expect(projectCacheLoad).toHaveBeenCalledTimes(1);
    const [calledProjectKey, calledMetas] = vi.mocked(projectCacheLoad).mock.calls[0];
    expect(calledProjectKey).toBe('proj-abc-123');
    expect(calledMetas).toEqual([
      expect.objectContaining({ name: 'f_vendas', rowCount: 10_000_000 }),
    ]);

    expect(useBiStore.getState().refreshAllData).not.toHaveBeenCalled();
    const table = useBiStore.getState().tables.find(t => t.id === 'f_vendas');
    expect(table?.rowsTruncated).toBe(false);
    expect(useBiStore.getState().refreshStatus?.message).toContain('10.000.000 linhas');
  });

  it('sem cache válido (ausente/incompatível): cai para refreshAllData, como hoje', async () => {
    vi.mocked(projectCacheLoad).mockResolvedValue({
      loaded: false,
      reason: 'cache inexistente',
      tables: [],
      totalRows: 0,
      elapsedMs: 5,
    });

    const state = baseProjectState();
    useBiStore.getState().loadStateFromProject(state);

    await new Promise(resolve => setTimeout(resolve, 0));
    await new Promise(resolve => setTimeout(resolve, 0));

    expect(projectCacheLoad).toHaveBeenCalledTimes(1);
    expect(useBiStore.getState().refreshAllData).toHaveBeenCalledTimes(1);
    // Sem cache, o aviso de cache local não deve aparecer.
    expect(useBiStore.getState().refreshStatus?.message || '').not.toContain('cache local');
  });

  it('fora do Tauri (browser): não chama project_cache_load nem refreshAllData', async () => {
    vi.mocked(isTauriRuntime).mockReturnValue(false);

    const state = baseProjectState();
    useBiStore.getState().loadStateFromProject(state);

    await new Promise(resolve => setTimeout(resolve, 0));

    expect(projectCacheLoad).not.toHaveBeenCalled();
    expect(useBiStore.getState().refreshAllData).not.toHaveBeenCalled();
  });
});
