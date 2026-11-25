import { describe, it, expect } from 'vitest';
import {
  buildConnectorConfig,
  buildDbSourceConfig,
  buildTableFromDbImport,
  generateCredentialKey,
  stripPassword,
  validateDbConnectionForm,
  DEFAULT_PORT_BY_KIND,
} from '../dbConnector';
import { NativeImportedTable } from '../../bridge/tauriBridge';

describe('dbConnector (v0.8.0 native connectors)', () => {
  it('validates required fields per connector kind', () => {
    expect(validateDbConnectionForm({ kind: 'sqlite' })).toEqual(['Informe o caminho do arquivo.']);
    expect(validateDbConnectionForm({ kind: 'sqlite', path: '/tmp/x.sqlite' })).toEqual([]);

    expect(validateDbConnectionForm({ kind: 'postgres' })).toEqual([
      'Informe o host.',
      'Informe o banco de dados.',
      'Informe o usuário.',
    ]);
    expect(
      validateDbConnectionForm({ kind: 'postgres', host: 'h', database: 'd', user: 'u' })
    ).toEqual([]);

    expect(validateDbConnectionForm({ kind: 'parquet_glob' })).toEqual([
      'Informe a URL/caminho do Parquet.',
    ]);
  });

  it('builds a Postgres ConnectorConfig with default port and no empty optional fields', () => {
    const config = buildConnectorConfig({
      kind: 'postgres',
      host: 'db.internal',
      database: 'sales',
      user: 'reader',
      password: 'secret',
    });
    expect(config).toEqual({
      kind: 'postgres',
      host: 'db.internal',
      port: DEFAULT_PORT_BY_KIND.postgres,
      database: 'sales',
      user: 'reader',
      password: 'secret',
      schema: undefined,
      sslMode: undefined,
    });
  });

  it('builds a MySQL ConnectorConfig with default port', () => {
    const config = buildConnectorConfig({
      kind: 'mysql',
      host: 'mysql.internal',
      database: 'shop',
      user: 'admin',
      password: 'hunter2',
    });
    expect(config).toMatchObject({ kind: 'mysql', port: 3306, host: 'mysql.internal' });
  });

  it('builds a Parquet glob config with hivePartitioning flag', () => {
    const config = buildConnectorConfig({ kind: 'parquet_glob', url: 's3://b/*.parquet', hivePartitioning: true });
    expect(config).toEqual({ kind: 'parquet_glob', url: 's3://b/*.parquet', hivePartitioning: true });
  });

  it('stripPassword removes password but keeps everything else', () => {
    const config = buildConnectorConfig({
      kind: 'postgres',
      host: 'h',
      database: 'd',
      user: 'u',
      password: 'should-not-persist',
    });
    const stripped = stripPassword(config);
    expect(stripped.password).toBeUndefined();
    expect(stripped.host).toBe('h');
    expect(JSON.stringify(stripped)).not.toContain('should-not-persist');
  });

  it('generateCredentialKey returns distinct non-empty keys', () => {
    const a = generateCredentialKey();
    const b = generateCredentialKey();
    expect(a).not.toBe(b);
    expect(a.length).toBeGreaterThan(5);
  });

  it('buildDbSourceConfig attaches credentialKey only for password-bearing connectors', () => {
    const pgConfig = buildConnectorConfig({ kind: 'postgres', host: 'h', database: 'd', user: 'u', password: 'x' });
    const key = generateCredentialKey();
    const src = buildDbSourceConfig(pgConfig, { mode: 'table', schema: 'public', table: 'clientes' }, key);
    expect(src.credentialKey).toBe(key);
    expect(src.config.password).toBeUndefined();

    const sqliteConfig = buildConnectorConfig({ kind: 'sqlite', path: '/tmp/x.sqlite' });
    const sqliteSrc = buildDbSourceConfig(sqliteConfig, { mode: 'table', schema: 'main', table: 't' }, key);
    expect(sqliteSrc.credentialKey).toBeUndefined();
  });

  it('buildTableFromDbImport maps columns/rows and tags sourceConfig as db', () => {
    const imported: NativeImportedTable = {
      tableName: 'clientes',
      originalName: 'clientes',
      columns: [
        { name: 'id', originalName: 'id', dataType: 'number', duckdbType: 'BIGINT' },
        { name: 'nome', originalName: 'nome', dataType: 'text', duckdbType: 'VARCHAR' },
      ],
      rows: [{ id: 1, nome: 'Ana' }],
      totalRows: 1,
      truncated: false,
    };
    const dbSourceConfig = buildDbSourceConfig(
      buildConnectorConfig({ kind: 'sqlite', path: '/tmp/x.sqlite' }),
      { mode: 'table', schema: 'main', table: 'clientes' }
    );
    const table = buildTableFromDbImport(imported, dbSourceConfig, { x: 10, y: 20 });
    expect(table.name).toBe('clientes');
    expect(table.columns).toHaveLength(2);
    expect(table.rows).toEqual([{ id: 1, nome: 'Ana' }]);
    expect(table.sourceConfig?.type).toBe('db');
    expect(table.sourceConfig?.db?.kind).toBe('sqlite');
    expect(table.x).toBe(10);
    expect(table.y).toBe(20);
  });
});
