export type DataType = 'text' | 'number' | 'date' | 'boolean' | 'currency' | 'percentage';

export interface Column {
  id: string;
  name: string;
  originalName: string;
  dataType: DataType;
  dataCategory?: 'city' | 'state' | 'country' | 'url' | 'none';
  format?: string;
  sortByColumn?: string;
  isHidden?: boolean;
  isCalculated?: boolean;
  expression?: string;
  description?: string;
}

export interface ColumnProfile {
  columnName: string;
  totalCount: number;
  distinctCount: number;
  nullCount: number;
  validPercent: number;
  nullPercent: number;
  sampleDistribution: { value: string; count: number }[];
  minValue?: any;
  maxValue?: any;
  errorCount?: number;
}

export interface TableSourceConfig {
  type: 'xlsx' | 'csv' | 'parquet' | 'json' | 'pbip' | 'rest' | 'sample' | 'db';
  filePath?: string;
  sheetName?: string;
  mScript?: string;
  pbipRoot?: string;
  url?: string;
  lastRefreshedAt?: string;
  /** Presente apenas quando type === 'db' (conectores nativos v0.8.0). */
  db?: DbSourceConfig;
}

export type DbConnectorKind = 'sqlite' | 'duckdb_file' | 'postgres' | 'mysql' | 'parquet_glob';

/**
 * Configuração de um conector de banco de dados nativo (via DuckDB
 * ATTACH). NUNCA contém senha — a senha fica apenas no keyring do Linux
 * (Secret Service), referenciada por `credentialKey`. Ao reimportar via
 * "Atualizar Dados", o Rust reconecta buscando a senha pelo
 * `credentialKey` (ver `db_refresh_import` em `connectors::manager`).
 */
export interface DbSourceConfig {
  kind: DbConnectorKind;
  /** Chave estável no keyring (gerada uma vez na primeira conexão); ausente para SQLite/.duckdb/Parquet (sem senha). */
  credentialKey?: string;
  /** Parâmetros de conexão sem senha, no mesmo shape de `ConnectorConfig` do Rust (tag "kind"). */
  config: Record<string, any>;
  /** Origem dos dados importados: tabela do catálogo ou consulta SQL livre. */
  source: { mode: 'table'; schema: string; table: string } | { mode: 'sql'; sql: string };
}

export interface Table {
  id: string;
  name: string;
  originalName: string;
  columns: Column[];
  rows: Record<string, any>[];
  sourceConfig?: TableSourceConfig;
  isCalculated?: boolean;
  expression?: string;
  isCalendar?: boolean;
  calendarDateColumn?: string;
  fiscalYearEndMonth?: number;
  calendarDiagnostics?: {
    isValid: boolean;
    isMarkedAsDateTable: boolean;
    dateColumn?: string;
    minDate?: string;
    maxDate?: string;
    totalDays: number;
    expectedDays: number;
    hasNulls: boolean;
    hasDuplicates: boolean;
    isContinuous: boolean;
    errors: string[];
    warnings: string[];
  };
  autoDateTimeEnabled?: boolean;
  isHidden?: boolean;
  isDim?: boolean;
  x?: number; // for model view coordinates
  y?: number;
  /** DuckDB table name backing this table's data (CSV/Parquet/JSON/XLSX
   * import, .pbip load, DB connector, ETL apply/refresh). When set, `rows`
   * is only a UI preview (capped, see PREVIEW_ROW_LIMIT in Rust) and the
   * Rust DAX model loads the full table straight from DuckDB instead of
   * `rows` — see docs/RUST_ENGINE_2.md §3 step 2. Absent for sample data,
   * manual entry and other tables with no DuckDB-backed source. */
  duckdbTable?: string;
  /** True when `rows` is a truncated preview of a larger DuckDB table
   * (i.e. totalRows > rows.length). Drives the "⚠ amostra de 5.000 linhas"
   * indicator on visuals that still compute in JS off `rows`. */
  rowsTruncated?: boolean;
  /** Total row count in the backing DuckDB table (when known), even though
   * `rows` itself is capped at PREVIEW_ROW_LIMIT. */
  totalRows?: number;
}

export interface Relationship {
  id: string;
  fromTable: string;
  fromColumn: string;
  toTable: string;
  toColumn: string;
  cardinality: '1:N' | 'N:1' | '1:1' | 'N:N';
  crossFilterDirection: 'single' | 'both';
  isActive: boolean;
}

export interface Measure {
  id: string;
  name: string;
  tableName: string;
  tableId?: string;
  formula: string;
  format: 'currency' | 'number' | 'percentage' | 'integer';
  formatString?: string;
  description?: string;
  displayFolder?: string;
}

