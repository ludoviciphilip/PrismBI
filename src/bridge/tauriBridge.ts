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

export interface ExportPbipPayload {
  projectName: string;
  tables: {
    name: string;
    isHidden?: boolean;
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
    }[];
  }[];
}

export interface DaxModelSyncPayload {
  tables: { name: string; columns: string[]; rows: Record<string, any>[] }[];
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

export async function importXlsxNative(path: string): Promise<NativeImportedTable[]> {
  return await invoke<NativeImportedTable[]>('import_xlsx_native', { path });
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
    title: 'Abrir arquivo de dados (CSV / Excel)',
    filters: [
      {
        name: 'Planilhas e Dados',
        extensions: ['csv', 'tsv', 'xlsx', 'xls'],
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
