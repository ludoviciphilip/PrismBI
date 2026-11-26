import { create } from 'zustand';
import {
  ActiveView,
  Table,
  Column,
  TableSourceConfig,
  Relationship,
  Measure,
  Page,
  Visual,
  VisualType,
  VisualDataConfig,
  Filter,
  Bookmark,
  DashboardTile,
  Goal,
  RlsRole,
  WhatIfParameter,
  PowerQueryStep,
  PerformanceMetric,
  BiTheme,
  ActiveDrillthrough,
  CalculationGroup,
  UserDefinedFunction,
  DaxQueryTab,
  CanvasSizeConfig,
} from '../types/bi';
import { EtlQuery, EtlStep, EtlSource } from '../types/etl';
import { etlApply, etlFromM, etlLoadRaw, setBrowserRawProvider } from '../services/etl/etlClient';
import {
  buildSlicerFilterValue,
  findSyncSiblings,
  pageInSyncScope,
  resolveSlicerField,
  slicerFilterId,
} from '../components/report/slicer/slicerFilter';

export interface ApplyAllQueriesResult {
  applied: { tableId: string; name: string; rowCount: number; truncated: boolean; measuresOnly?: boolean }[];
  errors: { tableId: string; name: string; stepIndex?: number; message: string } | null;
}
import { tableToEtlSource } from '../services/etl/etlSource';
import { getSampleDatasets, generateCalendarTable } from '../data/sampleDatasets';
import { globalDataEngine } from '../engine/dataEngine';
import { DaxEngine } from '../engine/daxEngine';
import { saveProjectToStorage, loadProjectFromStorage, ProjectState } from '../engine/storage';
import { migrateProjectState, syncWhatIfWithModel } from '../engine/migrations';
import { globalDuckDb } from '../engine/duckdbEngine';
import { getEffectiveCanvasSize } from '../engine/shared/canvasPresets';
import {
  isTauriRuntime,
  syncModelToRust,
  refreshTableNative,
  refreshPbipTables,
  dbRefreshImport,
  NativeImportedTable,
  UpdateCheckResult,
} from '../bridge/tauriBridge';
import { buildTableFromDbImport } from '../adapters/dbConnector';

export interface RecentProjectEntry {
  id: string;
  name: string;
  type: 'pbip' | 'pbix' | 'insight';
  path?: string;
  openedAt: string;
}

const RECENT_PROJECTS_KEY = 'prismbi_recent_projects';

