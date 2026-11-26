import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

export interface NativeEngineInfo {
  engine: string;
  version: string;
  tableCount: number;
}

export interface NativeColumnSchema {
  name: string;
  originalName: string;
  dataType: 'text' | 'number' | 'date' | 'boolean' | 'currency' | 'percentage';
  duckdbType: string;
}

export interface NativeImportedTable {
  tableName: string;
  originalName: string;
  columns: NativeColumnSchema[];
  rows: Record<string, any>[];
  totalRows: number;
  truncated: boolean;
  sourcePath?: string | null;
  sheetName?: string | null;
}

export interface RefreshTableNativeRequest {
  tableName: string;
  sourceType: string;
  filePath?: string;
  sheetName?: string;
  mScript?: string;
  pbipRoot?: string;
}

export interface RefreshPbipResult {
  loadedTables: NativeImportedTable[];
  warnings: string[];
}

export interface NativeSqlQueryResult {
  columns: string[];
  rows: Record<string, any>[];
  rowCount: number;
}

export interface PbiColumnMeta {
  name: string;
  dataType: string;
  formatString?: string | null;
  sortByColumn?: string | null;
  isHidden: boolean;
  isCalculated: boolean;
  expression?: string | null;
}

export interface PbiMeasureMeta {
  name: string;
  tableName: string;
  expression: string;
  formatString?: string | null;
  displayFolder?: string | null;
  description?: string | null;
}

export interface PbiPartitionMeta {
  name: string;
  mode: string;
  sourceType: string;
  expression: string;
}

export interface PbiTableMeta {
  name: string;
  columns: PbiColumnMeta[];
  measures: PbiMeasureMeta[];
  partitions: PbiPartitionMeta[];
  isCalculated: boolean;
  isHidden: boolean;
}

export interface PbiRelationshipMeta {
  fromTable: string;
  fromColumn: string;
  toTable: string;
  toColumn: string;
  cardinality: string;
  crossFilterDirection: string;
  isActive: boolean;
}

export interface PbiFilterMeta {
  table: string;
  column: string;
  values: string[];
}

export interface PbiCondFormatRuleMeta {
  operator: string;
  value: number;
  value2?: number | null;
  color: string;
}

export interface PbiCondFormatStopMeta {
  kind: string;
  value?: number | null;
  color: string;
}

/** v0.10 Frente B: formatação condicional lida do PBIR (ver src-tauri/src/pbi/pbip.rs). */
export interface PbiConditionalFormatMeta {
  target: string;
  applyTo: string;
  mode: string;
  rules?: PbiCondFormatRuleMeta[];
  gradientMin?: PbiCondFormatStopMeta | null;
  gradientMid?: PbiCondFormatStopMeta | null;
  gradientMax?: PbiCondFormatStopMeta | null;
  dataBarPositive?: string | null;
  dataBarNegative?: string | null;
}

export interface PbiVisualMeta {
  id: string;
  visualType: string;
  originalVisualType: string;
  title: string;
  x: number;
  y: number;
  w: number;
  h: number;
  tableHint?: string | null;
  categoryField?: string | null;
  legendField?: string | null;
  valueFields: string[];
  columnCount?: number | null;
  slicerMode?: string | null;
  actionType?: string | null;
  actionTarget?: string | null;
  textContent?: string | null;
  filters?: PbiFilterMeta[];
  /** From PBIR query.sortDefinition (first sort entry). */
  sortField?: string | null;
  sortOrder?: 'asc' | 'desc' | null;
  /** v0.10 Frente B. */
  conditionalFormats?: PbiConditionalFormatMeta[];
}

export interface PbiPageMeta {
  id: string;
  name: string;
  width: number;
  height: number;
  visuals: PbiVisualMeta[];
  filters?: PbiFilterMeta[];
}

export interface PbiPowerQueryScript {
  name: string;
  script: string;
}

export interface PbiProjectSummary {
  sourceType: 'pbip' | 'pbix';
  projectName: string;
  rootPath: string;
  tables: PbiTableMeta[];
  relationships: PbiRelationshipMeta[];
  pages: PbiPageMeta[];
  powerQueryScripts: PbiPowerQueryScript[];
  loadedTables?: NativeImportedTable[];
  warnings: string[];
}

