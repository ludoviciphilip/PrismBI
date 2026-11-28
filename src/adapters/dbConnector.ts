/**
 * Conector real de banco de dados (v0.8.0): funções puras usadas pela UI
 * (`DatabaseConnectorModal`) e pelo fluxo de "Atualizar Dados" para
 * montar a configuração enviada ao Rust (`db_connect` / `db_refresh_import`)
 * e para transformar o resultado importado em uma `Table` do modelo, do
 * mesmo jeito que os importadores nativos de CSV/XLSX já fazem.
 *
 * Substitui o uso de `postgresMockAdapter` nos caminhos de produção: o
 * mock continua existindo apenas como dado de demonstração/teste, sem
 * ser mais referenciado pelo `DataSourceModal`.
 */
import { Column, DataType, DbConnectorKind, DbSourceConfig, Table } from '../types/bi';
import { DbConnectorConfig, DbImportSource, NativeImportedTable } from '../bridge/tauriBridge';

export interface DbConnectionFormState {
  kind: DbConnectorKind;
  path?: string;
  host?: string;
  port?: string | number;
  database?: string;
  user?: string;
  password?: string;
  schema?: string;
  sslMode?: string;
  url?: string;
  hivePartitioning?: boolean;
}

export const DEFAULT_PORT_BY_KIND: Record<DbConnectorKind, number | undefined> = {
  sqlite: undefined,
  duckdb_file: undefined,
  postgres: 5432,
  mysql: 3306,
  parquet_glob: undefined,
};

/** Gera uma chave estável para o keyring (independente do connectionId efêmero de sessão). */
export function generateCredentialKey(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return `vetrobi-db-${crypto.randomUUID()}`;
  }
  return `vetrobi-db-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

/** Valida os campos obrigatórios do formulário de conexão por tipo de conector. */
export function validateDbConnectionForm(form: DbConnectionFormState): string[] {
  const errors: string[] = [];
  switch (form.kind) {
    case 'sqlite':
    case 'duckdb_file':
      if (!form.path?.trim()) errors.push('Informe o caminho do arquivo.');
      break;
    case 'postgres':
    case 'mysql':
      if (!form.host?.trim()) errors.push('Informe o host.');
      if (!form.database?.trim()) errors.push('Informe o banco de dados.');
      if (!form.user?.trim()) errors.push('Informe o usuário.');
      break;
    case 'parquet_glob':
      if (!form.url?.trim()) errors.push('Informe a URL/caminho do Parquet.');
      break;
  }
  return errors;
}

/** Monta o `ConnectorConfig` (com senha, se houver) a ser enviado ao Rust em `db_connect`. */
export function buildConnectorConfig(form: DbConnectionFormState): DbConnectorConfig {
  switch (form.kind) {
    case 'sqlite':
      return { kind: 'sqlite', path: form.path || '' };
    case 'duckdb_file':
      return { kind: 'duckdb_file', path: form.path || '' };
    case 'postgres':
      return {
        kind: 'postgres',
        host: form.host || '',
        port: Number(form.port || DEFAULT_PORT_BY_KIND.postgres),
        database: form.database || '',
        user: form.user || '',
        password: form.password || undefined,
        schema: form.schema || undefined,
        sslMode: form.sslMode || undefined,
      };
    case 'mysql':
      return {
        kind: 'mysql',
        host: form.host || '',
        port: Number(form.port || DEFAULT_PORT_BY_KIND.mysql),
        database: form.database || '',
        user: form.user || '',
        password: form.password || undefined,
      };
    case 'parquet_glob':
      return { kind: 'parquet_glob', url: form.url || '', hivePartitioning: !!form.hivePartitioning };
  }
}

/**
 * Remove explicitamente qualquer campo de senha antes de persistir a
 * configuração como `TableSourceConfig.db.config` (defesa em profundidade:
 * o backend já nunca devolve a senha, mas o frontend nunca deve guardá-la
 * de qualquer forma mesmo que um bug a reenviasse).
 */
export function stripPassword(config: DbConnectorConfig): Record<string, any> {
  const { ...rest } = config as Record<string, any>;
  delete rest.password;
  return rest;
}

/** Monta o `TableSourceConfig` persistido no modelo (`.insight`), sem senha. */
export function buildDbSourceConfig(
  config: DbConnectorConfig,
  source: DbImportSource,
  credentialKey?: string
): DbSourceConfig {
  return {
    kind: config.kind,
    credentialKey:
      config.kind === 'postgres' || config.kind === 'mysql' ? credentialKey : undefined,
    config: stripPassword(config),
    source,
  };
}

/** Converte o payload importado (via `db_import_table`/`db_refresh_import`) em uma `Table` do modelo. */
export function buildTableFromDbImport(
  imported: NativeImportedTable,
  dbSourceConfig: DbSourceConfig,
  position: { x: number; y: number } = { x: 40, y: 40 }
): Table {
  const columns: Column[] = imported.columns.map((c, idx) => ({
    id: `${imported.tableName}_col_${idx}_${c.name}`,
    name: c.name,
    originalName: c.originalName,
    dataType: (c.dataType as DataType) || 'text',
  }));

  return {
    id: imported.tableName,
    name: imported.tableName,
    originalName: imported.originalName,
    columns,
    rows: imported.rows,
    sourceConfig: {
      type: 'db',
      db: dbSourceConfig,
      lastRefreshedAt: new Date().toISOString(),
    },
    duckdbTable: imported.tableName,
    rowsTruncated: imported.truncated,
    totalRows: imported.totalRows,
    x: position.x,
    y: position.y,
  };
}

export const DB_CONNECTOR_LABELS: Record<DbConnectorKind, string> = {
  sqlite: 'SQLite',
  duckdb_file: 'Arquivo .duckdb',
  postgres: 'PostgreSQL',
  mysql: 'MySQL / MariaDB',
  parquet_glob: 'Parquet (particionado / HTTP / S3)',
};
