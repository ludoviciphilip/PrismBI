import {
  Table,
  Relationship,
  Measure,
  Page,
  Filter,
  Bookmark,
  DashboardTile,
  Goal,
  RlsRole,
  WhatIfParameter,
  BiTheme,
  Column,
  TableSourceConfig,
  CalculationGroup,
  UserDefinedFunction,
  DaxQueryTab,
} from '../types/bi';
import { EtlQuery } from '../types/etl';
import { migrateProjectState, CURRENT_ENGINE_VERSION } from './migrations';
import { APP_VERSION } from '../version';

export interface ProjectState {
  version: string;
  appVersion?: string;
  /** Id estável do projeto, gerado uma vez e persistido (v0.11+). Usado
   * só como chave para o cache colunar nativo (`project_cache_*`,
   * ver src-tauri/src/project_store) — nunca como caminho de arquivo. */
  projectId?: string;
  projectName: string;
  sourcePbipPath?: string | null;
  tables: Table[];
  relationships: Relationship[];
  measures: Measure[];
  pages: Page[];
  activePageId: string;
  filters: Filter[];
  bookmarks: Bookmark[];
  dashboardTiles: DashboardTile[];
  goals: Goal[];
  rlsRoles: RlsRole[];
  whatIfParameters: WhatIfParameter[];
  calculationGroups?: CalculationGroup[];
  userDefinedFunctions?: UserDefinedFunction[];
  daxQueries?: DaxQueryTab[];
  theme: string;
  activeThemeId?: string;
  customThemes?: BiTheme[];
  gridSize?: number;
  showGrid?: boolean;
  snapToGrid?: boolean;
  language?: string;
  currency?: string;
  /** Consultas de ETL por tabela (v0.9.0+). Chave = tableId. */
  appliedSteps?: Record<string, EtlQuery>;
  /** Snapshot da última consulta aplicada com sucesso por tabela — usado para "consultas sujas". */
  appliedQuerySnapshot?: Record<string, EtlQuery>;
  updatedAt: string;
}

export interface SaveResult {
  success: boolean;
  timestamp: string;
  error?: string;
}

export interface ChunkedTableRecord {
  id: string;
  name: string;
  originalName: string;
  columns: Column[];
  sourceConfig?: TableSourceConfig;
  isCalculated?: boolean;
  isCalendar?: boolean;
  isHidden?: boolean;
  isDim?: boolean;
  x?: number;
  y?: number;
  totalRows: number;
  chunks: Record<string, any>[][];
  /** DuckDB table name backing this table (see Table.duckdbTable). DuckDB
   * is in-memory, so right after a project reopens this table may not
   * exist yet; sync_model_to_rust falls back to the full `rows` restored
   * from `chunks` below (NOT just a 5000-row preview — unlike the native
   * import payload, IndexedDB stores every row) until something (refresh,
   * .pbip reopen) recreates the DuckDB table and the fast path resumes. */
  duckdbTable?: string;
  rowsTruncated?: boolean;
  /** Full row count in the backing DuckDB table (distinct from `totalRows`
   * above, which is this record's own restored-row count). */
  duckdbTotalRows?: number;
}

const DB_NAME = 'InsightHubDB';
const DB_VERSION = 2;
const STORE_META = 'meta';
const STORE_TABLES = 'tables';
const V1_LEGACY_STORE = 'projectStore';
const META_KEY = 'current_project_meta';
const CHUNK_SIZE = 20000;

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onupgradeneeded = (event) => {
      const db = request.result;
      // Store for metadata (pages, measures, filters, theme, etc.)
      if (!db.objectStoreNames.contains(STORE_META)) {
        db.createObjectStore(STORE_META);
      }
      // Store for tables chunked by rows (20,000 rows per chunk)
      if (!db.objectStoreNames.contains(STORE_TABLES)) {
        db.createObjectStore(STORE_TABLES, { keyPath: 'id' });
      }
      // Keep legacy store if upgrading from v1
      if (event.oldVersion < 1 && !db.objectStoreNames.contains(V1_LEGACY_STORE)) {
        db.createObjectStore(V1_LEGACY_STORE);
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error('Falha ao abrir IndexedDB'));
  });
}

/**
 * Split an array of rows into chunks of maximum size chunkSize
 */
function chunkRows(rows: Record<string, any>[], chunkSize = CHUNK_SIZE): Record<string, any>[][] {
  if (!rows || rows.length === 0) return [];
  const chunks: Record<string, any>[][] = [];
  for (let i = 0; i < rows.length; i += chunkSize) {
    chunks.push(rows.slice(i, i + chunkSize));
  }
  return chunks;
}

/**
 * Saves project state to IndexedDB v2 with meta and chunked tables.
 * Guaranteed completion via tx.oncomplete.
 */