export interface ExportPbipFilter {
  table: string;
  column: string;
  values: string[];
}

export interface ExportPbipPayload {
  projectName: string;
  tables: {
    name: string;
    isHidden?: boolean;
    mScript?: string | null;
    columns: {
      name: string;
      dataType: string;
      formatString?: string | null;
      sortByColumn?: string | null;
      isHidden?: boolean;
      isCalculated?: boolean;
      expression?: string | null;
    }[];
  }[];
  relationships: {
    fromTable: string;
    fromColumn: string;
    toTable: string;
    toColumn: string;
    crossFilterDirection: string;
    isActive: boolean;
  }[];
  measures: {
    name: string;
    tableName: string;
    formula: string;
    formatString?: string | null;
    displayFolder?: string | null;
    description?: string | null;
  }[];
  pages: {
    id: string;
    name: string;
    width: number;
    height: number;
    filters?: ExportPbipFilter[];
    visuals: {
      id: string;
      visualType: string;
      title: string;
      x: number;
      y: number;
      w: number;
      h: number;
      tableId: string;
      xAxisField?: string | null;
      legendField?: string | null;
      yAxisFields: string[];
      cardColumns?: number | null;
      slicerMode?: string | null;
      textContent?: string | null;
      actionType?: string | null;
      actionTarget?: string | null;
      showTitle?: boolean | null;
      backgroundColor?: string | null;
      borderColor?: string | null;
      filters?: ExportPbipFilter[];
      /** Written as PBIR query.sortDefinition. */
      sortField?: string | null;
      sortOrder?: 'asc' | 'desc' | null;
    }[];
  }[];
  themeName?: string | null;
  themePalette?: string[] | null;
}

export interface DaxModelSyncPayload {
  tables: { name: string; columns: string[]; rows: Record<string, any>[]; columnTypes?: (string | null)[] }[];
  relationships: {
    fromTable: string;
    fromColumn: string;
    toTable: string;
    toColumn: string;
    crossFilterDirection: string;
    isActive: boolean;
  }[];
  measures: { name: string; tableName: string; formula: string }[];
}

export function isTauriRuntime(): boolean {
  return (
    typeof window !== 'undefined' &&
    Boolean((window as any).__TAURI_INTERNALS__ || (window as any).__TAURI__)
  );
}

export async function getNativeEngineInfo(): Promise<NativeEngineInfo> {
  return await invoke<NativeEngineInfo>('get_engine_info');
}

export async function importCsvNative(
  path: string,
  tableName?: string
): Promise<NativeImportedTable> {
  return await invoke<NativeImportedTable>('import_csv_native', {
    path,
    tableName: tableName ?? null,
  });
}

export async function importParquetNative(
  path: string,
  tableName?: string
): Promise<NativeImportedTable> {
  return await invoke<NativeImportedTable>('import_parquet_native', {
    path,
    tableName: tableName ?? null,
  });
}

export async function importJsonNative(
  path: string,
  tableName?: string
): Promise<NativeImportedTable> {
  return await invoke<NativeImportedTable>('import_json_native', {
    path,
    tableName: tableName ?? null,
  });
}

export async function importXlsxNative(path: string): Promise<NativeImportedTable[]> {
  return await invoke<NativeImportedTable[]>('import_xlsx_native', { path });
}

export async function importXlsxSheetNative(
  path: string,
  sheetName?: string,
  tableName?: string
): Promise<NativeImportedTable> {
  return await invoke<NativeImportedTable>('import_xlsx_sheet_native', {
    path,
    sheetName: sheetName ?? null,
    tableName: tableName ?? null,
  });
}

export async function refreshTableNative(
  request: RefreshTableNativeRequest
): Promise<NativeImportedTable> {
  return await invoke<NativeImportedTable>('refresh_table_native', { request });
}

export async function refreshPbipTables(path: string): Promise<RefreshPbipResult> {
  return await invoke<RefreshPbipResult>('refresh_pbip_tables', { path });
}

