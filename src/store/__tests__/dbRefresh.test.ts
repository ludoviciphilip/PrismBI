import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('../../bridge/tauriBridge', async () => {
  const actual = await vi.importActual<any>('../../bridge/tauriBridge');
  return {
    ...actual,
    isTauriRuntime: vi.fn(() => true),
    dbRefreshImport: vi.fn(),
    syncModelToRust: vi.fn(async () => 0),
  };
});

import { useBiStore } from '../useBiStore';
import { dbRefreshImport, isTauriRuntime } from '../../bridge/tauriBridge';
import { buildDbSourceConfig, buildConnectorConfig } from '../../adapters/dbConnector';

describe('useBiStore.refreshTable (db-backed TableSourceConfig, v0.8.0)', () => {
  beforeEach(() => {
    useBiStore.setState({ tables: [], relationships: [], measures: [] } as any);
    vi.mocked(dbRefreshImport).mockReset();
    vi.mocked(isTauriRuntime).mockReturnValue(true);
  });

  it('calls dbRefreshImport with the stored (password-free) config and credentialKey, then replaces table data', async () => {
    const pgConfig = buildConnectorConfig({ kind: 'postgres', host: 'h', database: 'd', user: 'u', password: 'ignored' });
    const dbSourceConfig = buildDbSourceConfig(
      pgConfig,
      { mode: 'table', schema: 'public', table: 'clientes' },
      'cred-key-123'
    );

    useBiStore.setState(s => ({
      ...s,
      tables: [
        {
          id: 'clientes',
          name: 'clientes',
          originalName: 'clientes',
          columns: [],
          rows: [],
          sourceConfig: { type: 'db', db: dbSourceConfig },
        } as any,
      ],
    }));

    vi.mocked(dbRefreshImport).mockResolvedValue({
      tableName: 'clientes',
      originalName: 'clientes',
      columns: [{ name: 'id', originalName: 'id', dataType: 'number', duckdbType: 'BIGINT' }],
      rows: [{ id: 1 }, { id: 2 }],
      totalRows: 2,
      truncated: false,
    } as any);

    const result = await useBiStore.getState().refreshTable('clientes');

    expect(result.ok).toBe(true);
    expect(dbRefreshImport).toHaveBeenCalledTimes(1);
    const [calledConfig, calledCredKey, calledSource, calledTableName] = vi.mocked(dbRefreshImport).mock.calls[0];
    expect((calledConfig as any).password).toBeUndefined();
    expect(calledCredKey).toBe('cred-key-123');
    expect(calledSource).toEqual({ mode: 'table', schema: 'public', table: 'clientes' });
    expect(calledTableName).toBe('clientes');

    const updated = useBiStore.getState().tables.find(t => t.id === 'clientes');
    expect(updated?.rows).toEqual([{ id: 1 }, { id: 2 }]);
    expect(updated?.sourceConfig?.lastRefreshedAt).toBeTruthy();
  });

  it('surfaces a refresh error without throwing when dbRefreshImport rejects', async () => {
    const sqliteConfig = buildConnectorConfig({ kind: 'sqlite', path: '/tmp/x.sqlite' });
    const dbSourceConfig = buildDbSourceConfig(sqliteConfig, { mode: 'table', schema: 'main', table: 't' });

    useBiStore.setState(s => ({
      ...s,
      tables: [
        {
          id: 't',
          name: 't',
          originalName: 't',
          columns: [],
          rows: [],
          sourceConfig: { type: 'db', db: dbSourceConfig },
        } as any,
      ],
    }));

    vi.mocked(dbRefreshImport).mockRejectedValue(new Error('arquivo não encontrado'));

    const result = await useBiStore.getState().refreshTable('t');
    expect(result.ok).toBe(false);
    expect(result.message).toContain('não encontrado');
  });
});