function loadRecentProjects(): RecentProjectEntry[] {
  try {
    if (typeof localStorage === 'undefined') return [];
    const raw = localStorage.getItem(RECENT_PROJECTS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.slice(0, 8) : [];
  } catch {
    return [];
  }
}

function saveRecentProjects(entries: RecentProjectEntry[]): void {
  try {
    if (typeof localStorage === 'undefined') return;
    localStorage.setItem(RECENT_PROJECTS_KEY, JSON.stringify(entries.slice(0, 8)));
  } catch {}
}
import {
  SupportedLocale,
  SupportedCurrency,
  setGlobalLocale,
  setGlobalCurrency,
} from '../engine/shared/formatter';
import {
  ThemeMode,
  getStoredThemeMode,
  setStoredThemeMode,
  applyThemeToDocument,
  resolveIsDark,
  initSystemThemeListener,
} from '../services/themeService';

export const PRESET_THEMES: BiTheme[] = [
  {
    id: 'powerbi-classic',
    name: 'Power BI Clássico (Âmbar/Azul)',
    palette: ['#f59e0b', '#3b82f6', '#10b981', '#8b5cf6', '#ec4899', '#06b6d4', '#64748b'],
    backgroundColor: '#ffffff',
    cardBackground: '#ffffff',
    borderColor: '#e2e8f0',
    textColor: '#1e293b',
  },
  {
    id: 'corporate-blue',
    name: 'Executivo Corporativo (Azul/Ciano)',
    palette: ['#1e3a8a', '#2563eb', '#38bdf8', '#0d9488', '#f59e0b', '#64748b', '#475569'],
    backgroundColor: '#f8fafc',
    cardBackground: '#ffffff',
    borderColor: '#cbd5e1',
    textColor: '#0f172a',
  },
  {
    id: 'emerald-growth',
    name: 'Esmeralda & Sustentabilidade (Verde)',
    palette: ['#065f46', '#059669', '#10b981', '#34d399', '#a7f3d0', '#f59e0b', '#0284c7'],
    backgroundColor: '#f0fdf4',
    cardBackground: '#ffffff',
    borderColor: '#bbf7d0',
    textColor: '#064e3b',
  },
  {
    id: 'purple-fintech',
    name: 'Finanças & Tecnologia (Roxo/Violeta)',
    palette: ['#581c87', '#7c3aed', '#a855f7', '#c084fc', '#f43f5e', '#fbbf24', '#06b6d4'],
    backgroundColor: '#faf5ff',
    cardBackground: '#ffffff',
    borderColor: '#e9d5ff',
    textColor: '#3b0764',
  },
  {
    id: 'dark-contrast',
    name: 'Alto Contraste Escuro (Neon)',
    palette: ['#fbbf24', '#38bdf8', '#4ade80', '#f472b6', '#a78bfa', '#f97316', '#34d399'],
    backgroundColor: '#0f172a',
    cardBackground: '#1e293b',
    borderColor: '#334155',
    textColor: '#f8fafc',
  },
  {
    id: 'warm-sunset',
    name: 'Pôr do Sol & Energia (Laranja/Rubi)',
    palette: ['#e11d48', '#ea580c', '#d97706', '#ca8a04', '#65a30d', '#0284c7', '#9333ea'],
    backgroundColor: '#fffbeb',
    cardBackground: '#ffffff',
    borderColor: '#fed7aa',
    textColor: '#7c2d12',
  },
  {
    id: 'ocean-depths',
    name: 'Oceano Profundo (Teal/Turquesa)',
    palette: ['#0f172a', '#1e293b', '#0369a1', '#0ea5e9', '#38bdf8', '#06b6d4', '#14b8a6'],
    backgroundColor: '#f0f9ff',
    cardBackground: '#ffffff',
    borderColor: '#bae6fd',
    textColor: '#0c4a6e',
  },
];

interface BiState {
  // Navigation & View
  activeView: ActiveView;
  setActiveView: (view: ActiveView) => void;

  // Project Info
  projectName: string;
  setProjectName: (name: string) => void;
  lastSavedAt: string | null;
  dataVersion: number;
  sourcePbipPath: string | null;
  isRefreshingData: boolean;
  refreshStatus: { type: 'success' | 'warning' | 'error'; message: string; timestamp: string } | null;
  setRefreshStatus: (status: { type: 'success' | 'warning' | 'error'; message: string; timestamp: string } | null) => void;
  replaceTargetTableId: string | null;
  setReplaceTargetTableId: (tableId: string | null) => void;
  openReplaceTableModal: (tableId: string) => void;

  // Schema
  tables: Table[];
  relationships: Relationship[];
  measures: Measure[];
  setTables: (tables: Table[]) => void;
  addTable: (table: Table) => void;
  addTables: (tables: Table[]) => void;
  updateTable: (id: string, partial: Partial<Table>) => void;
  deleteTable: (id: string) => void;
  duplicateTable: (id: string) => Table | undefined;
  renameTable: (tableId: string, newName: string) => { ok: boolean; error?: string };
  replaceTableData: (
    targetTableId: string,
    newColumns: Column[],
    newRows: Record<string, any>[],
    newSourceConfig?: TableSourceConfig,
    meta?: { duckdbTable?: string; rowsTruncated?: boolean; totalRows?: number }
  ) => boolean;
  refreshTable: (tableId: string) => Promise<{ ok: boolean; message: string }>;
  refreshAllData: () => Promise<{ refreshedTables: string[]; warnings: string[]; error?: string }>;

  addRelationship: (rel: Relationship) => void;
  deleteRelationship: (id: string) => void;

  addMeasure: (measure: Measure) => void;
  updateMeasure: (id: string, partial: Partial<Measure>) => void;
  deleteMeasure: (id: string) => void;
  editingMeasureId: string | null;
  setEditingMeasureId: (id: string | null) => void;
  openCustomMeasureModal: (measureId?: string) => void;

  // Pages & Visuals
  pages: Page[];
  activePageId: string;
  selectedVisualId: string | null;
  selectedVisualIds: string[];
  canvasZoom: number;
  isPresentationMode: boolean;
  layoutMode: 'desktop' | 'mobile';
  historyPast: Page[][];
  historyFuture: Page[][];
  canUndo: boolean;
  canRedo: boolean;

  setActivePageId: (id: string) => void;
  addPage: (name?: string) => void;
  renamePage: (id: string, newName: string) => void;
  duplicatePage: (id: string) => void;
  deletePage: (id: string) => void;
  setPageCanvasSize: (pageId: string, canvasSize: CanvasSizeConfig, applyToAllPages?: boolean) => void;
  setSelectedVisualId: (id: string | null) => void;
  setSelectedVisualIds: (ids: string[]) => void;
  toggleVisualSelection: (id: string) => void;
  setCanvasZoom: (zoom: number) => void;
  setIsPresentationMode: (val: boolean) => void;
  setLayoutMode: (mode: 'desktop' | 'mobile') => void;

  addVisual: (type: VisualType, tableId?: string) => void;
  updateVisual: (id: string, partial: Partial<Visual>) => void;
  deleteVisual: (id: string) => void;
  moveVisualsDelta: (ids: string[], dx: number, dy: number) => void;
  deleteSelectedVisuals: () => void;
  alignSelectedVisuals: (
    mode: 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom' | 'distribute-h' | 'distribute-v'
  ) => void;
  undo: () => void;
  redo: () => void;

  // Filtering & Cross-filtering
  filters: Filter[];
  addFilter: (filter: Filter) => void;
  removeFilter: (id: string) => void;
  clearFilters: () => void;
  setCrossFilter: (tableId: string, column: string, value: any) => void;
  /** Sets a slicer visual's selection (list/between/relative), computes its Filter,
   *  and — when the visual has a syncGroup — propagates the same selection to every
   *  other slicer visual sharing that group on the pages allowed by its syncPages. */
  setSlicerSelection: (
    visualId: string,
    selection: Partial<Pick<NonNullable<VisualDataConfig['slicerConfig']>, 'selectedValues' | 'between' | 'relative'>>
  ) => void;
  /** Re-resolves every 'relative-date' slicer's filter against "today" (called on project load). */
  recalculateRelativeDateFilters: () => void;

  // Drill-through
  activeDrillthrough: ActiveDrillthrough | null;
  executeDrillthrough: (targetPageId: string, field: string, value: any, sourceVisualTitle?: string) => void;
  clearDrillthrough: (returnToSource?: boolean) => void;

  // Bookmarks
  bookmarks: Bookmark[];
  addBookmark: (name: string) => void;
  applyBookmark: (id: string) => void;
  deleteBookmark: (id: string) => void;

  // Dashboards & Goals
  dashboardTiles: DashboardTile[];
  goals: Goal[];
  pinVisualToDashboard: (visualId: string, pageId: string, title?: string) => void;
  removeDashboardTile: (tileId: string) => void;
  addGoal: (goal: Goal) => void;
  updateGoal: (id: string, partial: Partial<Goal>) => void;

  // Power Query / ETL
  appliedSteps: Record<string, EtlQuery>;
  /** Snapshot da última EtlQuery efetivamente aplicada por tabela — usado para detectar "consultas sujas". */
  appliedQuerySnapshot: Record<string, EtlQuery>;
  selectedStepByTable: Record<string, string | undefined>;
  getOrInitQuery: (tableId: string) => EtlQuery;
  /** Como getOrInitQuery, mas converte o M de tabelas .pbip em etapas (etl_from_m) e persiste a consulta. */
  ensureQuery: (tableId: string) => Promise<EtlQuery>;
  /** "Atualizar" de uma tabela com consulta ETL: relê a fonte bruta e reaplica as etapas. */
  refreshEtlTable: (tableId: string) => Promise<void>;
  setQuery: (tableId: string, query: EtlQuery) => void;
  addQueryStep: (tableId: string, step: EtlStep) => void;
  updateQueryStep: (tableId: string, stepId: string, patch: Partial<EtlStep>) => void;
  removeQueryStep: (tableId: string, stepId: string) => void;
  moveQueryStepUp: (tableId: string, stepId: string) => void;
  moveQueryStepDown: (tableId: string, stepId: string) => void;
  renameQueryStep: (tableId: string, stepId: string, name: string) => void;
  selectQueryStep: (tableId: string, stepId: string | undefined) => void;
  closeAndApplyQuery: (tableId: string) => Promise<{ rowCount: number; truncated: boolean; measuresOnly: boolean }>;
  refreshTableData: (tableId: string) => Promise<void>;
  /** true se as etapas da tabela mudaram desde a última aplicação bem-sucedida. */
  isQueryDirty: (tableId: string) => boolean;
  /** true se QUALQUER tabela com consulta Power Query tem alterações pendentes. */
  hasPendingQueryChanges: () => boolean;
  /**
   * Aplica TODAS as consultas sujas, em ordem de dependência (mergeQueries/appendQueries
   * primeiro), reportando progresso via onProgress. Para no primeiro erro — mantém as
   * tabelas já aplicadas com sucesso e devolve qual consulta/etapa falhou.
   */
  applyAllDirtyQueries: (onProgress?: (info: { name: string; index: number; total: number }) => void) => Promise<ApplyAllQueriesResult>;

  // Governance & RLS
  rlsRoles: RlsRole[];
  activeRole: string | null;
  setActiveRole: (roleId: string | null) => void;
  calculationGroups: CalculationGroup[];
  setCalculationGroups: (groups: CalculationGroup[]) => void;
  activeUser: {
    username: string;
    userPrincipalName: string;
    userObjectId: string;
    userCulture: string;
    customData: string;
  };
  setActiveUser: (user: {
    username: string;
    userPrincipalName: string;
    userObjectId: string;
    userCulture: string;
    customData: string;
  }) => void;
  whatIfParameters: WhatIfParameter[];
  updateWhatIfValue: (id: string, value: number) => void;
  addWhatIfParameter: (param: WhatIfParameter) => void;

  // User Defined Functions (UDF)
  userDefinedFunctions: UserDefinedFunction[];
  setUserDefinedFunctions: (udfs: UserDefinedFunction[]) => void;
  addUserDefinedFunction: (udf: UserDefinedFunction) => void;
  updateUserDefinedFunction: (id: string, partial: Partial<UserDefinedFunction>) => void;
  deleteUserDefinedFunction: (id: string) => void;

  // DAX Query Tabs
  daxQueries: DaxQueryTab[];
  activeDaxQueryTabId: string;
  setDaxQueries: (queries: DaxQueryTab[]) => void;
  setActiveDaxQueryTabId: (id: string) => void;
  addDaxQueryTab: (title?: string, initialQuery?: string) => string;
  closeDaxQueryTab: (id: string) => void;
  updateDaxQueryText: (id: string, query: string) => void;

  // Performance Analyzer
  performanceMetrics: PerformanceMetric[];
  recordPerformanceMetric: (metric: PerformanceMetric) => void;

  // Theme & Appearance (Gerenciador de Temas & Dark Mode)
  theme: string;
  activeThemeId: string;
  customThemes: BiTheme[];
  activePalette: string[];
  themeModalOpen: boolean;
  themeMode: ThemeMode;
  isDarkMode: boolean;
  setThemeModalOpen: (open: boolean) => void;
  setTheme: (theme: string) => void;
  setThemeId: (themeId: string) => void;
  setThemeMode: (mode: ThemeMode) => void;
  toggleThemeMode: () => void;
  addCustomTheme: (theme: BiTheme) => void;
  deleteCustomTheme: (themeId: string) => void;

  // Grid & Canvas Settings
  gridSize: number;
  setGridSize: (size: number) => void;
  showGrid: boolean;
  setShowGrid: (show: boolean) => void;
  snapToGrid: boolean;
  setSnapToGrid: (snap: boolean) => void;

  // Modals
  dataSourceModalOpen: boolean;
  setDataSourceModalOpen: (open: boolean) => void;
  quickMeasureModalOpen: boolean;
  setQuickMeasureModalOpen: (open: boolean) => void;
  customMeasureModalOpen: boolean;
  setCustomMeasureModalOpen: (open: boolean) => void;
  limitationsModalOpen: boolean;
  setLimitationsModalOpen: (open: boolean) => void;
  aiModalOpen: boolean;
  setAiModalOpen: (open: boolean) => void;
  aiSettingsModalOpen: boolean;
  setAiSettingsModalOpen: (open: boolean) => void;
  canvasSizeModalOpen: boolean;
  setCanvasSizeModalOpen: (open: boolean) => void;
  updateModalOpen: boolean;
  setUpdateModalOpen: (open: boolean) => void;
  availableUpdate: UpdateCheckResult | null;
  setAvailableUpdate: (update: UpdateCheckResult | null) => void;

  // Welcome Screen & Recent Projects
  showWelcomeScreen: boolean;
  setShowWelcomeScreen: (show: boolean) => void;
  recentProjects: RecentProjectEntry[];
  addRecentProject: (entry: RecentProjectEntry) => void;
  removeRecentProject: (id: string) => void;

  // Language & Currency (i18n)
  language: SupportedLocale;
  currency: SupportedCurrency;
  setLanguage: (lang: SupportedLocale) => void;
  setCurrency: (currency: SupportedCurrency) => void;

  // State actions
  newProject: (name?: string) => void;
  executeDuckDbQuery: (sql: string) => Promise<any[]>;
  loadInitialSampleData: () => void;
  generateCalendar: (startDate?: string, endDate?: string) => void;
  saveProject: () => Promise<void>;
  loadProject: () => Promise<boolean>;
  getProjectState: () => ProjectState;
  loadStateFromProject: (state: ProjectState) => void;
}

const defaultPageId = 'page_executivo';

/** "Linhas Filtradas" -> "Linhas Filtradas1" se já existir (como o Power BI). */
export function uniqueStepName(name: string, existing: string[]): string {
  const taken = new Set(existing.map(n => n.toLowerCase()));
  if (!taken.has(name.toLowerCase()) && name.toLowerCase() !== 'source') return name;
  for (let i = 1; ; i++) {
    const candidate = `${name}${i}`;
    if (!taken.has(candidate.toLowerCase())) return candidate;
  }
}

/** Nenhuma chamada ao backend pode deixar "Atualizando..." preso para sempre. */
const REFRESH_STEP_TIMEOUT_MS = 120_000;
function withTimeout<T>(p: Promise<T>, label: string, ms = REFRESH_STEP_TIMEOUT_MS): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${label}: sem resposta em ${Math.round(ms / 1000)}s`)), ms);
  });
  return Promise.race([p, timeout]).finally(() => clearTimeout(timer)) as Promise<T>;
}

/** A tabela tem uma consulta ETL que precisa ser reexecutada na atualização. */
function hasEtlQuery(q: EtlQuery | undefined): q is EtlQuery {
  return !!q && (q.steps.length > 0 || q.source.kind !== 'inline');
}


const initialVisuals: Visual[] = [
  {
    id: 'vis_kpi_fat',
    type: 'card',
    title: 'Faturamento Total (R$)',
    x: 20,
    y: 20,
    w: 270,
    h: 120,
    dataConfig: {
      tableId: 'f_Vendas',
      yAxisFields: ['ValorTotal'],
      aggregation: 'SUM',
    },
    formatConfig: {
      showTitle: true,
      title: 'Faturamento Total Realizado',
      showDataLabels: true,
      backgroundColor: '#ffffff',
      borderColor: '#e2e8f0',
      borderRadius: 8,
    },
  },
  {
    id: 'vis_kpi_lucro',
    type: 'card',
    title: 'Lucro Bruto (R$)',
    x: 310,
    y: 20,
    w: 270,
    h: 120,
    dataConfig: {
      tableId: 'f_Vendas',
      yAxisFields: ['MargemLucro'],
      aggregation: 'SUM',
    },
    formatConfig: {
      showTitle: true,
      title: 'Margem Bruta Acumulada',
      showDataLabels: true,
      backgroundColor: '#ffffff',
      borderColor: '#e2e8f0',
      borderRadius: 8,
    },
  },
  {
    id: 'vis_kpi_margempct',
    type: 'card',
    title: 'Margem Bruta %',
    x: 600,
    y: 20,
    w: 270,
    h: 120,
    dataConfig: {
      tableId: 'f_Vendas',
      yAxisFields: ['MargemPercentual'],
      aggregation: 'AVG',
    },
    formatConfig: {
      showTitle: true,
      title: 'Margem Média Percentual',
      showDataLabels: true,
      backgroundColor: '#ffffff',
      borderColor: '#e2e8f0',
      borderRadius: 8,
    },
  },
  {
    id: 'vis_slicer_canal',
    type: 'slicer',
    title: 'Segmentação: Canal',
    x: 890,
    y: 20,
    w: 260,
    h: 120,
    dataConfig: {
      tableId: 'f_Vendas',
      xAxisField: 'Canal',
      slicerConfig: {
        mode: 'dropdown',
        selectedValues: [],
      },
    },
    formatConfig: {
      showTitle: true,
      title: 'Filtrar Canal de Venda',
      backgroundColor: '#ffffff',
      borderColor: '#e2e8f0',
      borderRadius: 8,
    },
  },
  {
    id: 'vis_col_categoria',
    type: 'column',
    title: 'Faturamento por Categoria de Produto',
    x: 20,
    y: 160,
    w: 560,
    h: 300,
    dataConfig: {
      tableId: 'f_Vendas',
      xAxisField: 'Categoria',
      yAxisFields: ['ValorTotal'],
      aggregation: 'SUM',
      sortField: 'ValorTotal',
      sortOrder: 'desc',
    },
    formatConfig: {
      showTitle: true,
      showLegend: false,
      showDataLabels: true,
      backgroundColor: '#ffffff',
      borderColor: '#e2e8f0',
      borderRadius: 8,
    },
  },
  {
    id: 'vis_line_evolucao',
    type: 'line',
    title: 'Evolução Mensal do Faturamento (2025)',
    x: 600,
    y: 160,
    w: 550,
    h: 300,
    dataConfig: {
      tableId: 'f_Vendas',
      xAxisField: 'Mes',
      yAxisFields: ['ValorTotal'],
      aggregation: 'SUM',
    },
    formatConfig: {
      showTitle: true,
      showDataLabels: false,
      backgroundColor: '#ffffff',
      borderColor: '#e2e8f0',
      borderRadius: 8,
    },
  },
  {
    id: 'vis_pie_regiao',
    type: 'donut',
    title: 'Distribuição Geográfica por Região',
    x: 20,
    y: 480,
    w: 370,
    h: 270,
    dataConfig: {
      tableId: 'f_Vendas',
      xAxisField: 'Regiao',
      yAxisFields: ['ValorTotal'],
      aggregation: 'SUM',
    },
    formatConfig: {
      showTitle: true,
      showLegend: true,
      backgroundColor: '#ffffff',
      borderColor: '#e2e8f0',
      borderRadius: 8,
    },
  },
  {
    id: 'vis_bar_vendedores',
    type: 'bar',
    title: 'Top Vendedores por Desempenho',
    x: 410,
    y: 480,
    w: 360,
    h: 270,
    dataConfig: {
      tableId: 'f_Vendas',
      xAxisField: 'Vendedor',
      yAxisFields: ['ValorTotal'],
      aggregation: 'SUM',
      sortOrder: 'desc',
    },
    formatConfig: {
      showTitle: true,
      showLegend: false,
      backgroundColor: '#ffffff',
      borderColor: '#e2e8f0',
      borderRadius: 8,
    },
  },
  {
    id: 'vis_table_transacoes',
    type: 'table',
    title: 'Detalhamento de Vendas Recentes',
    x: 790,
    y: 480,
    w: 360,
    h: 270,
    dataConfig: {
      tableId: 'f_Vendas',
      limit: 25,
    },
    formatConfig: {
      showTitle: true,
      backgroundColor: '#ffffff',
      borderColor: '#e2e8f0',
      borderRadius: 8,
    },
  },
];

const initialGoals: Goal[] = [
  {
    id: 'goal_fat_trimestral',
    name: 'Meta Faturamento 1º Semestre',
    target: 1500000,
    current: 1280000,
    unit: 'R$',
    owner: 'Camila Santos (Diretora Comercial)',
    deadline: '30/06/2025',
    status: 'on_track',
    history: [
      { date: 'Jan/25', value: 380000 },
      { date: 'Fev/25', value: 410000 },
      { date: 'Mar/25', value: 490000 },
    ],
  },
  {
    id: 'goal_margem_ebitda',
    name: 'EBITDA Acumulado',
    target: 350000,
    current: 295000,
    unit: 'R$',
    owner: 'Rodrigo Pires (CFO)',
    deadline: '30/06/2025',
    status: 'on_track',
    history: [
      { date: 'Jan/25', value: 85000 },
      { date: 'Fev/25', value: 102000 },
      { date: 'Mar/25', value: 108000 },
    ],
  },
  {
    id: 'goal_custo_km',
    name: 'Custo Médio por Km na Logística',
    target: 3.20,
    current: 3.48,
    unit: 'R$',
    owner: 'Carlos Silva (Gerente Logística)',
    deadline: '31/05/2025',
    status: 'at_risk',
    history: [
      { date: 'Jan/25', value: 3.65 },
      { date: 'Fev/25', value: 3.52 },
      { date: 'Mar/25', value: 3.48 },
    ],
  },
];

const initialWhatIf: WhatIfParameter[] = [
  {
    id: 'p_reajuste_preco',
    name: 'Reajuste Médio de Preço (%)',
    min: -20,
    max: 30,
    step: 1,
    defaultValue: 0,
    currentValue: 0,
    unit: '%',
  },
  {
    id: 'p_meta_crescimento',
    name: 'Crescimento de Volume Projetado (%)',
    min: -15,
    max: 50,
    step: 5,
    defaultValue: 10,
    currentValue: 10,
    unit: '%',
  },
];

const savedLang = (typeof localStorage !== 'undefined' ? localStorage.getItem('insighthub_language') : null) as SupportedLocale | null;
const initialLanguage: SupportedLocale = savedLang && (savedLang === 'pt-BR' || savedLang === 'en-US' || savedLang === 'es-ES') ? savedLang : 'pt-BR';

const savedCurr = (typeof localStorage !== 'undefined' ? localStorage.getItem('insighthub_currency') : null) as SupportedCurrency | null;
const initialCurrency: SupportedCurrency = savedCurr && (savedCurr === 'BRL' || savedCurr === 'USD' || savedCurr === 'EUR') ? savedCurr : 'BRL';

setGlobalLocale(initialLanguage);
setGlobalCurrency(initialCurrency);

const initialThemeMode = getStoredThemeMode();
const initialIsDark = applyThemeToDocument(initialThemeMode);

async function pushModelToRustAsync(
  tables: Table[],
  relationships: Relationship[],
  measures: Measure[],
  onSynced?: () => void
) {
  if (!isTauriRuntime()) return;
  try {
    await syncModelToRust({
      tables: tables.map(t => ({
        name: t.name,
        columns: t.columns.map(c => c.name),
        // DuckDB-backed tables (duckdbTable set): Rust loads every row
        // straight from DuckDB and ignores `rows`. The preview rows still go
        // along as the fallback for when the DuckDB table does not exist
        // (DuckDB is in-memory: right after reopening a project). Sending []
        // there would leave the table EMPTY in the model.
        rows: t.rows || [],
        columnTypes: t.columns.map(c => c.dataType ?? null),
        duckdbTable: t.duckdbTable,
      })),
      relationships: relationships.map(r => ({
        fromTable: r.fromTable,
        fromColumn: r.fromColumn,
        toTable: r.toTable,
        toColumn: r.toColumn,
        crossFilterDirection: r.crossFilterDirection,
        isActive: r.isActive,
      })),
      measures: measures.map(m => ({
        name: m.name,
        tableName: m.tableName,
        formula: m.formula,
      })),
    });
    onSynced?.();
  } catch {
    // Ignore bridge errors when running outside Tauri
  }
}

function escapeRegExp(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function formatDaxTableToken(tableName: string): string {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(tableName)
    ? tableName
    : `'${tableName.replace(/'/g, "''")}'`;
}