export async function queryDuckDbNative(sql: string): Promise<NativeSqlQueryResult> {
  return await invoke<NativeSqlQueryResult>('query_duckdb', { sql });
}

export async function openPbipProject(path: string): Promise<PbiProjectSummary> {
  return await invoke<PbiProjectSummary>('open_pbip_project', { path });
}

export async function openPbixFile(path: string): Promise<PbiProjectSummary> {
  return await invoke<PbiProjectSummary>('open_pbix_file', { path });
}

export async function pickDataFile(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const selected = await openDialog({
    multiple: false,
    title: 'Abrir arquivo de dados (CSV / Excel / Parquet / JSON)',
    filters: [
      {
        name: 'Arquivos de Dados',
        extensions: ['csv', 'tsv', 'xlsx', 'xls', 'parquet', 'json', 'ndjson'],
      },
    ],
  });
  return typeof selected === 'string' ? selected : null;
}

export async function savePbipProject(
  targetDir: string,
  payload: ExportPbipPayload
): Promise<string> {
  return await invoke<string>('save_pbip_project', { targetDir, payload });
}

export async function syncModelToRust(payload: DaxModelSyncPayload): Promise<number> {
  return await invoke<number>('sync_model_to_rust', { payload });
}

export async function evaluateDaxNative(
  expression: string,
  filters?: { tableId: string; column: string; values: any[] }[]
): Promise<{ value: any; display: string }> {
  return await invoke<{ value: any; display: string }>('evaluate_dax_native', {
    expression,
    filters: filters ?? null,
  });
}

export interface NativeVisualQueryRequest {
  visualId: string;
  visualType: string;
  tableId: string;
  xAxisField?: string;
  legendField?: string;
  yAxisFields: string[];
  aggregation?: string;
  filters: { tableId: string; column: string; values: any[] }[];
}

export interface NativeVisualSeries {
  name: string;
  data: number[];
}

export interface NativeVisualQueryResult {
  categories: string[];
  series: NativeVisualSeries[];
  summaryValue?: number | null;
  queryTimeMs: number;
  engine: string;
}

export async function queryVisualNative(
  request: NativeVisualQueryRequest
): Promise<NativeVisualQueryResult> {
  return await invoke<NativeVisualQueryResult>('query_visual_native', { request });
}

export async function pickFolderDialog(title = 'Selecionar pasta de destino'): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const selected = await openDialog({
    directory: true,
    multiple: false,
    title,
  });
  return typeof selected === 'string' ? selected : null;
}

export async function pickPowerBiFile(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const selected = await openDialog({
    multiple: false,
    title: 'Abrir projeto Power BI (.pbip ou .pbix)',
    filters: [
      {
        name: 'Projetos Power BI',
        extensions: ['pbip', 'pbix'],
      },
    ],
  });
  return typeof selected === 'string' ? selected : null;
}

export interface UpdateCheckResult {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  releaseUrl: string;
  releaseNotes: string;
  publishedAt: string;
  assetName: string;
  downloadUrl: string;
  assetSize: number;
  expectedSha256?: string | null;
  /** SHA256SUMS.txt com assinatura Ed25519 válida da chave oficial. */
  signatureVerified?: boolean;
  /** O backend guardou um pacote verificado pronto para instalar. */
  canInstall?: boolean;
  installKind?: 'appImage' | 'deb' | 'rpm' | 'managed' | 'unknown' | 'msiInstalled' | 'portable';
  installBlockedReason?: string | null;
  isAppimage: boolean;
  appimagePath?: string | null;
}

export interface UpdateInstallResult {
  installed: boolean;
  needsRestart: boolean;
  savedPath?: string | null;
  message: string;
}

export async function checkForUpdatesNative(currentVersion: string): Promise<UpdateCheckResult> {
  return await invoke<UpdateCheckResult>('check_for_updates', { currentVersion });
}

/**
 * Instala o pacote que o backend verificou no último `checkForUpdatesNative`.
 * Por segurança o frontend não escolhe URL nem hash.
 */