export type VisualType =
  | 'column'
  | 'stacked-column'
  | 'bar'
  | 'stacked-bar'
  | 'line'
  | 'area'
  | 'combo'
  | 'pie'
  | 'donut'
  | 'treemap'
  | 'waterfall'
  | 'funnel'
  | 'scatter'
  | 'gauge'
  | 'card'
  | 'multi-card'
  | 'kpi'
  | 'table'
  | 'matrix'
  | 'slicer'
  | 'action-button'
  | 'textbox';

export interface ConditionalFormatRule {
  operator: '>' | '<' | '>=' | '<=' | '==' | 'between';
  value: number;
  value2?: number;
  color: string;
}

// --- Bloco B (v0.10, Frente "Formatação condicional") ---
export type CondFormatTarget = 'background' | 'fontColor' | 'dataBar' | 'icon';
export interface CondFormatStop { kind: 'lowest' | 'highest' | 'number' | 'percent'; value?: number; color: string; }
export interface ConditionalFormat {
  id: string;
  target: CondFormatTarget;
  /** Campo do visual que recebe a formatação (nome da coluna/medida como aparece em yAxisFields/xAxisField). */
  applyTo: string;
  /** Valor que decide a cor; padrão = o próprio campo. */
  basedOn?: { field: string; kind: 'measure' | 'column'; aggregation?: VisualDataConfig['aggregation'] };
  mode: 'rules' | 'gradient' | 'fieldValue';
  rules?: Array<ConditionalFormatRule & { valueKind?: 'number' | 'percent' }>;
  gradient?: { min: CondFormatStop; mid?: CondFormatStop; max: CondFormatStop };
  dataBar?: { positiveColor: string; negativeColor: string; showBarOnly?: boolean };
  icons?: { set: 'arrows' | 'traffic' | 'flags'; rules: ConditionalFormatRule[] };
  /** v0.10 Frente H: "Aplicar a" da matriz — Somente valores (padrão/ausente)
   * ou Valores e totais. Quando `true`, totais/subtotais também recebem a
   * formatação, com min/max (gradiente) calculados sobre a própria escala
   * dos totais — ver `styleForTotalCell` em conditionalFormat/apply.ts. */
  applyToTotals?: boolean;
}
// --- fim Bloco B ---

export interface VisualFormatConfig {
  showTitle: boolean;
  title?: string;
  titleColor?: string;
  titleFontSize?: number;
  backgroundColor?: string;
  borderColor?: string;
  borderRadius?: number;
  showDataLabels?: boolean;
  showLegend?: boolean;
  colorPalette?: string[];
  isTransparent?: boolean;
  showBlankAsZero?: boolean;
  cardColumns?: number;
  displayUnits?: 'none' | 'auto';
  colorBySign?: boolean;
  textContent?: string;
  actionType?: 'back' | 'page-navigation' | 'bookmark';
  actionTargetPageId?: string;
  /** v0.10 (Indicadores): id do Bookmark aplicado por este botão quando actionType === 'bookmark'. */
  actionBookmarkId?: string;
  /** v0.10 (Indicadores/Seleção): visual oculto do canvas; alternado pelo painel "Seleção" ou por um indicador. */
  hidden?: boolean;
  /** @deprecated legado (pré v0.10), migrado para `conditionalFormats` em src/engine/migrations/condfmt.ts */
  conditionalFormatting?: {
    field: string;
    rules: ConditionalFormatRule[];
  };
  /** v0.10: formatação condicional completa (gradiente, regras, valor de campo, barras, ícones). */
  conditionalFormats?: ConditionalFormat[];
}

export interface VisualCalculation {
  id: string;
  name: string;
  formula: string;
  format?: 'currency' | 'number' | 'percentage' | 'integer';
  description?: string;
  axis?: 'ROWS' | 'COLUMNS';
}

