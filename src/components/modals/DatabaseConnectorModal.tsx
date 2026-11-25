import React, { useState } from 'react';
import { Database, Loader2, CheckCircle2, AlertCircle, Table2, Code2 } from 'lucide-react';
import { useBiStore } from '../../store/useBiStore';
import {
  dbConnect,
  dbDisconnect,
  dbImportTable,
  dbListSchemas,
  dbListTables,
  dbPreviewTable,
  dbQueryPreview,
  DbConnectionInfo,
  DbPreviewResult,
  DbSchemaInfo,
  DbTableInfo,
} from '../../bridge/tauriBridge';
import {
  DB_CONNECTOR_LABELS,
  DbConnectionFormState,
  buildConnectorConfig,
  buildDbSourceConfig,
  buildTableFromDbImport,
  generateCredentialKey,
  validateDbConnectionForm,
} from '../../adapters/dbConnector';
import { DbConnectorKind } from '../../types/bi';

const CONNECTOR_KINDS: DbConnectorKind[] = ['sqlite', 'duckdb_file', 'postgres', 'mysql', 'parquet_glob'];

/**
 * Painel real de conectores nativos de banco de dados (v0.8.0): conector
 * -> conexão -> navegador de schemas/tabelas com pré-visualização -> SQL
 * livre -> carregar como tabela do modelo. Substitui, nesta aba, o antigo
 * fluxo simulado de `postgresMockAdapter`.
 */