export async function downloadAndInstallUpdateNative(): Promise<UpdateInstallResult> {
  return await invoke<UpdateInstallResult>('download_and_install_update');
}

// ---------------------------------------------------------------------
// Conectores nativos de banco de dados (v0.8.0)
// ---------------------------------------------------------------------

export type DbConnectorConfig =
  | { kind: 'sqlite'; path: string }
  | { kind: 'duckdb_file'; path: string }
  | {
      kind: 'postgres';
      host: string;
      port: number;
      database: string;
      user: string;
      password?: string | null;
      schema?: string | null;
      sslMode?: string | null;
    }
  | {
      kind: 'mysql';
      host: string;
      port: number;
      database: string;
      user: string;
      password?: string | null;
    }
  | { kind: 'parquet_glob'; url: string; hivePartitioning?: boolean };

export interface DbConnectionInfo {
  connectionId: string;
  kind: DbConnectorConfig['kind'];
  config: DbConnectorConfig;
}

export interface DbSchemaInfo {
  name: string;
}

export interface DbTableInfo {
  schema: string;
  name: string;
}

export interface DbPreviewResult {
  columns: string[];
  rows: Record<string, any>[];
  rowCount: number;
}

export type DbImportSource =
  | { mode: 'table'; schema: string; table: string }
  | { mode: 'sql'; sql: string };

export async function dbConnect(
  config: DbConnectorConfig,
  persistCredentialKey?: string
): Promise<DbConnectionInfo> {
  return await invoke<DbConnectionInfo>('db_connect', {
    config,
    persistCredentialKey: persistCredentialKey ?? null,
  });
}

export async function dbListSchemas(connectionId: string): Promise<DbSchemaInfo[]> {
  return await invoke<DbSchemaInfo[]>('db_list_schemas', { connectionId });
}

export async function dbListTables(connectionId: string, schema?: string): Promise<DbTableInfo[]> {
  return await invoke<DbTableInfo[]>('db_list_tables', { connectionId, schema: schema ?? null });
}

export async function dbPreviewTable(
  connectionId: string,
  schema: string,
  table: string,
  limit?: number
): Promise<DbPreviewResult> {
  return await invoke<DbPreviewResult>('db_preview_table', {
    connectionId,
    schema,
    table,
    limit: limit ?? null,
  });
}

export async function dbQueryPreview(
  connectionId: string,
  sql: string,
  limit?: number
): Promise<DbPreviewResult> {
  return await invoke<DbPreviewResult>('db_query_preview', { connectionId, sql, limit: limit ?? null });
}

export async function dbImportTable(
  connectionId: string,
  source: DbImportSource,
  targetTableName: string
): Promise<NativeImportedTable> {
  return await invoke<NativeImportedTable>('db_import_table', {
    connectionId,
    source: source.mode === 'table'
      ? { mode: 'table', schema: source.schema, table: source.table }
      : { mode: 'sql', sql: source.sql },
    targetTableName,
  });
}

export async function dbDisconnect(connectionId: string): Promise<void> {
  await invoke<void>('db_disconnect', { connectionId });
}

/**
 * Usado pelo "Atualizar Dados": reconecta com a senha guardada no
 * keyring (identificada por `credentialKey`, nunca enviada antes pelo
 * frontend) e reimporta a tabela/consulta.
 */
export async function dbRefreshImport(
  config: DbConnectorConfig,
  credentialKey: string | undefined,
  source: DbImportSource,
  targetTableName: string
): Promise<NativeImportedTable> {
  return await invoke<NativeImportedTable>('db_refresh_import', {
    config,
    credentialKey: credentialKey ?? null,
    source: source.mode === 'table'
      ? { mode: 'table', schema: source.schema, table: source.table }
      : { mode: 'sql', sql: source.sql },
    targetTableName,
  });
}

// ---------------------------------------------------------------------
// Credenciais seguras (chaveiro Linux Secret Service) — v0.8.0
// ---------------------------------------------------------------------

export async function credentialsSet(key: string, secret: string): Promise<void> {
  await invoke<void>('credentials_set', { key, secret });
}

export async function credentialsGetExists(key: string): Promise<boolean> {
  return await invoke<boolean>('credentials_get_exists', { key });
}