export interface VisualDataConfig {
  tableId: string;
  xAxisField?: string;
  yAxisFields?: string[];
  legendField?: string;
  tooltipFields?: string[];
  aggregation?: 'SUM' | 'AVG' | 'COUNT' | 'DISTINCTCOUNT' | 'MIN' | 'MAX' | 'NONE';
  sortField?: string;
  sortOrder?: 'asc' | 'desc';
  limit?: number;
  drillFields?: string[];
  currentDrillLevel?: number;
  kpiTarget?: number;
  kpiComparisonField?: string;
  visualCalculations?: VisualCalculation[];
  slicerConfig?: {
    mode: 'list' | 'dropdown' | 'slider' | 'relative-date' | 'tiles' | 'between';
    selectedValues?: any[];
    /** @deprecated migrated to `relative` by src/engine/migrations/slicers.ts */
    dateRange?: {
      start?: string;
      end?: string;
      relativePreset?: 'last7days' | 'last30days' | 'thisMonth' | 'thisYear' | 'all';
    };
    /** 'between' mode range. ISO date string for date columns, number for numeric columns. */
    between?: { start?: string | number; end?: string | number };
    /** 'relative-date' mode definition. Re-resolved to a concrete range on every load via src/engine/relativeDate.ts. */
    relative?: RelativeDateFilter;
    /** Search box on top of list/dropdown/tiles modes. */
    showSearch?: boolean;
    /** Slicers with the same syncGroup share their selection (Power BI "Sincronizar segmentações"). */
    syncGroup?: string;
    /** Pages where the sync applies; empty/absent = all pages. */
    syncPages?: string[];
  };
}

export type RelativeDateUnit = 'days' | 'weeks' | 'months' | 'quarters' | 'years';
export interface RelativeDateFilter {
  anchor: 'last' | 'this' | 'next';
  count: number;
  unit: RelativeDateUnit;
  includeToday?: boolean;
}

export interface Visual {
  id: string;
  type: VisualType;
  title: string;
  x: number;
  y: number;
  w: number;
  h: number;
  zIndex?: number;
  dataConfig: VisualDataConfig;
  formatConfig: VisualFormatConfig;
  visualCalculations?: VisualCalculation[];
}

export interface DrillthroughConfig {
  isDrillthroughPage?: boolean;
  targetFields?: string[];
  keepAllFilters?: boolean;
}

export interface ActiveDrillthrough {
  sourcePageId: string;
  sourcePageName: string;
  sourceVisualTitle: string;
  field: string;
  value: any;
  filterId: string;
}

export type CanvasSizePreset =
  | 'default'
  | '16:9'
  | '16:9_fhd'
  | '4:3'
  | 'letter_portrait'
  | 'letter_landscape'
  | 'tooltip'
  | 'custom';

export interface CanvasSizeConfig {
  preset: CanvasSizePreset;
  width: number;
  height: number;
}

export interface Page {
  id: string;
  name: string;
  order: number;
  isHidden?: boolean;
  visuals: Visual[];
  backgroundColor?: string;
  layoutMode?: 'desktop' | 'mobile';
  drillthroughConfig?: DrillthroughConfig;
  canvasSize?: CanvasSizeConfig;
}

export interface Filter {
  id: string;
  level: 'visual' | 'page' | 'report';
  visualId?: string;
  tableId: string;
  column: string;
  operator: 'in' | 'notIn' | 'equals' | 'notEquals' | 'greaterThan' | 'greaterThanOrEqual' | 'lessThan' | 'lessThanOrEqual' | 'between' | 'contains';
  value: any;
}

/** Per-visual display state captured by a Bookmark (capture.display), Power BI style. */
export interface BookmarkVisualState {
  hidden?: boolean;
  drillLevel?: number;
  sortField?: string;
  sortOrder?: 'asc' | 'desc';
}

export interface Bookmark {
  id: string;
  name: string;
  activePageId: string;
  filters: Filter[];
  /** Posição na lista do painel Indicadores (ordenação manual/arrastar). */
  order?: number;
  /** Nome do grupo de indicadores (indicadores com o mesmo valor pertencem ao mesmo grupo). */
  groupId?: string;
  /** O que este indicador captura/restaura, como "Dados / Exibição / Página atual" no Power BI. */
  capture?: { data: boolean; display: boolean; currentPage: boolean };
  /** Estado de exibição por visual (visibilidade, nível de drill, ordenação). */
  visualState?: Record<string, BookmarkVisualState>;
  /** visualId -> seleção do segmentador (selectedValues/between/relative de slicerConfig). */
  slicerSelections?: Record<string, unknown>;
  /** Visuais afetados: 'all' (todos) ou a lista de ids dos visuais selecionados na captura. */
  scope?: 'all' | string[];
}

export interface BiTheme {
  id: string;
  name: string;
  palette: string[];
  backgroundColor?: string;
  cardBackground?: string;
  borderColor?: string;
  textColor?: string;
  isCustom?: boolean;
}