function rewriteTableRefsInDax(formula: string, oldNames: string[], newName: string): string {
  if (!formula) return formula;
  let updated = formula;
  const formattedNew = formatDaxTableToken(newName);
  for (const oldName of oldNames) {
    if (!oldName || oldName === newName) continue;
    const esc = escapeRegExp(oldName);
    // Quoted table reference followed by column bracket: 'Old Name'[Col]
    updated = updated.replace(new RegExp(`'${esc}'\\[`, 'gi'), `${formattedNew}[`);
    // Unquoted table reference followed by column bracket: OldName[Col]
    updated = updated.replace(new RegExp(`(^|[^A-Za-z0-9_'])${esc}\\[`, 'gi'), `$1${formattedNew}[`);
    // Table function argument like ALL('Old Name') or ALL(OldName)
    updated = updated.replace(
      new RegExp(`(\\b(?:ALL|ALLSELECTED|ALLEXCEPT|VALUES|DISTINCT)\\s*\\(\\s*)(?:'${esc}'|${esc})(\\s*[\\),])`, 'gi'),
      `$1${formattedNew}$2`
    );
  }
  return updated;
}

function mergeRefreshedColumns(
  incomingColumns: Column[],
  existingColumns: Column[],
  tableId: string
): Column[] {
  const existingMap = new Map(existingColumns.map(c => [c.name.toLowerCase(), c]));
  const merged = incomingColumns.map((col, idx) => {
    const prev = existingMap.get(col.name.toLowerCase());
    if (!prev) {
      return {
        ...col,
        id: col.id || `${tableId}_col_${idx}`,
      };
    }
    return {
      ...col,
      id: prev.id,
      format: col.format || prev.format,
      sortByColumn: col.sortByColumn || prev.sortByColumn,
      isHidden: prev.isHidden,
      description: prev.description,
      dataCategory: prev.dataCategory,
      dataType: prev.dataType !== 'text' || col.dataType === 'text' ? (col.dataType || prev.dataType) : col.dataType,
    };
  });
  // Keep calculated columns defined on the table if any
  for (const prev of existingColumns) {
    if (prev.isCalculated && !merged.some(m => m.name.toLowerCase() === prev.name.toLowerCase())) {
      merged.push(prev);
    }
  }
  return merged;
}

function nativePayloadToColumns(payload: NativeImportedTable, tableId: string): Column[] {
  return payload.columns.map((c, idx) => ({
    id: `${tableId}_col_${idx}_${c.name}`,
    name: c.name,
    originalName: c.originalName,
    dataType: c.dataType || 'text',
  }));
}

function inferColumnsFromJsonRows(rows: Record<string, any>[], tableId: string): Column[] {
  if (!rows || rows.length === 0) return [];
  const keys = Array.from(new Set(rows.slice(0, 50).flatMap(r => Object.keys(r || {}))));
  return keys.map((key, idx) => {
    const sample = rows.find(r => r[key] !== null && r[key] !== undefined && r[key] !== '')?.[key];
    let dataType: Column['dataType'] = 'text';
    if (typeof sample === 'number') dataType = 'number';
    else if (typeof sample === 'boolean') dataType = 'boolean';
    else if (typeof sample === 'string' && /^\d{4}-\d{2}-\d{2}/.test(sample)) dataType = 'date';
    return {
      id: `${tableId}_col_${idx}_${key}`,
      name: key,
      originalName: key,
      dataType,
    };
  });
}

const MAX_CANVAS_HISTORY = 50;

function clonePagesSnapshot(pages: Page[]): Page[] {
  return pages.map(p => ({
    ...p,
    canvasSize: p.canvasSize ? { ...p.canvasSize } : undefined,
    visuals: p.visuals.map(v => ({
      ...v,
      dataConfig: {
        ...v.dataConfig,
        yAxisFields: v.dataConfig.yAxisFields ? [...v.dataConfig.yAxisFields] : undefined,
        tooltipFields: v.dataConfig.tooltipFields ? [...v.dataConfig.tooltipFields] : undefined,
        drillFields: v.dataConfig.drillFields ? [...v.dataConfig.drillFields] : undefined,
        visualCalculations: v.dataConfig.visualCalculations
          ? v.dataConfig.visualCalculations.map(c => ({ ...c }))
          : undefined,
        slicerConfig: v.dataConfig.slicerConfig
          ? {
              ...v.dataConfig.slicerConfig,
              selectedValues: v.dataConfig.slicerConfig.selectedValues
                ? [...v.dataConfig.slicerConfig.selectedValues]
                : undefined,
              dateRange: v.dataConfig.slicerConfig.dateRange
                ? { ...v.dataConfig.slicerConfig.dateRange }
                : undefined,
            }
          : undefined,
      },
      formatConfig: {
        ...v.formatConfig,
        colorPalette: v.formatConfig.colorPalette ? [...v.formatConfig.colorPalette] : undefined,
        conditionalFormatting: v.formatConfig.conditionalFormatting
          ? {
              ...v.formatConfig.conditionalFormatting,
              rules: v.formatConfig.conditionalFormatting.rules.map(r => ({ ...r })),
            }
          : undefined,
      },
      visualCalculations: v.visualCalculations ? v.visualCalculations.map(c => ({ ...c })) : undefined,
    })),
  }));
}