export async function credentialsGet(key: string): Promise<string | null> {
  return await invoke<string | null>('credentials_get', { key });
}

export async function credentialsDelete(key: string): Promise<void> {
  await invoke<void>('credentials_delete', { key });
}

// ---------------------------------------------------------------------
// ETL v0.9 (Power Query no Rust/DuckDB): contrato em docs/ETL_V09.md.
// Tipos espelham src-tauri/src/etl/model.rs (serde tag "op", camelCase).
// ---------------------------------------------------------------------

export type NativeEtlType = 'text' | 'integer' | 'decimal' | 'currency' | 'date' | 'datetime' | 'boolean';
export type NativeEtlScalar = string | number | boolean | null;

export type NativeEtlCond =
  | {
      column: string;
      op: 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte' | 'contains' | 'notContains' | 'startsWith' | 'endsWith';
      value: NativeEtlScalar;
    }
  | { column: string; op: 'isNull' | 'isNotNull' | 'isBlank' | 'isNotBlank' }
  | { column: string; op: 'in' | 'notIn'; values: NativeEtlScalar[] }
  | { and: NativeEtlCond[] }
  | { or: NativeEtlCond[] }
  | { not: NativeEtlCond };

export type NativeEtlAggOp = 'sum' | 'avg' | 'min' | 'max' | 'count' | 'countDistinct' | 'countRows' | 'median';
export interface NativeEtlAgg {
  name: string;
  op: NativeEtlAggOp;
  column?: string;
}
export type NativeEtlValueOrColumn = NativeEtlScalar | { column: string };

export type NativeEtlStep = { id: string; name: string } & (
  | { op: 'promoteHeaders' }
  | { op: 'changeType'; columns: { column: string; type: NativeEtlType }[]; culture?: string }
  | { op: 'renameColumns'; renames: [string, string][] }
  | { op: 'removeColumns'; columns: string[] }
  | { op: 'selectColumns'; columns: string[] }
  | { op: 'duplicateColumn'; column: string; newColumn: string }
  | { op: 'filterRows'; condition: NativeEtlCond }
  | { op: 'removeBlankRows' }
  | { op: 'removeRowsWithErrors'; columns?: string[] }
  | { op: 'keepTopRows' | 'removeTopRows' | 'keepBottomRows' | 'removeBottomRows'; count: number }
  | { op: 'removeDuplicates'; columns?: string[] }
  | { op: 'sort'; by: { column: string; descending: boolean }[] }
  | {
      op: 'replaceValues';
      columns: string[];
      find: NativeEtlScalar;
      replace: NativeEtlScalar;
      matchEntireCell: boolean;
    }
  | { op: 'fillDown' | 'fillUp'; columns: string[] }
  | {
      op: 'splitColumn';
      column: string;
      mode: 'delimiterEach' | 'delimiterLeftmost' | 'delimiterRightmost' | 'positions' | 'fixedLength';
      delimiter?: string;
      positions?: number[];
      length?: number;
      into: string[];
    }
  | { op: 'mergeColumns'; columns: string[]; separator: string; newColumn: string }
  | { op: 'textTransform'; columns: string[]; fn: 'upper' | 'lower' | 'proper' | 'trim' | 'clean' }
  | { op: 'unpivotOtherColumns'; keep: string[]; attributeColumn: string; valueColumn: string }
  | { op: 'unpivotColumns'; columns: string[]; attributeColumn: string; valueColumn: string }
  | { op: 'pivot'; column: string; valuesColumn: string; aggregation: NativeEtlAggOp }
  | { op: 'groupBy'; keys: string[]; aggregations: NativeEtlAgg[] }
  | {
      op: 'addConditionalColumn';
      newColumn: string;
      rules: { when: NativeEtlCond; then: NativeEtlValueOrColumn }[];
      otherwise: NativeEtlValueOrColumn;
      type?: NativeEtlType;
    }
  | { op: 'addCustomColumn'; newColumn: string; expression: string; type?: NativeEtlType }
  | { op: 'addIndexColumn'; newColumn: string; start: number; step: number }
  | {
      op: 'mergeQueries';
      rightQuery: string;
      leftKeys: string[];
      rightKeys: string[];
      joinKind: 'leftOuter' | 'inner' | 'rightOuter' | 'fullOuter' | 'leftAnti' | 'rightAnti';
      expand: { column: string; as: string }[];
    }
  | { op: 'appendQueries'; queries: string[] }
  | { op: 'opaqueM'; m: string }
  // ---- Rodada 2 (docs/ETL_V09.md §"Rodada 2") ----
  | { op: 'transformColumns'; columns: string[]; expression: string; type?: NativeEtlType }
  | { op: 'reorderColumns'; columns: string[] }
  | { op: 'demoteHeaders' }
  | { op: 'transpose' }
  | { op: 'reverseRows' }
  | { op: 'keepRange'; offset: number; count: number }
  | { op: 'removeAlternateRows'; offset: number; remove: number; keep: number }
  | { op: 'keepDuplicates'; columns?: string[] }
  | { op: 'replaceErrors'; replacements: { column: string; value: NativeEtlScalar }[] }
  | { op: 'keepRowsWithErrors'; columns?: string[] }
);