export interface PowerQueryStep {
  id: string;
  name: string;
  type:
    | 'source'
    | 'rename'
    | 'changeType'
    | 'filter'
    | 'removeColumns'
    | 'removeColumn'
    | 'removeOtherColumns'
    | 'removeTopRows'
    | 'removeBottomRows'
    | 'removeBlankRows'
    | 'keepTopRows'
    | 'keepBottomRows'
    | 'useFirstRowAsHeaders'
    | 'useHeadersAsFirstRow'
    | 'detectDataTypes'
    | 'addCustomColumn'
    | 'conditionalColumn'
    | 'replaceValues'
    | 'splitColumn'
    | 'groupBy'
    | 'pivot'
    | 'unpivot'
    | 'sort'
    | 'deduplicate'
    | 'fillDown'
    | 'fillUp'
    | 'textTransform'
    | 'textExtract'
    | 'dateExtract'
    | 'addIndexColumn'
    | 'duplicateColumn';
  params: Record<string, any>;
  description?: string;
}

export interface TableQueryPipeline {
  tableId: string;
  steps: PowerQueryStep[];
}

export interface DashboardTile {
  id: string;
  visualId: string;
  pageId: string;
  title: string;
  x: number;
  y: number;
  w: number;
  h: number;
  alertThreshold?: number;
  alertCondition?: 'above' | 'below';
}

export interface Goal {
  id: string;
  name: string;
  target: number;
  current: number;
  unit: 'R$' | '%' | 'unidades' | 'horas' | 'km';
  owner: string;
  deadline: string;
  status: 'on_track' | 'at_risk' | 'delayed';
  history: { date: string; value: number }[];
}

export interface CalculationItem {
  id: string;
  name: string;
  expression: string; // DAX expression using SELECTEDMEASURE()
  formatStringExpression?: string;
  ordinal?: number;
  description?: string;
}

export interface CalculationGroup {
  id: string;
  name: string;
  precedence: number;
  columnName?: string; // default: "Calculation Item"
  calculationItems: CalculationItem[];
  description?: string;
}

export interface RlsRole {
  id: string;
  name: string;
  rules: {
    tableId: string;
    filterExpression: string; // e.g. "[Regiao] = \"Sudeste\""
  }[];
  olsRules?: {
    tableId: string;
    column?: string; // if omitted, entire table is restricted
    access: 'none' | 'read';
  }[];
  members?: string[];
}

export interface WhatIfParameter {
  id: string;
  name: string;
  min: number;
  max: number;
  step: number;
  defaultValue: number;
  currentValue: number;
  unit: string;
}

export interface DataSourceConnection {
  id: string;
  name: string;
  type: 'file' | 'rest_api' | 'postgres' | 'mysql' | 'sqlserver' | 'oracle' | 'gateway';
  status: 'connected' | 'disconnected' | 'testing';
  config: {
    host?: string;
    database?: string;
    port?: number;
    username?: string;
    url?: string;
    headers?: Record<string, string>;
    gatewayUrl?: string;
    environment?: 'dev' | 'test' | 'prod';
  };
}

export interface PerformanceMetric {
  visualId: string;
  visualTitle: string;
  queryTimeMs: number;
  renderTimeMs: number;
  rowCount: number;
}

export type ActiveView = 'report' | 'data' | 'model' | 'query' | 'metabase' | 'dashboard' | 'governance' | 'daxquery' | 'daxcompliance';

export interface UserDefinedFunction {
  id: string;
  name: string;
  parameters: string[];
  body: string;
  description?: string;
  category?: string;
  returnType?: string;
  createdAt?: string;
  updatedAt?: string;
}

export interface DaxQueryExecutionStage {
  name: string;
  durationMs: number;
  details?: string;
  status: 'success' | 'warning' | 'error';
}

export interface DaxQueryResultItem {
  statementIndex: number;
  table?: any; // DaxTable
  scalar?: any;
  rowCount?: number;
  columns?: string[];
  rows?: Record<string, any>[];
}

export interface DaxQueryExecutionResult {
  success: boolean;
  results: DaxQueryResultItem[];
  executionPlan: DaxQueryExecutionStage[];
  totalDurationMs: number;
  error?: string;
  definedObjects?: {
    measures: { table: string; name: string; formula: string }[];
    columns: { table: string; name: string; formula: string }[];
    tables: { name: string; formula: string }[];
    functions: { name: string; parameters: string[]; body: string }[];
    variables: { name: string; formula: string }[];
  };
}

export interface DaxQueryTab {
  id: string;
  title: string;
  query: string;
  isDirty?: boolean;
}

export interface DaxQueryHistoryItem {
  id: string;
  timestamp: string;
  query: string;
  success: boolean;
  durationMs: number;
  rowCount: number;
  error?: string;
}

export interface DaxSnippet {
  id: string;
  title: string;
  category: 'Time Intelligence' | 'Ranking' | 'ABC / Pareto' | 'Growth' | 'Security';
  description: string;
  template: string;
}