export const useBiStore = create<BiState>((set, get) => {
  const initialData = getSampleDatasets();
  const initialSynced = syncWhatIfWithModel(initialWhatIf, initialData.tables, initialData.measures);
  globalDataEngine.setSchema(initialSynced.tables, initialData.relationships, initialSynced.measures);

  const pushPagesHistory = (nextPages: Page[], extra: Partial<BiState> = {}) => {
    const past = [...get().historyPast, clonePagesSnapshot(get().pages)].slice(-MAX_CANVAS_HISTORY);
    set({
      pages: nextPages,
      historyPast: past,
      historyFuture: [],
      canUndo: past.length > 0,
      canRedo: false,
      ...extra,
    });
  };

  return {
    language: initialLanguage,
    currency: initialCurrency,
    setLanguage: (lang: SupportedLocale) => {
      setGlobalLocale(lang);
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem('insighthub_language', lang);
      }
      set({ language: lang });
    },
    setCurrency: (currency: SupportedCurrency) => {
      setGlobalCurrency(currency);
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem('insighthub_currency', currency);
      }
      set({ currency });
    },

    activeView: 'report',
    setActiveView: (view) => set({ activeView: view }),

    projectName: 'Relatório Executivo Geral - InsightHub',
    setProjectName: (name) => set({ projectName: name }),
    lastSavedAt: null,
    dataVersion: 0,
    sourcePbipPath: null,
    isRefreshingData: false,
    refreshStatus: null,
    setRefreshStatus: (refreshStatus) => set({ refreshStatus }),
    replaceTargetTableId: null,
    setReplaceTargetTableId: (replaceTargetTableId) => set({ replaceTargetTableId }),
    openReplaceTableModal: (tableId) =>
      set({
        replaceTargetTableId: tableId,
        selectedVisualId: null,
        dataSourceModalOpen: true,
      }),

    tables: initialSynced.tables,
    relationships: initialData.relationships,
    measures: initialSynced.measures,

    setTables: (tables) => {
      set(s => ({ tables, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(tables, get().relationships, get().measures);
      tables.forEach(t => globalDuckDb.registerTable(t));
      pushModelToRustAsync(tables, get().relationships, get().measures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    addTable: (table) => {
      const newTables = [...get().tables, table];
      set(s => ({ tables: newTables, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(newTables, get().relationships, get().measures);
      globalDuckDb.registerTable(table);
      pushModelToRustAsync(newTables, get().relationships, get().measures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    addTables: (newTbls) => {
      const newTables = [...get().tables, ...newTbls];
      set(s => ({ tables: newTables, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(newTables, get().relationships, get().measures);
      newTbls.forEach(t => globalDuckDb.registerTable(t));
      pushModelToRustAsync(newTables, get().relationships, get().measures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    updateTable: (id, partial) => {
      const newTables = get().tables.map(t => (t.id === id ? { ...t, ...partial } : t));
      set(s => ({ tables: newTables, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(newTables, get().relationships, get().measures);
      const updated = newTables.find(t => t.id === id);
      if (updated) globalDuckDb.registerTable(updated);
      pushModelToRustAsync(newTables, get().relationships, get().measures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    deleteTable: (id) => {
      const target = get().tables.find(t => t.id === id || t.name === id);
      const matchNames = new Set([id, target?.id, target?.name].filter(Boolean) as string[]);
      const newTables = get().tables.filter(t => !matchNames.has(t.id) && !matchNames.has(t.name));
      const newRels = get().relationships.filter(
        r => !matchNames.has(r.fromTable) && !matchNames.has(r.toTable)
      );
      set(s => ({ tables: newTables, relationships: newRels, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(newTables, newRels, get().measures);
      pushModelToRustAsync(newTables, newRels, get().measures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    renameTable: (tableId, newName) => {
      const trimmed = newName.trim();
      if (!trimmed) {
        return { ok: false, error: 'O nome da tabela não pode ficar em branco.' };
      }
      const target = get().tables.find(t => t.id === tableId || t.name === tableId);
      if (!target) {
        return { ok: false, error: 'Tabela não encontrada no modelo.' };
      }
      const oldNames = Array.from(new Set([target.id, target.name]));
      const collision = get().tables.some(
        t => t.id !== target.id && (t.name.toLowerCase() === trimmed.toLowerCase() || t.id.toLowerCase() === trimmed.toLowerCase())
      );
      if (collision) {
        return { ok: false, error: `Já existe outra tabela chamada "${trimmed}".` };
      }

      const isOldMatch = (ref?: string | null) =>
        !!ref && oldNames.some(o => o.toLowerCase() === ref.toLowerCase());

      const renameQualifiedField = (field?: string) => {
        if (!field) return field;
        for (const o of oldNames) {
          if (field.toLowerCase().startsWith(`${o.toLowerCase()}.`)) {
            return `${trimmed}${field.slice(o.length)}`;
          }
        }
        return field;
      };

      const newTables = get().tables.map(t =>
        t.id === target.id
          ? {
              ...t,
              id: trimmed,
              name: trimmed,
            }
          : t
      );

      const newRels = get().relationships.map(r => ({
        ...r,
        fromTable: isOldMatch(r.fromTable) ? trimmed : r.fromTable,
        toTable: isOldMatch(r.toTable) ? trimmed : r.toTable,
      }));

      const newMeasures = get().measures.map(m => ({
        ...m,
        tableName: isOldMatch(m.tableName) ? trimmed : m.tableName,
        tableId: isOldMatch(m.tableId) ? trimmed : m.tableId,
        formula: rewriteTableRefsInDax(m.formula, oldNames, trimmed),
      }));

      const newPages = get().pages.map(p => ({
        ...p,
        visuals: p.visuals.map(v => {
          const dc = v.dataConfig;
          const updatedDc = {
            ...dc,
            tableId: isOldMatch(dc.tableId) ? trimmed : dc.tableId,
            xAxisField: renameQualifiedField(dc.xAxisField),
            legendField: renameQualifiedField(dc.legendField),
            sortField: renameQualifiedField(dc.sortField),
            yAxisFields: dc.yAxisFields?.map(f => renameQualifiedField(f) || f),
            drillFields: dc.drillFields?.map(f => renameQualifiedField(f) || f),
            tooltipFields: dc.tooltipFields?.map(f => renameQualifiedField(f) || f),
          };
          return { ...v, dataConfig: updatedDc };
        }),
      }));

      const newFilters = get().filters.map(f => ({
        ...f,
        tableId: isOldMatch(f.tableId) ? trimmed : f.tableId,
      }));

      const newRlsRoles = get().rlsRoles.map(role => ({
        ...role,
        rules: role.rules.map(rule => ({
          ...rule,
          tableId: isOldMatch(rule.tableId) ? trimmed : rule.tableId,
        })),
      }));

      const nextSteps = { ...get().appliedSteps };
      if (nextSteps[target.id] && target.id !== trimmed) {
        nextSteps[trimmed] = nextSteps[target.id];
        delete nextSteps[target.id];
      }

      set(s => ({
        tables: newTables,
        relationships: newRels,
        measures: newMeasures,
        pages: newPages,
        filters: newFilters,
        rlsRoles: newRlsRoles,
        appliedSteps: nextSteps,
        dataVersion: s.dataVersion + 1,
      }));

      globalDataEngine.setSchema(newTables, newRels, newMeasures);
      const renamedTable = newTables.find(t => t.id === trimmed);
      if (renamedTable) globalDuckDb.registerTable(renamedTable);
      pushModelToRustAsync(newTables, newRels, newMeasures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );

      return { ok: true };
    },

    replaceTableData: (targetTableId, newColumns, newRows, newSourceConfig, meta) => {
      const target = get().tables.find(
        t =>
          t.id === targetTableId ||
          t.name === targetTableId ||
          t.name.toLowerCase() === targetTableId.toLowerCase()
      );
      if (!target) return false;

      const mergedColumns = mergeRefreshedColumns(newColumns, target.columns, target.id);
      const nowIso = new Date().toISOString();
      const updatedSourceConfig: TableSourceConfig | undefined = newSourceConfig
        ? {
            ...target.sourceConfig,
            ...newSourceConfig,
            lastRefreshedAt: nowIso,
          }
        : target.sourceConfig
        ? {
            ...target.sourceConfig,
            lastRefreshedAt: nowIso,
          }
        : undefined;

      const newTables = get().tables.map(t =>
        t.id === target.id
          ? {
              ...t,
              columns: mergedColumns,
              rows: newRows,
              sourceConfig: updatedSourceConfig,
              duckdbTable: meta?.duckdbTable ?? t.duckdbTable,
              rowsTruncated: meta?.rowsTruncated ?? t.rowsTruncated,
              totalRows: meta?.totalRows ?? t.totalRows,
            }
          : t
      );

      set(s => ({ tables: newTables, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(newTables, get().relationships, get().measures);
      const updatedTable = newTables.find(t => t.id === target.id);
      if (updatedTable) globalDuckDb.registerTable(updatedTable);
      pushModelToRustAsync(newTables, get().relationships, get().measures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
      return true;
    },

    refreshTable: async (tableId) => {
      const target = get().tables.find(t => t.id === tableId || t.name === tableId);
      if (!target) {
        return { ok: false, message: 'Tabela não encontrada no modelo.' };
      }

      set({ isRefreshingData: true });
      const nowTime = new Date().toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' });
      try {
        const sc = target.sourceConfig;
        // Tabela com consulta do Power Query: as etapas editadas mandam. O
        // interpretador M antigo e o recarregamento do .pbip as descartariam.
        if (hasEtlQuery(get().appliedSteps[target.id])) {
          await get().refreshEtlTable(target.id);
          await withTimeout(pushModelToRustAsync(get().tables, get().relationships, get().measures), 'Sincronizar modelo');
          set(s => ({
            dataVersion: s.dataVersion + 1,
            refreshStatus: { type: 'success', message: `Tabela "${target.name}" atualizada pela consulta do Power Query.`, timestamp: nowTime },
          }));
          return { ok: true, message: `Tabela "${target.name}" atualizada.` };
        }
        if (sc?.type === 'db' && sc.db && isTauriRuntime()) {
          const dbConfig = sc.db;
          const payload = await withTimeout(dbRefreshImport(
            dbConfig.config as any,
            dbConfig.credentialKey,
            dbConfig.source,
            target.name
          ), 'Banco de dados');
          const cols = nativePayloadToColumns(payload, target.id);
          get().replaceTableData(target.id, cols, payload.rows, {
            ...sc,
            lastRefreshedAt: new Date().toISOString(),
          }, {
            duckdbTable: payload.tableName,
            rowsTruncated: payload.truncated,
            totalRows: payload.totalRows,
          });
          await withTimeout(pushModelToRustAsync(get().tables, get().relationships, get().measures), 'Sincronizar modelo');
          set(s => ({
            dataVersion: s.dataVersion + 1,
            refreshStatus: {
              type: 'success',
              message: `Tabela "${target.name}" atualizada via banco de dados (${payload.totalRows.toLocaleString('pt-BR')} linhas no modelo).`,
              timestamp: nowTime,
            },
          }));
          return { ok: true, message: `Tabela "${target.name}" atualizada.` };
        }

        if (sc?.type === 'rest' && sc.url) {
          const resp = await fetch(sc.url);
          if (!resp.ok) throw new Error(`HTTP ${resp.status} ao consultar ${sc.url}`);
          const json = await resp.json();
          const rows: Record<string, any>[] = Array.isArray(json)
            ? json
            : Array.isArray((json as any)?.data)
            ? (json as any).data
            : (Object.values(json || {}).find(v => Array.isArray(v)) as Record<string, any>[]) || [];
          const cols = inferColumnsFromJsonRows(rows, target.id);
          get().replaceTableData(target.id, cols.length > 0 ? cols : target.columns, rows, {
            ...sc,
            lastRefreshedAt: new Date().toISOString(),
          });
          await withTimeout(pushModelToRustAsync(get().tables, get().relationships, get().measures), 'Sincronizar modelo');
          set(s => ({
            dataVersion: s.dataVersion + 1,
            refreshStatus: {
              type: 'success',
              message: `Tabela "${target.name}" atualizada via REST (${rows.length.toLocaleString('pt-BR')} linhas).`,
              timestamp: nowTime,
            },
          }));
          return { ok: true, message: `Tabela "${target.name}" atualizada.` };
        }

        if (isTauriRuntime() && sc?.filePath && sc.type !== 'sample') {
          const payload = await withTimeout(refreshTableNative({
            tableName: target.name,
            sourceType: sc.type,
            filePath: sc.filePath,
            sheetName: sc.sheetName,
            mScript: sc.mScript,
            pbipRoot: sc.pbipRoot || get().sourcePbipPath || undefined,
          }), 'Arquivo fonte');
          const cols = nativePayloadToColumns(payload, target.id);
          get().replaceTableData(target.id, cols, payload.rows, {
            ...sc,
            filePath: payload.sourcePath || sc.filePath,
            sheetName: payload.sheetName || sc.sheetName,
            lastRefreshedAt: new Date().toISOString(),
          }, {
            duckdbTable: payload.tableName,
            rowsTruncated: payload.truncated,
            totalRows: payload.totalRows,
          });
          await withTimeout(pushModelToRustAsync(get().tables, get().relationships, get().measures), 'Sincronizar modelo');
          set(s => ({
            dataVersion: s.dataVersion + 1,
            refreshStatus: {
              type: 'success',
              message: `Tabela "${target.name}" atualizada (${payload.totalRows.toLocaleString('pt-BR')} linhas no modelo).`,
              timestamp: nowTime,
            },
          }));
          return { ok: true, message: `Tabela "${target.name}" atualizada.` };
        }

        if (isTauriRuntime() && get().sourcePbipPath) {
          const pbipRes = await withTimeout(refreshPbipTables(get().sourcePbipPath!), 'Projeto .pbip');
          const matched = pbipRes.loadedTables.find(
            lt =>
              lt.tableName.toLowerCase() === target.name.toLowerCase() ||
              lt.tableName.toLowerCase() === target.originalName.toLowerCase()
          );
          if (matched) {
            const cols = nativePayloadToColumns(matched, target.id);
            get().replaceTableData(target.id, cols, matched.rows, {
              type: matched.sourcePath?.toLowerCase().endsWith('.csv') ? 'csv' : 'xlsx',
              filePath: matched.sourcePath || sc?.filePath,
              sheetName: matched.sheetName || sc?.sheetName,
              mScript: sc?.mScript,
              pbipRoot: sc?.pbipRoot || get().sourcePbipPath || undefined,
            }, {
              duckdbTable: matched.tableName,
              rowsTruncated: matched.truncated,
              totalRows: matched.totalRows,
            });
            await withTimeout(pushModelToRustAsync(get().tables, get().relationships, get().measures), 'Sincronizar modelo');
            set(s => ({
              dataVersion: s.dataVersion + 1,
              refreshStatus: {
                type: 'success',
                message: `Tabela "${target.name}" atualizada do projeto .pbip (${matched.totalRows.toLocaleString('pt-BR')} linhas no modelo).`,
                timestamp: nowTime,
              },
            }));
            return { ok: true, message: `Tabela "${target.name}" atualizada.` };
          }
        }

        const warnMsg = `Tabela "${target.name}" não possui arquivo/URL vinculado. Use "Substituir dados" no painel Campos para vincular.`;
        set({
          refreshStatus: {
            type: 'warning',
            message: warnMsg,
            timestamp: nowTime,
          },
        });
        return { ok: false, message: warnMsg };
      } catch (err: any) {
        const msg = err?.message || String(err);
        set({
          refreshStatus: {
            type: 'error',
            message: `Erro ao atualizar "${target.name}": ${msg}`,
            timestamp: nowTime,
          },
        });
        return { ok: false, message: msg };
      } finally {
        set({ isRefreshingData: false });
      }
    },

    refreshAllData: async () => {
      set({ isRefreshingData: true });
      const nowTime = new Date().toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' });
      const refreshedTables: string[] = [];
      const warnings: string[] = [];

      try {
        const currentTables = get().tables;
        const pbipPath = get().sourcePbipPath;

        // 1. Tabelas com consulta do Power Query: fonte bruta + etapas (ordem do
        //    modelo; consultas que mesclam/acrescentam leem as já aplicadas).
        const etlTableIds = new Set<string>();
        for (const tbl of currentTables) {
          if (!hasEtlQuery(get().appliedSteps[tbl.id])) continue;
          etlTableIds.add(tbl.id);
          try {
            await get().refreshEtlTable(tbl.id);
            refreshedTables.push(tbl.name);
          } catch (err: any) {
            warnings.push(`${tbl.name}: ${err?.message || String(err)}`);
          }
        }

        for (const tbl of currentTables) {
          const sc = tbl.sourceConfig;
          if (!sc || etlTableIds.has(tbl.id)) continue;

          if (sc.type === 'db' && sc.db && isTauriRuntime()) {
            try {
              const dbConfig = sc.db;
              const payload = await withTimeout(dbRefreshImport(
                dbConfig.config as any,
                dbConfig.credentialKey,
                dbConfig.source,
                tbl.name
              ), 'Banco de dados');
              const cols = nativePayloadToColumns(payload, tbl.id);
              get().replaceTableData(tbl.id, cols, payload.rows, {
                ...sc,
                lastRefreshedAt: new Date().toISOString(),
              });
              refreshedTables.push(tbl.name);
            } catch (err: any) {
              warnings.push(`${tbl.name}: ${err?.message || String(err)}`);
            }
          } else if (sc.type === 'rest' && sc.url) {
            try {
              const resp = await fetch(sc.url);
              if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
              const json = await resp.json();
              const rows: Record<string, any>[] = Array.isArray(json)
                ? json
                : Array.isArray((json as any)?.data)
                ? (json as any).data
                : (Object.values(json || {}).find(v => Array.isArray(v)) as Record<string, any>[]) || [];
              const cols = inferColumnsFromJsonRows(rows, tbl.id);
              get().replaceTableData(tbl.id, cols.length > 0 ? cols : tbl.columns, rows, sc);
              refreshedTables.push(tbl.name);
            } catch (err: any) {
              warnings.push(`${tbl.name}: ${err?.message || String(err)}`);
            }
          } else if (isTauriRuntime() && sc.filePath && sc.type !== 'sample') {
            try {
              const payload = await withTimeout(refreshTableNative({
                tableName: tbl.name,
                sourceType: sc.type,
                filePath: sc.filePath,
                sheetName: sc.sheetName,
                mScript: sc.mScript,
                pbipRoot: sc.pbipRoot || pbipPath || undefined,
              }), 'Arquivo fonte');
              const cols = nativePayloadToColumns(payload, tbl.id);
              get().replaceTableData(tbl.id, cols, payload.rows, {
                ...sc,
                filePath: payload.sourcePath || sc.filePath,
                sheetName: payload.sheetName || sc.sheetName,
              });
              refreshedTables.push(tbl.name);
            } catch (err: any) {
              warnings.push(`${tbl.name}: ${err?.message || String(err)}`);
            }
          }
        }

        // Refresh remaining PBIP tables if project came from a .pbip file
        if (isTauriRuntime() && pbipPath) {
          try {
            const pbipRes = await withTimeout(refreshPbipTables(pbipPath), 'Projeto .pbip');
            for (const warn of pbipRes.warnings) {
              warnings.push(warn);
            }
            for (const lt of pbipRes.loadedTables) {
              const match = get().tables.find(
                t =>
                  !refreshedTables.includes(t.name) &&
                  (t.name.toLowerCase() === lt.tableName.toLowerCase() ||
                    t.originalName.toLowerCase() === lt.tableName.toLowerCase())
              );
              if (match) {
                const cols = nativePayloadToColumns(lt, match.id);
                const lowerPath = (lt.sourcePath || '').toLowerCase();
                const detectedType = lowerPath.endsWith('.csv')
                  ? 'csv'
                  : lowerPath.endsWith('.parquet')
                  ? 'parquet'
                  : lowerPath.endsWith('.json')
                  ? 'json'
                  : lt.sourcePath
                  ? 'xlsx'
                  : 'pbip';
                get().replaceTableData(match.id, cols, lt.rows, {
                  type: detectedType,
                  filePath: lt.sourcePath || match.sourceConfig?.filePath,
                  sheetName: lt.sheetName || match.sourceConfig?.sheetName,
                  mScript: match.sourceConfig?.mScript,
                  pbipRoot: pbipPath,
                });
                refreshedTables.push(match.name);
              }
            }
          } catch (err: any) {
            warnings.push(`.pbip: ${err?.message || String(err)}`);
          }
        }

        // Always re-sync model schema & DuckDB and bump dataVersion so visuals recalculate cleanly
        const latestTables = get().tables;
        const latestRels = get().relationships;
        const latestMeasures = get().measures;
        globalDataEngine.setSchema(latestTables, latestRels, latestMeasures);
        latestTables.forEach(t => globalDuckDb.registerTable(t));
        await withTimeout(pushModelToRustAsync(latestTables, latestRels, latestMeasures), 'Sincronizar modelo');
        set(s => ({ dataVersion: s.dataVersion + 1 }));

        if (refreshedTables.length > 0 && warnings.length === 0) {
          set({
            refreshStatus: {
              type: 'success',
              message: `${refreshedTables.length} tabela(s) atualizada(s): ${refreshedTables.join(', ')}.`,
              timestamp: nowTime,
            },
          });
        } else if (refreshedTables.length > 0 && warnings.length > 0) {
          set({
            refreshStatus: {
              type: 'warning',
              message: `${refreshedTables.length} atualizada(s), com ${warnings.length} aviso(s): ${warnings[0]}`,
              timestamp: nowTime,
            },
          });
        } else if (warnings.length > 0) {
          set({
            refreshStatus: {
              type: 'warning',
              message: `Nenhuma tabela externa atualizada: ${warnings[0]}`,
              timestamp: nowTime,
            },
          });
        } else {
          set({
            refreshStatus: {
              type: 'success',
              message: 'Modelo recalculado. Para reimportar arquivos, vincule a fonte ou substitua a tabela no painel Campos.',
              timestamp: nowTime,
            },
          });
        }

        return { refreshedTables, warnings };
      } catch (err: any) {
        const msg = err?.message || String(err);
        set({
          refreshStatus: {
            type: 'error',
            message: `Erro ao atualizar dados: ${msg}`,
            timestamp: nowTime,
          },
        });
        return { refreshedTables, warnings, error: msg };
      } finally {
        set({ isRefreshingData: false });
      }
    },

    duplicateTable: (id) => {
      const orig = get().tables.find(t => t.id === id || t.name === id);
      if (!orig) return undefined;
      const baseName = orig.name;
      const newName = `${baseName}_Copia`;
      const newId = `${orig.id}_copia_${Math.random().toString(36).substring(2, 6)}`;
      const duplicated: Table = {
        id: newId,
        name: newName,
        originalName: orig.originalName || orig.name,
        columns: orig.columns.map(c => ({
          ...c,
          id: 'c_' + Math.random().toString(36).substring(2, 6),
        })),
        rows: orig.rows.map(r => ({ ...r })),
      };
      get().addTable(duplicated);
      const existingQuery = get().appliedSteps[orig.id];
      if (existingQuery) {
        set({
          appliedSteps: {
            ...get().appliedSteps,
            [newId]: { ...existingQuery, name: newName, steps: existingQuery.steps.map(s => ({ ...s })) },
          },
        });
      }
      return duplicated;
    },

    addRelationship: (rel) => {
      const newRels = [...get().relationships, rel];
      set(s => ({ relationships: newRels, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(get().tables, newRels, get().measures);
      pushModelToRustAsync(get().tables, newRels, get().measures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    deleteRelationship: (id) => {
      const newRels = get().relationships.filter(r => r.id !== id);
      set(s => ({ relationships: newRels, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(get().tables, newRels, get().measures);
      pushModelToRustAsync(get().tables, newRels, get().measures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    addMeasure: (measure) => {
      const newMeasures = [...get().measures, measure];
      set(s => ({ measures: newMeasures, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(get().tables, get().relationships, newMeasures);
      pushModelToRustAsync(get().tables, get().relationships, newMeasures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    updateMeasure: (id, partial) => {
      const target = get().measures.find(m => m.id === id);
      if (target && partial.name && partial.name.trim() !== target.name.trim()) {
        const oldName = target.name.trim();
        const newName = partial.name.trim();
        const allVisuals = get().pages.flatMap(p => p.visuals);
        const { updatedMeasures, updatedVisuals } = DaxEngine.renameMeasure(oldName, newName, get().measures, allVisuals);
        const finalMeasures = updatedMeasures.map(m => (m.id === id ? { ...m, ...partial, name: newName } : m));

        const updatedPages = get().pages.map(p => ({
          ...p,
          visuals: p.visuals.map(v => updatedVisuals.find(uv => uv.id === v.id) || v),
        }));

        set(s => ({ measures: finalMeasures, pages: updatedPages, dataVersion: s.dataVersion + 1 }));
        globalDataEngine.setSchema(get().tables, get().relationships, finalMeasures);
        pushModelToRustAsync(get().tables, get().relationships, finalMeasures, () =>
          set(s => ({ dataVersion: s.dataVersion + 1 }))
        );
        return;
      }

      const newMeasures = get().measures.map(m => (m.id === id ? { ...m, ...partial } : m));
      set(s => ({ measures: newMeasures, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(get().tables, get().relationships, newMeasures);
      pushModelToRustAsync(get().tables, get().relationships, newMeasures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    deleteMeasure: (id) => {
      const newMeasures = get().measures.filter(m => m.id !== id);
      set(s => ({ measures: newMeasures, dataVersion: s.dataVersion + 1 }));
      globalDataEngine.setSchema(get().tables, get().relationships, newMeasures);
      pushModelToRustAsync(get().tables, get().relationships, newMeasures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    editingMeasureId: null,
    setEditingMeasureId: (id) => set({ editingMeasureId: id }),
    openCustomMeasureModal: (measureId) =>
      set({
        customMeasureModalOpen: true,
        editingMeasureId: measureId || null,
      }),

    // Pages & Visuals
    pages: [
      {
        id: defaultPageId,
        name: 'Visão Geral Executiva',
        order: 1,
        visuals: initialVisuals,
      },
      {
        id: 'page_dre',
        name: 'DRE & Finanças',
        order: 2,
        visuals: [
          {
            id: 'vis_dre_waterfall',
            type: 'waterfall',
            title: 'Demonstrativo do Resultado do Exercício (Cascata)',
            x: 20,
            y: 20,
            w: 720,
            h: 360,
            dataConfig: {
              tableId: 'f_DRE',
              xAxisField: 'ContaDRE',
              yAxisFields: ['ValorRealizado'],
              aggregation: 'SUM',
            },
            formatConfig: {
              showTitle: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
          {
            id: 'vis_dre_cards',
            type: 'card',
            title: 'Receita Bruta Total',
            x: 760,
            y: 20,
            w: 380,
            h: 160,
            dataConfig: {
              tableId: 'f_DRE',
              yAxisFields: ['ValorRealizado'],
              aggregation: 'SUM',
            },
            formatConfig: {
              showTitle: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
          {
            id: 'vis_dre_table',
            type: 'table',
            title: 'Detalhamento Contas DRE: Realizado vs Orçado',
            x: 20,
            y: 400,
            w: 1120,
            h: 320,
            dataConfig: {
              tableId: 'f_DRE',
              limit: 50,
            },
            formatConfig: {
              showTitle: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
        ],
      },
      {
        id: 'page_logistica',
        name: 'Logística & Frotas',
        order: 3,
        visuals: [
          {
            id: 'vis_log_rotas',
            type: 'bar',
            title: 'Receita de Frete por Rota',
            x: 20,
            y: 20,
            w: 560,
            h: 320,
            dataConfig: {
              tableId: 'f_Logistica',
              xAxisField: 'Rota',
              yAxisFields: ['ReceitaFrete'],
              aggregation: 'SUM',
            },
            formatConfig: {
              showTitle: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
          {
            id: 'vis_log_custo_km',
            type: 'scatter',
            title: 'Km Rodado vs Custo Total Viagem',
            x: 600,
            y: 20,
            w: 540,
            h: 320,
            dataConfig: {
              tableId: 'f_Logistica',
              xAxisField: 'KmRodado',
              yAxisFields: ['CustoTotalViagem'],
              aggregation: 'SUM',
            },
            formatConfig: {
              showTitle: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
        ],
      },
      {
        id: 'page_detalhes',
        name: 'Detalhamento de Entidade',
        order: 4,
        drillthroughConfig: {
          isDrillthroughPage: true,
          targetFields: ['Categoria', 'Regiao', 'Vendedor', 'Canal'],
        },
        visuals: [
          {
            id: 'vis_det_kpi_fat',
            type: 'card',
            title: 'Faturamento do Item / Entidade',
            x: 20,
            y: 20,
            w: 360,
            h: 120,
            dataConfig: {
              tableId: 'f_Vendas',
              yAxisFields: ['ValorTotal'],
              aggregation: 'SUM',
            },
            formatConfig: {
              showTitle: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
          {
            id: 'vis_det_kpi_lucro',
            type: 'card',
            title: 'Margem de Lucro Filtrada',
            x: 400,
            y: 20,
            w: 360,
            h: 120,
            dataConfig: {
              tableId: 'f_Vendas',
              yAxisFields: ['MargemLucro'],
              aggregation: 'SUM',
            },
            formatConfig: {
              showTitle: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
          {
            id: 'vis_det_kpi_qtd',
            type: 'card',
            title: 'Quantidade Total Vendida',
            x: 780,
            y: 20,
            w: 360,
            h: 120,
            dataConfig: {
              tableId: 'f_Vendas',
              yAxisFields: ['Quantidade'],
              aggregation: 'SUM',
            },
            formatConfig: {
              showTitle: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
          {
            id: 'vis_det_col_mes',
            type: 'column',
            title: 'Evolução Mensal do Item Selecionado',
            x: 20,
            y: 160,
            w: 680,
            h: 300,
            dataConfig: {
              tableId: 'f_Vendas',
              xAxisField: 'Mes',
              yAxisFields: ['ValorTotal'],
              aggregation: 'SUM',
            },
            formatConfig: {
              showTitle: true,
              showDataLabels: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
          {
            id: 'vis_det_donut_canal',
            type: 'donut',
            title: 'Mix por Canal de Venda',
            x: 720,
            y: 160,
            w: 420,
            h: 300,
            dataConfig: {
              tableId: 'f_Vendas',
              xAxisField: 'Canal',
              yAxisFields: ['ValorTotal'],
              aggregation: 'SUM',
            },
            formatConfig: {
              showTitle: true,
              showLegend: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
          {
            id: 'vis_det_table_vendas',
            type: 'table',
            title: 'Todas as Transações e Detalhes da Entidade Filtrada',
            x: 20,
            y: 480,
            w: 1120,
            h: 280,
            dataConfig: {
              tableId: 'f_Vendas',
              limit: 50,
            },
            formatConfig: {
              showTitle: true,
              backgroundColor: '#ffffff',
              borderRadius: 8,
            },
          },
        ],
      },
    ],
    activePageId: defaultPageId,
    selectedVisualId: null,
    selectedVisualIds: [],
    canvasZoom: 100,
    isPresentationMode: false,
    layoutMode: 'desktop',
    historyPast: [],
    historyFuture: [],
    canUndo: false,
    canRedo: false,

    setActivePageId: (id) => set({ activePageId: id, selectedVisualId: null, selectedVisualIds: [] }),

    addPage: (name) => {
      const newPageId = 'page_' + Math.random().toString(36).substr(2, 6);
      const newPage: Page = {
        id: newPageId,
        name: name || `Página ${get().pages.length + 1}`,
        order: get().pages.length + 1,
        visuals: [],
      };
      pushPagesHistory([...get().pages, newPage], {
        activePageId: newPageId,
        selectedVisualId: null,
        selectedVisualIds: [],
      });
    },

    renamePage: (id, newName) => {
      pushPagesHistory(get().pages.map(p => (p.id === id ? { ...p, name: newName } : p)));
    },

    duplicatePage: (id) => {
      const pageToDup = get().pages.find(p => p.id === id);
      if (!pageToDup) return;
      const newId = 'page_' + Math.random().toString(36).substr(2, 6);
      const duplicated: Page = {
        ...pageToDup,
        id: newId,
        name: `${pageToDup.name} (Cópia)`,
        order: get().pages.length + 1,
        visuals: pageToDup.visuals.map(v => ({
          ...v,
          id: 'vis_' + Math.random().toString(36).substr(2, 6),
        })),
      };
      pushPagesHistory([...get().pages, duplicated], {
        activePageId: newId,
        selectedVisualId: null,
        selectedVisualIds: [],
      });
    },

    deletePage: (id) => {
      if (get().pages.length <= 1) return;
      const remaining = get().pages.filter(p => p.id !== id);
      pushPagesHistory(remaining, {
        activePageId: remaining[0].id,
        selectedVisualId: null,
        selectedVisualIds: [],
      });
    },

    setPageCanvasSize: (pageId, canvasSize, applyToAllPages = false) => {
      const sanitizedWidth = Math.max(200, Math.min(4000, Math.round(canvasSize.width)));
      const sanitizedHeight = Math.max(150, Math.min(4000, Math.round(canvasSize.height)));
      const safeConfig: CanvasSizeConfig = {
        ...canvasSize,
        width: sanitizedWidth,
        height: sanitizedHeight,
      };

      pushPagesHistory(
        get().pages.map(p => {
          if (applyToAllPages || p.id === pageId) {
            return { ...p, canvasSize: safeConfig };
          }
          return p;
        })
      );
    },

    setSelectedVisualId: (id) => set({ selectedVisualId: id, selectedVisualIds: id ? [id] : [] }),
    setSelectedVisualIds: (ids) => {
      const unique = Array.from(new Set(ids));
      set({
        selectedVisualIds: unique,
        selectedVisualId: unique[unique.length - 1] || null,
      });
    },
    toggleVisualSelection: (id) => {
      const curr = get().selectedVisualIds;
      const exists = curr.includes(id);
      const next = exists ? curr.filter(x => x !== id) : [...curr, id];
      set({
        selectedVisualIds: next,
        selectedVisualId: next[next.length - 1] || null,
      });
    },
    setCanvasZoom: (zoom) => set({ canvasZoom: Math.max(30, Math.min(200, zoom)) }),
    setIsPresentationMode: (val) => set({ isPresentationMode: val }),
    setLayoutMode: (mode) => set({ layoutMode: mode }),

    addVisual: (type, tableId) => {
      const activePage = get().pages.find(p => p.id === get().activePageId);
      if (!activePage) return;

      const targetTable = tableId || get().tables[0]?.id || 'f_Vendas';
      const tbl = get().tables.find(t => t.id === targetTable);
      const numCol = tbl?.columns.find(c => c.dataType === 'number' || c.dataType === 'currency')?.name || 'ValorTotal';
      const catCol = tbl?.columns.find(c => c.dataType === 'text')?.name || 'Categoria';

      const newVisual: Visual = {
        id: 'vis_' + Math.random().toString(36).substr(2, 6),
        type,
        title: `Novo Visual (${type})`,
        x: 40 + (activePage.visuals.length % 3) * 40,
        y: 40 + (activePage.visuals.length % 3) * 40,
        w: type === 'card' || type === 'kpi' ? 260 : 480,
        h: type === 'card' || type === 'kpi' ? 120 : 280,
        dataConfig: {
          tableId: targetTable,
          xAxisField: catCol,
          yAxisFields: [numCol],
          aggregation: 'SUM',
        },
        formatConfig: {
          showTitle: true,
          backgroundColor: '#ffffff',
          borderColor: '#e2e8f0',
          borderRadius: 8,
          showDataLabels: true,
        },
      };

      pushPagesHistory(
        get().pages.map(p =>
          p.id === get().activePageId ? { ...p, visuals: [...p.visuals, newVisual] } : p
        ),
        {
          selectedVisualId: newVisual.id,
          selectedVisualIds: [newVisual.id],
        }
      );
    },

    updateVisual: (id, partial) => {
      pushPagesHistory(
        get().pages.map(p => ({
          ...p,
          visuals: p.visuals.map(v => (v.id === id ? { ...v, ...partial } : v)),
        }))
      );
    },

    deleteVisual: (id) => {
      const nextSelected = get().selectedVisualIds.filter(x => x !== id);
      pushPagesHistory(
        get().pages.map(p => ({
          ...p,
          visuals: p.visuals.filter(v => v.id !== id),
        })),
        {
          selectedVisualId: nextSelected[nextSelected.length - 1] || null,
          selectedVisualIds: nextSelected,
        }
      );
    },

    moveVisualsDelta: (ids, dx, dy) => {
      if (!ids.length || (dx === 0 && dy === 0)) return;
      const activePageId = get().activePageId;
      const activePage = get().pages.find(p => p.id === activePageId);
      if (!activePage) return;
      const { width: canvasWidth, height: canvasHeight } = getEffectiveCanvasSize(activePage);
      const idSet = new Set(ids);
      let changed = false;

      const nextPages = get().pages.map(p => {
        if (p.id !== activePageId) return p;
        return {
          ...p,
          visuals: p.visuals.map(v => {
            if (!idSet.has(v.id)) return v;
            const nx = Math.max(0, Math.min(Math.max(0, canvasWidth - v.w), Math.round(v.x + dx)));
            const ny = Math.max(0, Math.min(Math.max(0, canvasHeight - v.h), Math.round(v.y + dy)));
            if (nx !== v.x || ny !== v.y) changed = true;
            return { ...v, x: nx, y: ny };
          }),
        };
      });

      if (changed) {
        pushPagesHistory(nextPages);
      }
    },

    deleteSelectedVisuals: () => {
      const ids = get().selectedVisualIds;
      if (!ids.length) return;
      const activePageId = get().activePageId;
      const idSet = new Set(ids);
      pushPagesHistory(
        get().pages.map(p =>
          p.id === activePageId ? { ...p, visuals: p.visuals.filter(v => !idSet.has(v.id)) } : p
        ),
        {
          selectedVisualId: null,
          selectedVisualIds: [],
        }
      );
    },

    alignSelectedVisuals: (mode) => {
      const ids = get().selectedVisualIds;
      if (ids.length < 2) return;
      const activePageId = get().activePageId;
      const activePage = get().pages.find(p => p.id === activePageId);
      if (!activePage) return;
      const idSet = new Set(ids);
      const selected = activePage.visuals.filter(v => idSet.has(v.id));
      if (selected.length < 2) return;

      const minX = Math.min(...selected.map(v => v.x));
      const maxR = Math.max(...selected.map(v => v.x + v.w));
      const midCenterX = (minX + maxR) / 2;
      const minY = Math.min(...selected.map(v => v.y));
      const maxB = Math.max(...selected.map(v => v.y + v.h));
      const midCenterY = (minY + maxB) / 2;

      const nextPos = new Map<string, { x: number; y: number }>();

      if (mode === 'distribute-h' && selected.length >= 3) {
        const sorted = [...selected].sort((a, b) => a.x - b.x);
        const totalWidth = sorted.reduce((acc, v) => acc + v.w, 0);
        const span = sorted[sorted.length - 1].x + sorted[sorted.length - 1].w - sorted[0].x;
        const gap = (span - totalWidth) / (sorted.length - 1);
        let cursor = sorted[0].x;
        for (const v of sorted) {
          nextPos.set(v.id, { x: Math.round(cursor), y: v.y });
          cursor += v.w + gap;
        }
      } else if (mode === 'distribute-v' && selected.length >= 3) {
        const sorted = [...selected].sort((a, b) => a.y - b.y);
        const totalHeight = sorted.reduce((acc, v) => acc + v.h, 0);
        const span = sorted[sorted.length - 1].y + sorted[sorted.length - 1].h - sorted[0].y;
        const gap = (span - totalHeight) / (sorted.length - 1);
        let cursor = sorted[0].y;
        for (const v of sorted) {
          nextPos.set(v.id, { x: v.x, y: Math.round(cursor) });
          cursor += v.h + gap;
        }
      } else {
        for (const v of selected) {
          let x = v.x;
          let y = v.y;
          if (mode === 'left') x = minX;
          else if (mode === 'center') x = Math.round(midCenterX - v.w / 2);
          else if (mode === 'right') x = maxR - v.w;
          else if (mode === 'top') y = minY;
          else if (mode === 'middle') y = Math.round(midCenterY - v.h / 2);
          else if (mode === 'bottom') y = maxB - v.h;
          nextPos.set(v.id, { x: Math.max(0, x), y: Math.max(0, y) });
        }
      }

      const nextPages = get().pages.map(p =>
        p.id === activePageId
          ? {
              ...p,
              visuals: p.visuals.map(v => {
                const pos = nextPos.get(v.id);
                return pos ? { ...v, x: pos.x, y: pos.y } : v;
              }),
            }
          : p
      );
      pushPagesHistory(nextPages);
    },

    undo: () => {
      const { historyPast, historyFuture, pages, activePageId } = get();
      if (historyPast.length === 0) return;
      const previous = clonePagesSnapshot(historyPast[historyPast.length - 1]);
      const nextPast = historyPast.slice(0, -1);
      const nextFuture = [clonePagesSnapshot(pages), ...historyFuture].slice(0, MAX_CANVAS_HISTORY);
      const nextActivePage = previous.find(p => p.id === activePageId) || previous[0];
      set({
        pages: previous,
        activePageId: nextActivePage ? nextActivePage.id : activePageId,
        selectedVisualId: null,
        selectedVisualIds: [],
        historyPast: nextPast,
        historyFuture: nextFuture,
        canUndo: nextPast.length > 0,
        canRedo: nextFuture.length > 0,
      });
    },

    redo: () => {
      const { historyPast, historyFuture, pages, activePageId } = get();
      if (historyFuture.length === 0) return;
      const next = clonePagesSnapshot(historyFuture[0]);
      const nextFuture = historyFuture.slice(1);
      const nextPast = [...historyPast, clonePagesSnapshot(pages)].slice(-MAX_CANVAS_HISTORY);
      const nextActivePage = next.find(p => p.id === activePageId) || next[0];
      set({
        pages: next,
        activePageId: nextActivePage ? nextActivePage.id : activePageId,
        selectedVisualId: null,
        selectedVisualIds: [],
        historyPast: nextPast,
        historyFuture: nextFuture,
        canUndo: nextPast.length > 0,
        canRedo: nextFuture.length > 0,
      });
    },

    // Filtering & Cross-filtering
    filters: [],

    addFilter: (filter) => {
      set({ filters: [...get().filters.filter(f => f.id !== filter.id), filter] });
    },

    removeFilter: (id) => {
      set({ filters: get().filters.filter(f => f.id !== id) });
    },

    clearFilters: () => {
      set({ filters: [] });
    },

    setCrossFilter: (tableId, column, value) => {
      let resolvedTableId = tableId;
      let cleanColumn = column;

      // Handle table prefix if present, e.g. "d_Clientes.Segmento"
      if (column.includes('.')) {
        const [prefixT, col] = column.split('.', 2);
        const matchT = get().tables.find(
          t => t.id === prefixT || t.name === prefixT || t.id.toLowerCase() === prefixT.toLowerCase() || t.name.toLowerCase() === prefixT.toLowerCase()
        );
        if (matchT) {
          resolvedTableId = matchT.id;
          cleanColumn = col;
        }
      } else {
        const targetTable = get().tables.find(t => t.id === tableId || t.name === tableId);
        if (targetTable && !targetTable.columns.some(c => c.name.toLowerCase() === column.toLowerCase())) {
          for (const t of get().tables) {
            if (t.columns.some(c => c.name.toLowerCase() === column.toLowerCase())) {
              resolvedTableId = t.id;
              cleanColumn = t.columns.find(c => c.name.toLowerCase() === column.toLowerCase())!.name;
              break;
            }
          }
        }
      }

      const targetTable = get().tables.find(t => t.id === resolvedTableId || t.name === resolvedTableId);
      const matchedTableId = targetTable ? targetTable.id : resolvedTableId;
      const matchedTableName = targetTable ? targetTable.name : resolvedTableId;

      const existing = get().filters.find(
        f => (f.tableId === matchedTableId || f.tableId === matchedTableName) &&
             f.column === cleanColumn &&
             f.level === 'visual'
      );

      // If value is empty, null, undefined or 'TODOS', or if clicked again on same value -> toggle off
      if (!value || value === 'TODOS' || (existing && existing.value === value)) {
        if (existing) {
          set({ filters: get().filters.filter(f => f.id !== existing.id) });
        }
      } else {
        const newFilter: Filter = {
          id: 'cross_' + matchedTableId + '_' + cleanColumn,
          level: 'visual',
          tableId: matchedTableId,
          column: cleanColumn,
          operator: 'equals',
          value,
        };
        set({
          filters: [
            ...get().filters.filter(
              f => f.id !== newFilter.id &&
                   !( (f.tableId === matchedTableId || f.tableId === matchedTableName) && f.column === cleanColumn && f.level === 'visual' )
            ),
            newFilter,
          ],
        });
      }
    },

    setSlicerSelection: (visualId, selection) => {
      const state = get();
      let triggerVisual: Visual | null = null;
      for (const p of state.pages) {
        const v = p.visuals.find(x => x.id === visualId);
        if (v) {
          triggerVisual = v;
          break;
        }
      }
      if (!triggerVisual) return;

      const targets = [{ visual: triggerVisual, selectionOverride: selection }];
      const syncGroup = triggerVisual.dataConfig.slicerConfig?.syncGroup;
      const syncPages = triggerVisual.dataConfig.slicerConfig?.syncPages;
      if (syncGroup) {
        for (const sib of findSyncSiblings(state.pages, syncGroup, triggerVisual.id)) {
          if (pageInSyncScope(syncPages, sib.pageId)) {
            targets.push({ visual: sib.visual, selectionOverride: selection });
          }
        }
      }

      const updatedById = new Map<string, Visual>();
      for (const { visual, selectionOverride } of targets) {
        const nextSlicerConfig = { ...visual.dataConfig.slicerConfig, ...selectionOverride } as NonNullable<
          VisualDataConfig['slicerConfig']
        >;
        updatedById.set(visual.id, {
          ...visual,
          dataConfig: { ...visual.dataConfig, slicerConfig: nextSlicerConfig },
        });
      }

      const nextPages = state.pages.map(p => ({
        ...p,
        visuals: p.visuals.map(v => updatedById.get(v.id) || v),
      }));

      let nextFilters = state.filters;
      for (const visual of updatedById.values()) {
        const fId = slicerFilterId(visual.id);
        const { tableId, column } = resolveSlicerField(visual, state.tables);
        const resolved = buildSlicerFilterValue(visual.dataConfig.slicerConfig!);
        nextFilters = nextFilters.filter(f => f.id !== fId);
        if (resolved) {
          nextFilters = [
            ...nextFilters,
            { id: fId, level: 'visual', visualId: visual.id, tableId, column, operator: resolved.operator, value: resolved.value },
          ];
        }
      }

      pushPagesHistory(nextPages, { filters: nextFilters });
    },

    recalculateRelativeDateFilters: () => {
      const state = get();
      const today = new Date();
      let nextFilters = state.filters;
      for (const page of state.pages) {
        for (const visual of page.visuals) {
          const slicerConfig = visual.dataConfig.slicerConfig;
          if (!slicerConfig?.relative) continue;
          const fId = slicerFilterId(visual.id);
          const resolved = buildSlicerFilterValue(slicerConfig, today);
          nextFilters = nextFilters.filter(f => f.id !== fId);
          if (resolved) {
            const { tableId, column } = resolveSlicerField(visual, state.tables);
            nextFilters = [
              ...nextFilters,
              { id: fId, level: 'visual', visualId: visual.id, tableId, column, operator: resolved.operator, value: resolved.value },
            ];
          }
        }
      }
      if (nextFilters !== state.filters) set({ filters: nextFilters });
    },

    // Drill-through State & Actions
    activeDrillthrough: null,

    executeDrillthrough: (targetPageId, field, value, sourceVisualTitle) => {
      const state = get();
      const sourcePage = state.pages.find(p => p.id === state.activePageId) || state.pages[0];
      const sourceVisual = sourceVisualTitle || 'Visual';
      const filterId = `drill_filter_${Date.now()}`;

      // Identify table for this field (default to f_Vendas or first matching table)
      let tableId = 'f_Vendas';
      for (const t of state.tables) {
        if (t.columns.some(c => c.name.toLowerCase() === field.toLowerCase())) {
          tableId = t.id;
          break;
        }
      }

      const drillFilter: Filter = {
        id: filterId,
        level: 'page',
        tableId,
        column: field,
        operator: 'equals',
        value,
      };

      set({
        activePageId: targetPageId,
        selectedVisualId: null,
        filters: [...state.filters.filter(f => !f.id.startsWith('drill_filter_')), drillFilter],
        activeDrillthrough: {
          sourcePageId: sourcePage.id,
          sourcePageName: sourcePage.name,
          sourceVisualTitle: sourceVisual,
          field,
          value,
          filterId,
        },
      });
    },

    clearDrillthrough: (returnToSource = false) => {
      const state = get();
      const currentDrill = state.activeDrillthrough;
      const newFilters = currentDrill
        ? state.filters.filter(f => f.id !== currentDrill.filterId)
        : state.filters;

      const targetPageId = returnToSource && currentDrill ? currentDrill.sourcePageId : state.activePageId;

      set({
        activePageId: targetPageId,
        filters: newFilters,
        activeDrillthrough: null,
      });
    },

    // Bookmarks
    bookmarks: [
      {
        id: 'bm_padrao',
        name: 'Visão Geral Sem Filtros',
        activePageId: defaultPageId,
        filters: [],
      },
    ],

    addBookmark: (name) => {
      const newBm: Bookmark = {
        id: 'bm_' + Math.random().toString(36).substr(2, 6),
        name,
        activePageId: get().activePageId,
        filters: [...get().filters],
      };
      set({ bookmarks: [...get().bookmarks, newBm] });
    },

    applyBookmark: (id) => {
      const bm = get().bookmarks.find(b => b.id === id);
      if (bm) {
        set({
          activePageId: bm.activePageId,
          filters: [...bm.filters],
        });
      }
    },

    deleteBookmark: (id) => {
      set({ bookmarks: get().bookmarks.filter(b => b.id !== id) });
    },

    // Dashboard & Goals
    dashboardTiles: [
      {
        id: 'tile_1',
        visualId: 'vis_kpi_fat',
        pageId: defaultPageId,
        title: 'Faturamento Total da Empresa',
        x: 0,
        y: 0,
        w: 4,
        h: 2,
        alertThreshold: 500000,
        alertCondition: 'above',
      },
      {
        id: 'tile_2',
        visualId: 'vis_kpi_lucro',
        pageId: defaultPageId,
        title: 'Margem Bruta Geral',
        x: 4,
        y: 0,
        w: 4,
        h: 2,
      },
      {
        id: 'tile_3',
        visualId: 'vis_col_categoria',
        pageId: defaultPageId,
        title: 'Faturamento por Categoria',
        x: 0,
        y: 2,
        w: 8,
        h: 4,
      },
    ],
    goals: initialGoals,

    pinVisualToDashboard: (visualId, pageId, title) => {
      const existing = get().dashboardTiles.find(t => t.visualId === visualId);
      if (existing) return;
      const newTile: DashboardTile = {
        id: 'tile_' + Math.random().toString(36).substr(2, 6),
        visualId,
        pageId,
        title: title || 'Métrica Fixada',
        x: (get().dashboardTiles.length * 4) % 12,
        y: Math.floor(get().dashboardTiles.length / 3) * 3,
        w: 4,
        h: 3,
      };
      set({ dashboardTiles: [...get().dashboardTiles, newTile] });
    },

    removeDashboardTile: (tileId) => {
      set({ dashboardTiles: get().dashboardTiles.filter(t => t.id !== tileId) });
    },

    addGoal: (goal) => {
      set({ goals: [...get().goals, goal] });
    },

    updateGoal: (id, partial) => {
      set({
        goals: get().goals.map(g => (g.id === id ? { ...g, ...partial } : g)),
      });
    },

    // Power Query / ETL (v0.9.0 — ver docs/ETL_V09.md)
    appliedSteps: {},
    appliedQuerySnapshot: {},
    selectedStepByTable: {},

    getOrInitQuery: (tableId) => {
      const existing = get().appliedSteps[tableId];
      if (existing) return existing;
      const table = get().tables.find(t => t.id === tableId);
      const source: EtlSource = tableToEtlSource(table);
      return { name: table?.originalName || table?.name || tableId, source, steps: [] };
    },

    ensureQuery: async (tableId) => {
      const existing = get().appliedSteps[tableId];
      if (existing) return existing;
      const table = get().tables.find(t => t.id === tableId);
      if (!table) return get().getOrInitQuery(tableId);
      const name = table.originalName || table.name;
      const m = table.sourceConfig?.mScript;
      let query: EtlQuery | null = null;
      if (m && m.trim()) {
        // Tabela vinda de .pbip: as etapas do M viram etapas editáveis (round-trip).
        try {
          const parsed = await etlFromM({ name, m });
          const src = parsed.query.source;
          const fileKinds = ['excel', 'csv', 'parquet', 'json'];
          if (fileKinds.includes(src.kind) && table.sourceConfig?.filePath) {
            // O caminho do M costuma ser do Windows (\\tsclient\...); usa o já resolvido na importação.
            query = { ...parsed.query, name, source: { ...src, path: table.sourceConfig.filePath } };
          } else if (src.kind !== 'opaqueM' && !fileKinds.includes(src.kind)) {
            query = { ...parsed.query, name };
          }
          for (const w of parsed.warnings) console.warn(`[ETL] ${name}: ${w}`);
        } catch (e) {
          console.warn(`[ETL] ${name}: M não convertido, usando snapshot da tabela`, e);
        }
      }
      if (!query) query = { name, source: tableToEtlSource(table), steps: [] };
      // Outra chamada pode ter criado a consulta enquanto o M era convertido.
      const raced = get().appliedSteps[tableId];
      if (raced) return raced;
      get().setQuery(tableId, query);
      return query;
    },

    setQuery: (tableId, query) => {
      set({ appliedSteps: { ...get().appliedSteps, [tableId]: query } });
    },

    addQueryStep: (tableId, step) => {
      const query = get().getOrInitQuery(tableId);
      // Nomes de etapa são identificadores no M (#"Linhas Filtradas"): repetir
      // um nome faz a etapa referenciar a si mesma. Igual ao Power BI: sufixo 1, 2…
      step = { ...step, name: uniqueStepName(step.name, query.steps.map(s => s.name)) };
      get().setQuery(tableId, { ...query, steps: [...query.steps, step] });
      set({ selectedStepByTable: { ...get().selectedStepByTable, [tableId]: step.id } });
    },

    updateQueryStep: (tableId, stepId, patch) => {
      const query = get().getOrInitQuery(tableId);
      get().setQuery(tableId, {
        ...query,
        steps: query.steps.map(s => (s.id === stepId ? ({ ...s, ...patch } as EtlStep) : s)),
      });
    },

    removeQueryStep: (tableId, stepId) => {
      const query = get().getOrInitQuery(tableId);
      get().setQuery(tableId, { ...query, steps: query.steps.filter(s => s.id !== stepId) });
    },

    moveQueryStepUp: (tableId, stepId) => {
      const query = get().getOrInitQuery(tableId);
      const idx = query.steps.findIndex(s => s.id === stepId);
      if (idx <= 0) return;
      const steps = [...query.steps];
      [steps[idx - 1], steps[idx]] = [steps[idx], steps[idx - 1]];
      get().setQuery(tableId, { ...query, steps });
    },

    moveQueryStepDown: (tableId, stepId) => {
      const query = get().getOrInitQuery(tableId);
      const idx = query.steps.findIndex(s => s.id === stepId);
      if (idx === -1 || idx >= query.steps.length - 1) return;
      const steps = [...query.steps];
      [steps[idx], steps[idx + 1]] = [steps[idx + 1], steps[idx]];
      get().setQuery(tableId, { ...query, steps });
    },

    renameQueryStep: (tableId, stepId, name) => {
      const others = get().getOrInitQuery(tableId).steps.filter(s => s.id !== stepId).map(s => s.name);
      get().updateQueryStep(tableId, stepId, { name: uniqueStepName(name.trim() || 'Etapa', others) });
    },

    selectQueryStep: (tableId, stepId) => {
      set({ selectedStepByTable: { ...get().selectedStepByTable, [tableId]: stepId } });
    },

    closeAndApplyQuery: async (tableId) => {
      const query = get().getOrInitQuery(tableId);
      const table = get().tables.find(t => t.id === tableId);
      const payload = await etlApply({ query, targetTable: table?.originalName || table?.name || tableId });
      if (!payload) {
        // Consulta sem colunas: tabela que só hospeda medidas (como "Medidas"
        // no Power BI). Não há dados para carregar, mas a consulta está aplicada.
        set({ appliedQuerySnapshot: { ...get().appliedQuerySnapshot, [tableId]: query } });
        return { rowCount: 0, truncated: false, measuresOnly: true };
      }
      // Reaproveita o id da coluna de mesmo nome: medidas, relações e visuais
      // referenciam ids, e as etapas podem remover/reordenar colunas.
      const idByName = new Map((table?.columns || []).map(c => [c.name, c.id]));
      get().updateTable(tableId, {
        columns: payload.columns.map(c => ({
          id: idByName.get(c.name) || `c_${Math.random().toString(36).slice(2, 8)}`,
          name: c.name,
          originalName: c.originalName,
          dataType: (c.dataType as any) || 'text',
        })),
        rows: payload.rows,
        // Resultado materializado no DuckDB: o modelo Rust lê todas as linhas
        // dali (payload.rows é só a prévia de até 5.000).
        ...(payload.tableName ? { duckdbTable: payload.tableName } : {}),
        rowsTruncated: !!payload.truncated,
        totalRows: payload.totalRows ?? payload.rows.length,
      });
      set({ appliedQuerySnapshot: { ...get().appliedQuerySnapshot, [tableId]: query } });
      return { rowCount: payload.totalRows ?? payload.rows.length, truncated: !!payload.truncated, measuresOnly: false };
    },

    isQueryDirty: (tableId) => {
      const current = get().appliedSteps[tableId];
      if (!current) return false;
      const snapshot = get().appliedQuerySnapshot[tableId];
      // Nunca aplicada: só é "suja" se já tem etapas (consulta vazia = ainda nada a aplicar).
      if (!snapshot) return current.steps.length > 0;
      return JSON.stringify(current) !== JSON.stringify(snapshot);
    },

    hasPendingQueryChanges: () => {
      return get().tables.some(t => get().isQueryDirty(t.id));
    },

    applyAllDirtyQueries: async (onProgress) => {
      const dirtyIds = get().tables.map(t => t.id).filter(id => get().isQueryDirty(id));
      if (!dirtyIds.length) return { applied: [], errors: null };

      // Ordena por dependência: se a consulta de A referencia B (mergeQueries/appendQueries),
      // B deve ser aplicada primeiro — senão A lê um B ainda não atualizado.
      const nameToId = new Map(get().tables.map(t => [t.originalName || t.name, t.id] as const));
      const depsOf: Record<string, string[]> = {};
      dirtyIds.forEach(id => {
        const q = get().appliedSteps[id];
        const ds = new Set<string>();
        (q?.steps || []).forEach(s => {
          if (s.op === 'mergeQueries') {
            const depId = nameToId.get(s.rightQuery);
            if (depId && depId !== id) ds.add(depId);
          }
          if (s.op === 'appendQueries') {
            s.queries.forEach(n => {
              const depId = nameToId.get(n);
              if (depId && depId !== id) ds.add(depId);
            });
          }
        });
        depsOf[id] = Array.from(ds);
      });
      const ordered: string[] = [];
      const visiting = new Set<string>();
      const visited = new Set<string>();
      const visit = (id: string) => {
        if (visited.has(id) || visiting.has(id)) return; // guarda contra ciclos
        visiting.add(id);
        (depsOf[id] || []).forEach(d => {
          if (dirtyIds.includes(d)) visit(d);
        });
        visiting.delete(id);
        visited.add(id);
        ordered.push(id);
      };
      dirtyIds.forEach(visit);

      const applied: ApplyAllQueriesResult['applied'] = [];
      for (let i = 0; i < ordered.length; i++) {
        const tableId = ordered[i];
        const table = get().tables.find(t => t.id === tableId);
        const name = table?.originalName || table?.name || tableId;
        onProgress?.({ name, index: i + 1, total: ordered.length });
        try {
          const res = await get().closeAndApplyQuery(tableId);
          applied.push({ tableId, name, rowCount: res.rowCount, truncated: res.truncated, measuresOnly: res.measuresOnly });
        } catch (e: any) {
          const idx = get().appliedSteps[tableId]?.steps.length;
          return {
            applied,
            errors: { tableId, name, stepIndex: e?.stepIndex ?? idx, message: e?.message || String(e) },
          };
        }
      }
      return { applied, errors: null };
    },

    refreshEtlTable: async (tableId) => {
      const table = get().tables.find(t => t.id === tableId);
      const query = get().appliedSteps[tableId];
      if (!table || !query) return;
      const pbipRoot = table.sourceConfig?.pbipRoot || get().sourcePbipPath || undefined;
      await withTimeout(etlLoadRaw({ query: query.name, source: query.source, pbipRoot }), `Fonte de "${table.name}"`);
      await withTimeout(get().closeAndApplyQuery(tableId), `Consulta de "${table.name}"`);
    },

    refreshTableData: async (tableId) => {
      const table = get().tables.find(t => t.id === tableId);
      if (!table) return;
      const query = get().appliedSteps[tableId];
      if (query) {
        await get().refreshEtlTable(tableId);
      } else {
        await etlLoadRaw({ query: table.originalName || table.name, source: tableToEtlSource(table) });
      }
    },

    // Governance & RLS
    rlsRoles: [
      {
        id: 'role_vendedor_sp',
        name: 'Vendedor Regional (São Paulo)',
        rules: [{ tableId: 'f_Vendas', filterExpression: '[Estado] = "SP"' }],
      },
      {
        id: 'role_gestor_sul',
        name: 'Gerente Comercial (Região Sul)',
        rules: [{ tableId: 'f_Vendas', filterExpression: '[Regiao] = "Sul"' }],
      },
    ],
    activeRole: null,
    setActiveRole: (roleId) => {
      const foundRole = roleId ? get().rlsRoles.find(r => r.id === roleId) || null : null;
      DaxEngine.setActiveRole(foundRole);
      set({ activeRole: roleId });
    },
    calculationGroups: [],
    setCalculationGroups: (groups) => {
      DaxEngine.setCalculationGroups(groups);
      set({ calculationGroups: groups });
    },
    activeUser: {
      username: 'Admin',
      userPrincipalName: 'admin@insighthub.local',
      userObjectId: '00000000-0000-0000-0000-000000000001',
      userCulture: 'pt-BR',
      customData: '',
    },
    setActiveUser: (user) => {
      DaxEngine.setActiveUser(user);
      set({ activeUser: user });
    },

    whatIfParameters: initialWhatIf,
    updateWhatIfValue: (id, val) => {
      const updatedParams = get().whatIfParameters.map(p => (p.id === id ? { ...p, currentValue: val } : p));
      const synced = syncWhatIfWithModel(updatedParams, get().tables, get().measures);
      set({
        whatIfParameters: updatedParams,
        tables: synced.tables,
        measures: synced.measures,
      });
      globalDataEngine.setSchema(synced.tables, get().relationships, synced.measures);
    },

    addWhatIfParameter: (param) => {
      const updatedParams = [...get().whatIfParameters, param];
      const synced = syncWhatIfWithModel(updatedParams, get().tables, get().measures);
      set({
        whatIfParameters: updatedParams,
        tables: synced.tables,
        measures: synced.measures,
      });
      globalDataEngine.setSchema(synced.tables, get().relationships, synced.measures);
    },

    // User Defined Functions (UDF)
    userDefinedFunctions: [
      {
        id: 'udf_default_margin',
        name: 'MargemPercentual',
        parameters: ['receita', 'custo'],
        body: 'DIVIDE(receita - custo, receita, 0)',
        description: 'Calcula o percentual de margem bruta entre receita e custo',
        category: 'Financeiro',
        returnType: 'Percentage',
      },
      {
        id: 'udf_default_ticket',
        name: 'TicketMedio',
        parameters: ['faturamento', 'qtdPedidos'],
        body: 'DIVIDE(faturamento, qtdPedidos, 0)',
        description: 'Calcula o ticket médio por pedido',
        category: 'Comercial',
        returnType: 'Currency',
      },
    ],
    setUserDefinedFunctions: (udfs) => {
      DaxEngine.setUserDefinedFunctions(udfs);
      set({ userDefinedFunctions: udfs });
    },
    addUserDefinedFunction: (udf) => {
      const updated = [...get().userDefinedFunctions.filter(u => u.name.toLowerCase() !== udf.name.toLowerCase()), udf];
      DaxEngine.setUserDefinedFunctions(updated);
      set({ userDefinedFunctions: updated });
    },
    updateUserDefinedFunction: (id, partial) => {
      const updated = get().userDefinedFunctions.map(u => (u.id === id ? { ...u, ...partial } : u));
      DaxEngine.setUserDefinedFunctions(updated);
      set({ userDefinedFunctions: updated });
    },
    deleteUserDefinedFunction: (id) => {
      const updated = get().userDefinedFunctions.filter(u => u.id !== id);
      DaxEngine.setUserDefinedFunctions(updated);
      set({ userDefinedFunctions: updated });
    },

    // DAX Query Tabs
    daxQueries: [
      {
        id: 'query_1',
        title: 'Consulta 1',
        query: `DEFINE\n  MEASURE f_Vendas[Margem] = [Faturamento Total] - [Custo Total Mercadorias]\n\nEVALUATE\n  SUMMARIZECOLUMNS(\n    'd_Calendario'[Ano],\n    "Faturamento", [Faturamento Total],\n    "Margem", [Margem]\n  )\nORDER BY\n  'd_Calendario'[Ano] ASC`,
      },
    ],
    activeDaxQueryTabId: 'query_1',
    setDaxQueries: (queries) => set({ daxQueries: queries }),
    setActiveDaxQueryTabId: (id) => set({ activeDaxQueryTabId: id }),
    addDaxQueryTab: (title, initialQuery) => {
      const id = 'query_' + Math.random().toString(36).substring(2, 8);
      const newTab: DaxQueryTab = {
        id,
        title: title || `Consulta ${get().daxQueries.length + 1}`,
        query: initialQuery || `EVALUATE\n  SUMMARIZECOLUMNS(\n    'd_Produtos'[Categoria],\n    "Total", [Faturamento Total]\n  )\nORDER BY\n  [Total] DESC`,
      };
      set({ daxQueries: [...get().daxQueries, newTab], activeDaxQueryTabId: id });
      return id;
    },
    closeDaxQueryTab: (id) => {
      if (get().daxQueries.length <= 1) return;
      const remaining = get().daxQueries.filter(q => q.id !== id);
      set({
        daxQueries: remaining,
        activeDaxQueryTabId: get().activeDaxQueryTabId === id ? remaining[0].id : get().activeDaxQueryTabId,
      });
    },
    updateDaxQueryText: (id, query) => {
      set({
        daxQueries: get().daxQueries.map(q => (q.id === id ? { ...q, query, isDirty: true } : q)),
      });
    },

    // Performance Analyzer
    performanceMetrics: [],
    recordPerformanceMetric: (metric) => {
      set({
        performanceMetrics: [metric, ...get().performanceMetrics.slice(0, 19)],
      });
    },

    // Theme & Appearance (Gerenciador de Temas & Dark Mode)
    theme: 'powerbi-light',
    activeThemeId: 'powerbi-classic',
    customThemes: [],
    activePalette: PRESET_THEMES[0].palette,
    themeModalOpen: false,
    themeMode: initialThemeMode,
    isDarkMode: initialIsDark,
    setThemeModalOpen: (open) => set({ themeModalOpen: open }),
    setTheme: (theme) => set({ theme }),
    setThemeId: (themeId) => {
      const all = [...PRESET_THEMES, ...get().customThemes];
      const found = all.find(t => t.id === themeId);
      if (found) {
        set({ activeThemeId: themeId, activePalette: found.palette });
      } else {
        set({ activeThemeId: themeId });
      }
    },
    setThemeMode: (mode: ThemeMode) => {
      setStoredThemeMode(mode);
      const isDark = applyThemeToDocument(mode);
      set({ themeMode: mode, isDarkMode: isDark });
    },
    toggleThemeMode: () => {
      const current = get().themeMode;
      const next: ThemeMode = current === 'dark' ? 'light' : 'dark';
      setStoredThemeMode(next);
      const isDark = applyThemeToDocument(next);
      set({ themeMode: next, isDarkMode: isDark });
    },
    addCustomTheme: (theme) => {
      const updated = [...get().customThemes.filter(t => t.id !== theme.id), theme];
      set({
        customThemes: updated,
        activeThemeId: theme.id,
        activePalette: theme.palette,
      });
    },
    deleteCustomTheme: (themeId) => {
      const updated = get().customThemes.filter(t => t.id !== themeId);
      const nextActive = get().activeThemeId === themeId ? 'powerbi-classic' : get().activeThemeId;
      const all = [...PRESET_THEMES, ...updated];
      const activeObj = all.find(t => t.id === nextActive) || PRESET_THEMES[0];
      set({
        customThemes: updated,
        activeThemeId: activeObj.id,
        activePalette: activeObj.palette,
      });
    },

    // Grid & Canvas Settings
    gridSize: 20,
    setGridSize: (gridSize) => set({ gridSize }),
    showGrid: true,
    setShowGrid: (showGrid) => set({ showGrid }),
    snapToGrid: true,
    setSnapToGrid: (snapToGrid) => set({ snapToGrid }),

    // Modals
    dataSourceModalOpen: false,
    setDataSourceModalOpen: (open) => set({ dataSourceModalOpen: open }),
    quickMeasureModalOpen: false,
    setQuickMeasureModalOpen: (open) => set({ quickMeasureModalOpen: open }),
    customMeasureModalOpen: false,
    setCustomMeasureModalOpen: (open) => set({ customMeasureModalOpen: open }),
    limitationsModalOpen: false,
    setLimitationsModalOpen: (open) => set({ limitationsModalOpen: open }),
    aiModalOpen: false,
    setAiModalOpen: (open) => set({ aiModalOpen: open }),
    aiSettingsModalOpen: false,
    setAiSettingsModalOpen: (open) => set({ aiSettingsModalOpen: open }),
    canvasSizeModalOpen: false,
    setCanvasSizeModalOpen: (open) => set({ canvasSizeModalOpen: open }),
    updateModalOpen: false,
    setUpdateModalOpen: (open) => set({ updateModalOpen: open }),
    availableUpdate: null,
    setAvailableUpdate: (availableUpdate) => set({ availableUpdate }),

    // Welcome Screen & Recent Projects
    showWelcomeScreen: true,
    setShowWelcomeScreen: (showWelcomeScreen) => set({ showWelcomeScreen }),
    recentProjects: loadRecentProjects(),
    addRecentProject: (entry) => {
      const next = [entry, ...get().recentProjects.filter(r => r.id !== entry.id && !(entry.path && r.path === entry.path))].slice(0, 8);
      saveRecentProjects(next);
      set({ recentProjects: next });
    },
    removeRecentProject: (id) => {
      const next = get().recentProjects.filter(r => r.id !== id);
      saveRecentProjects(next);
      set({ recentProjects: next });
    },

    // State actions
    newProject: (name?: string) => {
      const blankPageId = 'page_' + Math.random().toString(36).substr(2, 6);
      const blankState = {
        projectName: name || 'Novo Projeto em Branco',
        sourcePbipPath: null,
        refreshStatus: null,
        dataVersion: get().dataVersion + 1,
        tables: [],
        relationships: [],
        measures: [],
        pages: [
          {
            id: blankPageId,
            name: 'Página 1',
            order: 1,
            visuals: [],
          },
        ],
        activePageId: blankPageId,
        selectedVisualId: null,
        selectedVisualIds: [],
        historyPast: [],
        historyFuture: [],
        canUndo: false,
        canRedo: false,
        activeView: 'report' as ActiveView,
        filters: [],
        bookmarks: [],
        dashboardTiles: [],
        goals: [],
        appliedSteps: {},
        appliedQuerySnapshot: {},
        performanceMetrics: [],
        lastSavedAt: null,
      };
      set(blankState);
      globalDataEngine.setSchema([], [], []);
      try {
        localStorage.removeItem('insighthub_project_backup');
      } catch {}
    },

    executeDuckDbQuery: async (sql: string) => {
      return await globalDuckDb.query(sql);
    },

    // Lifecycle & Persistence
    loadInitialSampleData: () => {
      const sample = getSampleDatasets();
      const shouldRestoreVisuals = get().pages.every(p => p.visuals.length === 0);
      set(s => ({
        projectName: shouldRestoreVisuals ? 'PrismBI Demo — Varejo Brasil' : s.projectName,
        tables: sample.tables,
        relationships: sample.relationships,
        measures: sample.measures,
        pages: shouldRestoreVisuals
          ? [{ id: defaultPageId, name: 'Visão Executiva', order: 1, visuals: initialVisuals }]
          : s.pages,
        activePageId: shouldRestoreVisuals ? defaultPageId : s.activePageId,
        sourcePbipPath: null,
        dataVersion: s.dataVersion + 1,
      }));
      globalDataEngine.setSchema(sample.tables, sample.relationships, sample.measures);
      sample.tables.forEach(t => globalDuckDb.registerTable(t));
      pushModelToRustAsync(sample.tables, sample.relationships, sample.measures, () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
    },

    generateCalendar: (startDate = '2025-01-01', endDate = '2026-12-31') => {
      const cal = generateCalendarTable(startDate, endDate);
      const existingIdx = get().tables.findIndex(t => t.id === 'd_Calendario');
      if (existingIdx >= 0) {
        get().updateTable('d_Calendario', cal);
      } else {
        get().addTable(cal);
      }
      globalDuckDb.registerTable(cal);
    },

    getProjectState: () => {
      return {
        version: '1.0.0',
        projectName: get().projectName,
        sourcePbipPath: get().sourcePbipPath,
        tables: get().tables,
        relationships: get().relationships,
        measures: get().measures,
        pages: get().pages,
        activePageId: get().activePageId,
        filters: get().filters,
        bookmarks: get().bookmarks,
        dashboardTiles: get().dashboardTiles,
        goals: get().goals,
        rlsRoles: get().rlsRoles,
        whatIfParameters: get().whatIfParameters,
        calculationGroups: get().calculationGroups,
        userDefinedFunctions: get().userDefinedFunctions,
        daxQueries: get().daxQueries,
        theme: get().theme,
        activeThemeId: get().activeThemeId,
        customThemes: get().customThemes,
        gridSize: get().gridSize,
        showGrid: get().showGrid,
        snapToGrid: get().snapToGrid,
        language: get().language,
        currency: get().currency,
        appliedSteps: get().appliedSteps,
        appliedQuerySnapshot: get().appliedQuerySnapshot,
        updatedAt: new Date().toISOString(),
      };
    },

    saveProject: async () => {
      const state = get().getProjectState();
      const res = await saveProjectToStorage(state);
      if (res.success) {
        set({ lastSavedAt: `Salvo às ${res.timestamp}` });
      } else {
        console.error('Falha ao salvar projeto:', res.error);
        set({ lastSavedAt: 'Falha ao salvar' });
      }
    },

    loadProject: async () => {
      const state = await loadProjectFromStorage();
      if (!state) return false;
      get().loadStateFromProject(state);
      return true;
    },

    loadStateFromProject: (rawState: ProjectState) => {
      const state = migrateProjectState(rawState);
      const allThemes = [...PRESET_THEMES, ...(state.customThemes || [])];
      const matchedTheme = allThemes.find(t => t.id === state.activeThemeId) || PRESET_THEMES[0];

      if (state.language) {
        setGlobalLocale(state.language as SupportedLocale);
      }
      if (state.currency) {
        setGlobalCurrency(state.currency as SupportedCurrency);
      }

      const udfs = state.userDefinedFunctions || [];
      DaxEngine.setUserDefinedFunctions(udfs);

      const daxQueries = (state.daxQueries && state.daxQueries.length > 0) ? state.daxQueries : get().daxQueries;

      set(s => ({
        language: (state.language as SupportedLocale) || get().language,
        currency: (state.currency as SupportedCurrency) || get().currency,
        projectName: state.projectName || 'Projeto InsightHub',
        sourcePbipPath: state.sourcePbipPath ?? null,
        refreshStatus: null,
        dataVersion: s.dataVersion + 1,
        tables: state.tables || [],
        relationships: state.relationships || [],
        measures: state.measures || [],
        pages: state.pages || [],
        activePageId: state.activePageId || state.pages?.[0]?.id || defaultPageId,
        selectedVisualId: null,
        selectedVisualIds: [],
        historyPast: [],
        historyFuture: [],
        canUndo: false,
        canRedo: false,
        filters: state.filters || [],
        bookmarks: state.bookmarks || [],
        dashboardTiles: state.dashboardTiles || [],
        goals: state.goals || [],
        rlsRoles: state.rlsRoles || [],
        whatIfParameters: state.whatIfParameters || [],
        calculationGroups: state.calculationGroups || [],
        userDefinedFunctions: udfs,
        daxQueries,
        activeDaxQueryTabId: daxQueries[0]?.id || 'query_1',
        theme: state.theme || 'powerbi-light',
        activeThemeId: state.activeThemeId || matchedTheme.id,
        customThemes: state.customThemes || [],
        activePalette: matchedTheme.palette,
        gridSize: state.gridSize || 20,
        showGrid: state.showGrid !== undefined ? state.showGrid : true,
        snapToGrid: state.snapToGrid !== undefined ? state.snapToGrid : true,
        appliedSteps: state.appliedSteps || {},
        appliedQuerySnapshot: state.appliedQuerySnapshot || {},
        lastSavedAt: new Date().toLocaleTimeString('pt-BR'),
      }));
      globalDataEngine.setSchema(state.tables || [], state.relationships || [], state.measures || []);
      (state.tables || []).forEach(t => globalDuckDb.registerTable(t));
      get().recalculateRelativeDateFilters();
      pushModelToRustAsync(state.tables || [], state.relationships || [], state.measures || [], () =>
        set(s => ({ dataVersion: s.dataVersion + 1 }))
      );
      // DuckDB é em memória: ao reabrir, tabelas maiores que a amostra
      // (5.000 linhas) só existem como prévia. Recarrega das fontes em segundo
      // plano para o modelo voltar a ter todas as linhas.
      if (isTauriRuntime() && (state.tables || []).some(t => t.rowsTruncated)) {
        setTimeout(() => {
          void get().refreshAllData();
        }, 0);
      }
    },
  };
});

if (typeof window !== 'undefined') {
  initSystemThemeListener((isSystemDark) => {
    if (useBiStore.getState().themeMode === 'system') {
      applyThemeToDocument('system');
      useBiStore.setState({ isDarkMode: isSystemDark });
    }
  });

  // Expõe as tabelas atuais do modelo ao fallback de ETL do navegador
  // (usado quando o app não roda dentro do Tauri).
  setBrowserRawProvider((name) => {
    const table = useBiStore
      .getState()
      .tables.find(t => t.originalName === name || t.name === name || t.id === name);
    if (!table) return undefined;
    return {
      columns: table.columns.map(c => ({ name: c.name, type: c.dataType })),
      rows: table.rows,
    };
  });
}