export interface NativeEtlSource {
  kind: 'excel' | 'csv' | 'parquet' | 'json' | 'db' | 'table' | 'inline' | 'listDates' | 'opaqueM';
  path?: string;
  sheet?: string;
  table?: string;
  m?: string;
  columns?: string[];
  rows?: NativeEtlScalar[][];
  start?: string;
  count?: number;
  stepDays?: number;
  function?: string;
}

export interface NativeEtlQuery {
  name: string;
  source: NativeEtlSource;
  steps: NativeEtlStep[];
}

export interface NativeEtlColumn {
  name: string;
  type: NativeEtlType;
}

export interface NativeEtlRawLoadResult {
  rawTable: string;
  columns: NativeEtlColumn[];
  rowCount: number;
  sourcePath?: string;
}

export interface NativeEtlPreviewResult {
  columns: NativeEtlColumn[];
  rows: Record<string, any>[];
  rowCount: number;
  warnings: string[];
  failedStep?: { index: number; message: string };
}

export interface NativeEtlFromMResult {
  query: NativeEtlQuery;
  warnings: string[];
}

/** (Re)carrega a fonte bruta em `raw__<query>`. */
export async function etlLoadRaw(
  query: string,
  source: NativeEtlSource,
  pbipRoot?: string
): Promise<NativeEtlRawLoadResult> {
  return await invoke<NativeEtlRawLoadResult>('etl_load_raw', { query, source, pbipRoot: pbipRoot ?? null });
}

/** Preview até o passo `uptoStep` (inclusivo; negativo = só a fonte; omitido = todos). */
export async function etlPreview(
  query: NativeEtlQuery,
  uptoStep?: number,
  limit?: number
): Promise<NativeEtlPreviewResult> {
  return await invoke<NativeEtlPreviewResult>('etl_preview', {
    query,
    uptoStep: uptoStep ?? null,
    limit: limit ?? null,
  });
}

/** "Fechar e Aplicar": materializa a consulta em `targetTable`. */
/** `null`: the query yields no columns (measures host table). */
export async function etlApply(query: NativeEtlQuery, targetTable: string): Promise<NativeImportedTable | null> {
  return await invoke<NativeImportedTable | null>('etl_apply', { query, targetTable });
}

/** Script M completo (`let … in`) da consulta. */
export async function etlToM(query: NativeEtlQuery): Promise<string> {
  return await invoke<string>('etl_to_m', { query });
}

/** Converte um script M em consulta (passos não suportados viram `opaqueM` + aviso). */
export async function etlFromM(name: string, m: string): Promise<NativeEtlFromMResult> {
  return await invoke<NativeEtlFromMResult>('etl_from_m', { name, m });
}

/** Expressão da barra de fórmulas de uma única etapa (`"= Table.AddColumn(...)"`). */
export async function etlStepM(query: NativeEtlQuery, index: number): Promise<string> {
  return await invoke<string>('etl_step_m', { query, index });
}