export async function saveProjectToStorage(state: ProjectState): Promise<SaveResult> {
  const timestamp = new Date().toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' });

  try {
    const normalized = migrateProjectState(state);
    const db = await openDatabase();

    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction([STORE_META, STORE_TABLES], 'readwrite');

      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error || new Error('Falha na transação IndexedDB'));
      tx.onabort = () => reject(tx.error || new Error('Transação IndexedDB abortada'));

      const metaStore = tx.objectStore(STORE_META);
      const tablesStore = tx.objectStore(STORE_TABLES);

      // 1. Save metadata (everything except heavy row data)
      const { tables, ...metaData } = normalized;
      metaStore.put(
        {
          ...metaData,
          tableIds: tables.map(t => t.id),
        },
        META_KEY
      );

      // 2. Clear old tables and save updated chunked table records
      tablesStore.clear();
      tables.forEach(table => {
        const chunks = chunkRows(table.rows || [], CHUNK_SIZE);
        const record: ChunkedTableRecord = {
          id: table.id,
          name: table.name,
          originalName: table.originalName,
          columns: table.columns,
          sourceConfig: table.sourceConfig,
          isCalculated: table.isCalculated,
          isCalendar: table.isCalendar,
          isHidden: table.isHidden,
          isDim: table.isDim,
          x: table.x,
          y: table.y,
          totalRows: table.rows ? table.rows.length : 0,
          chunks,
          duckdbTable: table.duckdbTable,
          rowsTruncated: table.rowsTruncated,
          duckdbTotalRows: table.totalRows,
        };
        tablesStore.put(record);
      });
    });

    // Save lightweight metadata only in localStorage (never full rows)
    try {
      const lightweightMeta = {
        projectName: normalized.projectName,
        updatedAt: normalized.updatedAt,
        tableCount: normalized.tables.length,
        version: normalized.version,
      };
      localStorage.setItem('insighthub_meta_backup', JSON.stringify(lightweightMeta));
      localStorage.setItem('insighthub_last_saved', new Date().toISOString());
    } catch {
      // Ignore localStorage errors
    }

    return { success: true, timestamp };
  } catch (err: any) {
    console.error('Falha ao salvar no IndexedDB v2:', err);
    return {
      success: false,
      timestamp,
      error: err?.message || 'Erro desconhecido ao gravar dados.',
    };
  }
}

/**
 * Loads project state from IndexedDB v2.
 * Automatically checks and migrates from v1 (projectStore) if needed.
 */
export async function loadProjectFromStorage(): Promise<ProjectState | null> {
  try {
    const db = await openDatabase();

    // Check v2 meta store first
    const meta: any = await new Promise((resolve) => {
      const tx = db.transaction(STORE_META, 'readonly');
      const store = tx.objectStore(STORE_META);
      const req = store.get(META_KEY);
      req.onsuccess = () => resolve(req.result || null);
      req.onerror = () => resolve(null);
    });

    if (meta) {
      // Load all tables from STORE_TABLES
      const tableRecords: ChunkedTableRecord[] = await new Promise((resolve) => {
        const tx = db.transaction(STORE_TABLES, 'readonly');
        const store = tx.objectStore(STORE_TABLES);
        const req = store.getAll();
        req.onsuccess = () => resolve(req.result || []);
        req.onerror = () => resolve([]);
      });

      // Reconstruct tables by assembling chunks
      const tables: Table[] = tableRecords.map(rec => {
        const rows: Record<string, any>[] = [];
        if (rec.chunks && Array.isArray(rec.chunks)) {
          rec.chunks.forEach(chunk => {
            rows.push(...chunk);
          });
        }
        return {
          id: rec.id,
          name: rec.name,
          originalName: rec.originalName,
          columns: rec.columns || [],
          rows,
          sourceConfig: rec.sourceConfig,
          isCalculated: rec.isCalculated,
          isCalendar: rec.isCalendar,
          isHidden: rec.isHidden,
          isDim: rec.isDim,
          x: rec.x,
          y: rec.y,
          duckdbTable: rec.duckdbTable,
          rowsTruncated: rec.rowsTruncated,
          totalRows: rec.duckdbTotalRows,
        };
      });

      return migrateProjectState({
        ...meta,
        tables,
      });
    }

    // Fallback: Check if legacy v1 projectStore exists
    if (db.objectStoreNames.contains(V1_LEGACY_STORE)) {
      const legacyState: any = await new Promise((resolve) => {
        const tx = db.transaction(V1_LEGACY_STORE, 'readonly');
        const store = tx.objectStore(V1_LEGACY_STORE);
        const req = store.get('current_project');
        req.onsuccess = () => resolve(req.result || null);
        req.onerror = () => resolve(null);
      });

      if (legacyState) {
        console.info('Migrando projeto do formato IndexedDB v1 para v2...');
        const migrated = migrateProjectState(legacyState);
        // Persist to v2 immediately
        await saveProjectToStorage(migrated);
        return migrated;
      }
    }

    return null;
  } catch (err) {
    console.warn('Erro ao carregar do IndexedDB, verificando backup legado:', err);
    try {
      const backup = localStorage.getItem('insighthub_project_backup');
      if (backup) {
        const parsed = JSON.parse(backup);
        return migrateProjectState(parsed);
      }
    } catch {
      return null;
    }
    return null;
  }
}

