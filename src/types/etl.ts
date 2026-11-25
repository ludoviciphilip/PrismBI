// Contrato EtlQuery/EtlStep — espelha docs/ETL_V09.md e src-tauri/src/etl/model.rs.
// Qualquer mudança de schema deve atualizar os dois lados no mesmo PR.

export type EtlType = 'text' | 'integer' | 'decimal' | 'currency' | 'date' | 'datetime' | 'boolean';

export type Scalar = string | number | boolean | null;

export type Cond =
  | { column: string; op: 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte' | 'contains' | 'notContains' | 'startsWith' | 'endsWith'; value: Scalar }
  | { column: string; op: 'isNull' | 'isNotNull' | 'isBlank' | 'isNotBlank' }
  | { column: string; op: 'in' | 'notIn'; values: Scalar[] }
  | { and: Cond[] }
  | { or: Cond[] }
  | { not: Cond };

export type AggOp = 'sum' | 'avg' | 'min' | 'max' | 'count' | 'countDistinct' | 'countRows' | 'median';

export interface Agg {
  name: string;
  op: AggOp;
  column?: string;
}

interface EtlStepBase {
  id: string;
  /** nome do passo no M, ex: #"Changed Type" */
  name: string;
}

export type EtlStep = EtlStepBase &
  (
    | { op: 'promoteHeaders' }
    | { op: 'changeType'; columns: { column: string; type: EtlType }[]; culture?: string }
    | { op: 'renameColumns'; renames: [string, string][] }
    | { op: 'removeColumns'; columns: string[] }
    | { op: 'selectColumns'; columns: string[] }
    | { op: 'duplicateColumn'; column: string; newColumn: string }
    | { op: 'filterRows'; condition: Cond }
    | { op: 'removeBlankRows' }
    | { op: 'removeRowsWithErrors'; columns?: string[] }
    | { op: 'keepTopRows' | 'removeTopRows' | 'keepBottomRows' | 'removeBottomRows'; count: number }
    | { op: 'removeDuplicates'; columns?: string[] }
    | { op: 'sort'; by: { column: string; descending: boolean }[] }
    | { op: 'replaceValues'; columns: string[]; find: Scalar; replace: Scalar; matchEntireCell: boolean }
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
    | { op: 'pivot'; column: string; valuesColumn: string; aggregation: AggOp }
    | { op: 'groupBy'; keys: string[]; aggregations: Agg[] }
    | {
        op: 'addConditionalColumn';
        newColumn: string;
        rules: { when: Cond; then: Scalar | { column: string } }[];
        otherwise: Scalar | { column: string };
        type?: EtlType;
      }
    | { op: 'addCustomColumn'; newColumn: string; expression: string; type?: EtlType }
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
    | { op: 'transformColumns'; columns: string[]; expression: string; type?: EtlType }
    | { op: 'reorderColumns'; columns: string[] }
    | { op: 'demoteHeaders' }
    | { op: 'transpose' }
    | { op: 'reverseRows' }
    | { op: 'keepRange'; offset: number; count: number }
    | { op: 'removeAlternateRows'; offset: number; remove: number; keep: number }
    | { op: 'keepDuplicates'; columns?: string[] }
    | { op: 'replaceErrors'; replacements: { column: string; value: Scalar }[] }
    | { op: 'keepRowsWithErrors'; columns?: string[] }
  );

export type EtlStepOp = EtlStep['op'];

/** Omit distributivo: preserva a união discriminada ao remover um campo comum. */
export type DistributiveOmit<T, K extends keyof any> = T extends any ? Omit<T, K> : never;

export interface EtlSource {
  kind: 'excel' | 'csv' | 'parquet' | 'json' | 'db' | 'table' | 'inline' | 'listDates' | 'opaqueM';
  path?: string;
  sheet?: string;
  /** 'table': outra consulta do modelo; 'db': relação DuckDB (ex. "pg.public.pedidos"). */
  table?: string;
  m?: string;
  /** 'inline': literal #table({..}, {{..}}) ou snapshot de uma tabela sem fonte recarregável. */
  columns?: string[];
  rows?: Scalar[][];
  /** 'listDates': List.Dates(#date, count, #duration(stepDays, 0, 0, 0)). */
  start?: string;
  count?: number;
  stepDays?: number;
  function?: string;
}

export interface EtlQuery {
  name: string;
  source: EtlSource;
  steps: EtlStep[];
}

export interface EtlColumnSchema {
  name: string;
  type: string;
}

export interface EtlLoadRawResult {
  rawTable: string;
  columns: EtlColumnSchema[];
  rowCount: number;
}

export interface EtlFailedStep {
  index: number;
  message: string;
}

export interface EtlPreviewResult {
  columns: EtlColumnSchema[];
  rows: Record<string, any>[];
  rowCount: number;
  warnings: string[];
  failedStep?: EtlFailedStep;
}

export interface EtlFromMResult {
  query: EtlQuery;
  warnings: string[];
}

// Payload de tabela importada, reutilizado por etl_apply (mesmo formato dos importadores atuais).
export interface ImportedTablePayload {
  tableName: string;
  originalName: string;
  columns: { name: string; originalName: string; dataType: string; duckdbType?: string }[];
  rows: Record<string, any>[];
  totalRows: number;
  truncated: boolean;
  sourcePath?: string | null;
  sheetName?: string | null;
}