export const NativeDbConnectorPanel: React.FC<{ onImported?: () => void }> = ({ onImported }) => {
  const addTable = useBiStore(s => s.addTable);

  const [form, setForm] = useState<DbConnectionFormState>({ kind: 'sqlite' });
  const [connecting, setConnecting] = useState(false);
  const [connectError, setConnectError] = useState<string | null>(null);
  const [connection, setConnection] = useState<DbConnectionInfo | null>(null);
  const [credentialKey, setCredentialKey] = useState<string | undefined>(undefined);

  const [schemas, setSchemas] = useState<DbSchemaInfo[]>([]);
  const [selectedSchema, setSelectedSchema] = useState<string | undefined>(undefined);
  const [tables, setTables] = useState<DbTableInfo[]>([]);
  const [selectedTable, setSelectedTable] = useState<DbTableInfo | null>(null);
  const [preview, setPreview] = useState<DbPreviewResult | null>(null);
  const [mode, setMode] = useState<'browse' | 'sql'>('browse');
  const [sql, setSql] = useState('SELECT * FROM ');
  const [targetTableName, setTargetTableName] = useState('');
  const [busy, setBusy] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);

  const resetConnection = async () => {
    if (connection) {
      try {
        await dbDisconnect(connection.connectionId);
      } catch {
        /* ignore */
      }
    }
    setConnection(null);
    setSchemas([]);
    setSelectedSchema(undefined);
    setTables([]);
    setSelectedTable(null);
    setPreview(null);
  };

  const handleConnect = async () => {
    const errors = validateDbConnectionForm(form);
    if (errors.length > 0) {
      setConnectError(errors.join(' '));
      return;
    }
    setConnecting(true);
    setConnectError(null);
    try {
      const config = buildConnectorConfig(form);
      const needsCredential = config.kind === 'postgres' || config.kind === 'mysql';
      const key = needsCredential ? generateCredentialKey() : undefined;
      const info = await dbConnect(config, key);
      setConnection(info);
      setCredentialKey(key);

      if (info.kind !== 'parquet_glob') {
        const schemaList = await dbListSchemas(info.connectionId);
        setSchemas(schemaList);
        const defaultSchema = schemaList.find(s => s.name === 'main' || s.name === 'public')?.name
          ?? schemaList[0]?.name;
        setSelectedSchema(defaultSchema);
        if (defaultSchema) {
          const tableList = await dbListTables(info.connectionId, defaultSchema);
          setTables(tableList);
        }
      } else {
        setMode('sql');
      }
    } catch (err: any) {
      setConnectError(err?.message || String(err));
    } finally {
      setConnecting(false);
    }
  };

  const handleSelectSchema = async (schema: string) => {
    if (!connection) return;
    setSelectedSchema(schema);
    setSelectedTable(null);
    setPreview(null);
    const tableList = await dbListTables(connection.connectionId, schema);
    setTables(tableList);
  };

  const handleSelectTable = async (table: DbTableInfo) => {
    if (!connection) return;
    setSelectedTable(table);
    setTargetTableName(table.name);
    setBusy(true);
    try {
      const result = await dbPreviewTable(connection.connectionId, table.schema, table.name, 100);
      setPreview(result);
    } catch (err: any) {
      setConnectError(err?.message || String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRunSql = async () => {
    if (!connection) return;
    setBusy(true);
    setConnectError(null);
    try {
      const result = await dbQueryPreview(connection.connectionId, sql, 100);
      setPreview(result);
    } catch (err: any) {
      setConnectError(err?.message || String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleLoad = async () => {
    if (!connection) return;
    const name = targetTableName.trim();
    if (!name) {
      setConnectError('Informe um nome para a tabela a ser criada.');
      return;
    }
    setBusy(true);
    setConnectError(null);
    try {
      const source = mode === 'sql'
        ? ({ mode: 'sql', sql } as const)
        : selectedTable
        ? ({ mode: 'table', schema: selectedTable.schema, table: selectedTable.name } as const)
        : null;
      if (!source) {
        setConnectError('Selecione uma tabela ou escreva uma consulta SQL.');
        setBusy(false);
        return;
      }
      const imported = await dbImportTable(connection.connectionId, source, name);
      const dbSourceConfig = buildDbSourceConfig(connection.config, source, credentialKey);
      const table = buildTableFromDbImport(imported, dbSourceConfig);
      addTable(table);
      setStatusMsg(`Tabela "${table.name}" importada com sucesso (${imported.totalRows.toLocaleString('pt-BR')} linhas).`);
      onImported?.();
    } catch (err: any) {
      setConnectError(err?.message || String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="p-3 bg-sky-50 dark:bg-sky-950/40 rounded-xl border border-sky-200 dark:border-sky-800 flex items-start gap-2">
        <Database size={18} className="text-sky-600 dark:text-sky-400 mt-0.5 shrink-0" />
        <div className="text-xs text-sky-900 dark:text-sky-200">
          Conector nativo via DuckDB (ATTACH). Senhas nunca ficam em arquivos do projeto — são
          guardadas no chaveiro nativo do Linux (Secret Service) e nunca retornam ao frontend.
        </div>
      </div>

      {!connection && (
        <div className="space-y-3">
          <div className="flex flex-wrap gap-1.5">
            {CONNECTOR_KINDS.map(kind => (
              <button
                key={kind}
                onClick={() => setForm({ kind })}
                className={`px-2.5 py-1 rounded-lg text-xs font-semibold cursor-pointer ${
                  form.kind === kind
                    ? 'bg-amber-500 text-slate-950'
                    : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300'
                }`}
              >
                {DB_CONNECTOR_LABELS[kind]}
              </button>
            ))}
          </div>

          <div className="grid grid-cols-2 gap-2">
            {(form.kind === 'sqlite' || form.kind === 'duckdb_file') && (
              <input
                className="col-span-2 px-2 py-1.5 rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm"
                placeholder="Caminho do arquivo (ex.: /dados/empresa.sqlite)"
                value={form.path || ''}
                onChange={e => setForm({ ...form, path: e.target.value })}
              />
            )}
            {(form.kind === 'postgres' || form.kind === 'mysql') && (
              <>
                <input
                  className="px-2 py-1.5 rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm"
                  placeholder="Host"
                  value={form.host || ''}
                  onChange={e => setForm({ ...form, host: e.target.value })}
                />
                <input
                  className="px-2 py-1.5 rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm"
                  placeholder="Porta"
                  value={form.port ?? ''}
                  onChange={e => setForm({ ...form, port: e.target.value })}
                />
                <input
                  className="px-2 py-1.5 rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm"
                  placeholder="Banco de dados"
                  value={form.database || ''}
                  onChange={e => setForm({ ...form, database: e.target.value })}
                />
                <input
                  className="px-2 py-1.5 rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm"
                  placeholder="Usuário"
                  value={form.user || ''}
                  onChange={e => setForm({ ...form, user: e.target.value })}
                />
                <input
                  className="col-span-2 px-2 py-1.5 rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm"
                  placeholder="Senha"
                  type="password"
                  value={form.password || ''}
                  onChange={e => setForm({ ...form, password: e.target.value })}
                />
              </>
            )}
            {form.kind === 'parquet_glob' && (
              <input
                className="col-span-2 px-2 py-1.5 rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm"
                placeholder="URL/glob (ex.: s3://bucket/dados/*/*.parquet)"
                value={form.url || ''}
                onChange={e => setForm({ ...form, url: e.target.value })}
              />
            )}
          </div>

          {connectError && (
            <div className="flex items-center gap-1.5 text-xs text-rose-600 dark:text-rose-400">
              <AlertCircle size={14} /> {connectError}
            </div>
          )}

          <button
            onClick={handleConnect}
            disabled={connecting}
            className="w-full py-2 rounded-lg bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold text-sm flex items-center justify-center gap-2 cursor-pointer disabled:opacity-60"
          >
            {connecting ? <Loader2 size={16} className="animate-spin" /> : <Database size={16} />}
            Conectar
          </button>
        </div>
      )}

      {connection && (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-bold text-emerald-600 dark:text-emerald-400 flex items-center gap-1.5">
              <CheckCircle2 size={14} /> Conectado ({DB_CONNECTOR_LABELS[connection.kind]})
            </span>
            <button onClick={resetConnection} className="text-xs text-slate-500 hover:underline cursor-pointer">
              Desconectar
            </button>
          </div>

          {connection.kind !== 'parquet_glob' && (
            <div className="flex gap-2 text-xs">
              <button
                onClick={() => setMode('browse')}
                className={`px-2 py-1 rounded-lg font-semibold cursor-pointer ${mode === 'browse' ? 'bg-amber-500 text-slate-950' : 'bg-slate-100 dark:bg-slate-800'}`}
              >
                <Table2 size={12} className="inline mr-1" /> Navegador
              </button>
              <button
                onClick={() => setMode('sql')}
                className={`px-2 py-1 rounded-lg font-semibold cursor-pointer ${mode === 'sql' ? 'bg-amber-500 text-slate-950' : 'bg-slate-100 dark:bg-slate-800'}`}
              >
                <Code2 size={12} className="inline mr-1" /> SQL
              </button>
            </div>
          )}

          {mode === 'browse' && connection.kind !== 'parquet_glob' && (
            <div className="grid grid-cols-2 gap-3">
              <div>
                <div className="text-[10px] font-bold uppercase text-slate-400 mb-1">Schemas</div>
                <select
                  className="w-full px-2 py-1 text-xs rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900"
                  value={selectedSchema || ''}
                  onChange={e => handleSelectSchema(e.target.value)}
                >
                  {schemas.map(s => (
                    <option key={s.name} value={s.name}>
                      {s.name}
                    </option>
                  ))}
                </select>
                <div className="mt-2 max-h-40 overflow-y-auto space-y-1">
                  {tables.map(t => (
                    <button
                      key={`${t.schema}.${t.name}`}
                      onClick={() => handleSelectTable(t)}
                      className={`w-full text-left px-2 py-1 rounded text-xs cursor-pointer ${
                        selectedTable?.name === t.name ? 'bg-amber-100 dark:bg-amber-900/40 font-semibold' : 'hover:bg-slate-100 dark:hover:bg-slate-800'
                      }`}
                    >
                      {t.name}
                    </button>
                  ))}
                </div>
              </div>
              <div>
                <div className="text-[10px] font-bold uppercase text-slate-400 mb-1">Nome da tabela no modelo</div>
                <input
                  className="w-full px-2 py-1 text-xs rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900"
                  value={targetTableName}
                  onChange={e => setTargetTableName(e.target.value)}
                />
              </div>
            </div>
          )}

          {mode === 'sql' && (
            <div className="space-y-2">
              <textarea
                className="w-full h-20 px-2 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900"
                value={sql}
                onChange={e => setSql(e.target.value)}
              />
              <div className="flex gap-2">
                <button
                  onClick={handleRunSql}
                  disabled={busy}
                  className="px-3 py-1.5 rounded-lg bg-slate-700 text-white text-xs font-semibold cursor-pointer disabled:opacity-60"
                >
                  Executar (pré-visualização)
                </button>
                <input
                  className="flex-1 px-2 py-1 text-xs rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900"
                  placeholder="Nome da tabela no modelo"
                  value={targetTableName}
                  onChange={e => setTargetTableName(e.target.value)}
                />
              </div>
            </div>
          )}

          {preview && (
            <div className="overflow-auto max-h-48 border border-slate-200 dark:border-slate-800 rounded-lg">
              <table className="text-xs w-full">
                <thead className="bg-slate-100 dark:bg-slate-800">
                  <tr>
                    {preview.columns.map(c => (
                      <th key={c} className="px-2 py-1 text-left font-bold">
                        {c}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {preview.rows.slice(0, 20).map((row, idx) => (
                    <tr key={idx} className="odd:bg-white even:bg-slate-50 dark:odd:bg-slate-900 dark:even:bg-slate-800/50">
                      {preview.columns.map(c => (
                        <td key={c} className="px-2 py-1">
                          {String(row[c] ?? '')}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {connectError && (
            <div className="flex items-center gap-1.5 text-xs text-rose-600 dark:text-rose-400">
              <AlertCircle size={14} /> {connectError}
            </div>
          )}
          {statusMsg && (
            <div className="flex items-center gap-1.5 text-xs text-emerald-600 dark:text-emerald-400">
              <CheckCircle2 size={14} /> {statusMsg}
            </div>
          )}

          <button
            onClick={handleLoad}
            disabled={busy}
            className="w-full py-2 rounded-lg bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold text-sm cursor-pointer disabled:opacity-60"
          >
            Carregar no modelo
          </button>
        </div>
      )}
    </div>
  );
};