/**
 * Export project to a downloadable .insight file using Blob and URL.createObjectURL.
 * Avoids browser data-URI size limits and registers schema migration.
 */
export function exportProjectToFile(state: ProjectState, fileName = 'Projeto_InsightHub.insight'): void {
  const normalized = migrateProjectState(state);
  (normalized as any).appVersion = APP_VERSION;
  const jsonString = JSON.stringify(normalized, null, 2);
  const blob = new Blob([jsonString], { type: 'application/json' });
  const objectUrl = URL.createObjectURL(blob);

  const downloadAnchor = document.createElement('a');
  downloadAnchor.setAttribute('href', objectUrl);
  downloadAnchor.setAttribute('download', fileName);
  document.body.appendChild(downloadAnchor);
  downloadAnchor.click();
  downloadAnchor.remove();

  // Free allocated blob URL
  setTimeout(() => {
    URL.revokeObjectURL(objectUrl);
  }, 1000);

  // Salva também o cache colunar nativo (melhor esforço, em segundo
  // plano): o .insight exportado é só os metadados; o cache colunar é
  // quem permite reabrir sem reler as fontes (ver §B em docs/V011.md).
  // Nunca bloqueia o download nem propaga erro — se falhar, a próxima
  // abertura simplesmente releria as fontes (comportamento atual).
  void saveProjectCacheBestEffort(normalized);
}

/** Dispara `project_cache_save` fora do Tauri runtime é um no-op; dentro
 * dele, grava em segundo plano e nunca lança — falhas só geram um
 * `console.warn`, porque o cache é um acelerador, não a fonte da verdade
 * (os metadados + fontes originais continuam sendo a fonte da verdade). */
export async function saveProjectCacheBestEffort(state: ProjectState): Promise<void> {
  try {
    const { isTauriRuntime, projectCacheSave } = await import('../bridge/tauriBridge');
    if (!isTauriRuntime()) return;
    if (!state.projectId) return;
    const metas = buildTableCacheMetas(state.tables || []);
    if (metas.length === 0) return;
    await projectCacheSave(state.projectId, metas);
  } catch (err) {
    console.warn('Falha ao salvar o cache colunar do projeto (não bloqueante):', err);
  }
}

// ---------------------------------------------------------------------
// Cache colunar de projeto (v0.11 Frente B) — ver src-tauri/src/project_store
// e bridge/tauriBridge.ts (projectCacheSave/Load/Info). O cache guarda uma
// cópia materializada das tabelas (DuckDB nativo) ao lado dos metadados
// acima, para reabrir sem reler as fontes. Este arquivo só monta a
// identidade/assinatura do que entra no cache; quem chama o Tauri é o
// useBiStore (saveProject/loadStateFromProject).
// ---------------------------------------------------------------------

/** Garante que o projeto tem um `projectId` estável, gerando um na
 * primeira vez que for necessário (ex.: primeiro save depois do v0.11). */
export function ensureProjectId(state: ProjectState): ProjectState {
  if (state.projectId) return state;
  const id =
    typeof crypto !== 'undefined' && 'randomUUID' in crypto
      ? crypto.randomUUID()
      : `proj_${Date.now()}_${Math.random().toString(36).slice(2)}`;
  return { ...state, projectId: id };
}

/** Assinatura estável do schema de uma tabela (nome+tipo de cada coluna,
 * ordenada), usada para invalidar o cache quando o schema muda. */
function columnsSignature(table: Table): string {
  return (table.columns || [])
    .map(c => `${c.name}:${c.dataType}`)
    .sort()
    .join('|');
}

/** Assinatura da fonte de uma tabela (sem segredos: nunca inclui senha,
 * só o que já é público no projeto), usada para invalidar o cache quando
 * a fonte muda (arquivo diferente, outra aba, outra query M, etc.). */
function sourceSignature(table: Table): string {
  const sc = table.sourceConfig;
  if (!sc) return 'no-source';
  const parts = [
    sc.type,
    sc.filePath || '',
    sc.sheetName || '',
    sc.url || '',
    sc.pbipRoot || '',
    sc.mScript || '',
    sc.db ? `${sc.db.kind}:${JSON.stringify(sc.db.config || {})}:${JSON.stringify(sc.db.source || {})}` : '',
  ];
  return parts.join('|');
}

/** Monta a lista de metadados por tabela que o backend usa para validar
 * (ou gravar) o cache colunar — ver `NativeTableCacheMeta` no bridge. */
export function buildTableCacheMetas(
  tables: Table[]
): { name: string; columnsSignature: string; sourceSignature: string; rowCount: number }[] {
  return tables
    .filter(t => !!t.duckdbTable)
    .map(t => ({
      name: t.duckdbTable as string,
      columnsSignature: columnsSignature(t),
      sourceSignature: sourceSignature(t),
      rowCount: t.totalRows ?? (t.rows ? t.rows.length : 0),
    }));
}

