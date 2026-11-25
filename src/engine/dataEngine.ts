import { Table, Column, DataType, Relationship, Filter, Measure, Visual } from '../types/bi';
import { DaxEngine, applyRlsRoleToFilterContext } from './daxEngine';
import { ModelAccess } from './model/modelAccess';
import { globalColumnStore } from './model/columnStore';
import { filterContextFromUIFilters } from './model/filterContext';
import { QueryPlanner } from './query/planner';
import { VisualSortSpec } from './visualSort';
import {
  formatValue,
  formatCurrency,
  formatNumber,
  formatPercent,
  formatDateBR,
  getGlobalLocale,
  setGlobalLocale,
  getGlobalCurrency,
  setGlobalCurrency,
  getCurrencySymbol,
} from './shared/formatter';

export {
  formatValue,
  formatCurrency,
  formatNumber,
  formatPercent,
  formatDateBR,
  getGlobalLocale,
  setGlobalLocale,
  getGlobalCurrency,
  setGlobalCurrency,
  getCurrencySymbol,
};

import { Evaluator } from './expr/evaluator';
import { isDaxTable } from './expr/types';

export class DataEngine implements ModelAccess {
  private tablesById: Map<string, Table> = new Map();
  private tableNamesToId: Map<string, string> = new Map();
  private relationships: Relationship[] = [];
  private measuresById: Map<string, Measure> = new Map();
  private measureNamesToId: Map<string, string> = new Map();
  private registeredAlaSqlTables: Set<string> = new Set();
  private schemaVersion = 0;
  private visualQueryCache: Map<string, ReturnType<QueryPlanner['query']>> = new Map();

  constructor() {
    // Inject self into DaxEngine to decouple from global window
    DaxEngine.setModelAccess(this);
  }

  public getModelAccess(): ModelAccess {
    return this;
  }

  public recomputeCalculatedTables(tables: Table[], measures: Measure[]): Table[] {
    const calcTables = tables.filter(t => t.isCalculated && t.expression);
    if (calcTables.length === 0) return tables;

    const resultTables = [...tables];
    for (const ct of calcTables) {
      try {
        const evaluator = new Evaluator({
          dialect: 'dax',
          tables: resultTables,
          measures,
          model: this,
        });
        const res = evaluator.evaluate(ct.expression!);
        if (isDaxTable(res)) {
          const idx = resultTables.findIndex(t => t.id === ct.id);
          if (idx !== -1) {
            const cols: Column[] = res.columns.map(c => {
              const sampleVal = res.rows[0]?.[c];
              let dataType: DataType = 'text';
              if (typeof sampleVal === 'number') dataType = 'number';
              else if (typeof sampleVal === 'boolean') dataType = 'boolean';
              else if (sampleVal instanceof Date) dataType = 'date';
              return {
                id: `col_${c}`,
                name: c,
                originalName: c,
                dataType,
                isCalculated: false,
              };
            });
            resultTables[idx] = {
              ...resultTables[idx],
              columns: cols.length > 0 ? cols : resultTables[idx].columns,
              rows: res.rows,
            };
          }
        }
      } catch (e) {
        console.warn(`Error evaluating calculated table ${ct.name}:`, e);
      }
    }
    return resultTables;
  }

  public recomputeCalculatedColumns(table: Table): Table {
    const calcCols = table.columns.filter(c => c.isCalculated && c.expression);
    if (calcCols.length === 0) return table;

    const updatedRows = table.rows.map(r => ({ ...r }));
    for (const col of calcCols) {
      for (let i = 0; i < updatedRows.length; i++) {
        const row = updatedRows[i];
        try {
          const evaluator = new Evaluator({
            dialect: 'column',
            row,
            rowIndex: i,
            tableId: table.id,
            tableName: table.name,
            tables: this.getTables(),
            measures: this.getMeasures(),
            model: this,
          });
          row[col.name] = evaluator.evaluate(col.expression!);
        } catch {
          row[col.name] = null;
        }
      }
    }

    return { ...table, rows: updatedRows };
  }

  public setSchema(tables: Table[], relationships: Relationship[], measures: Measure[]): void {
    this.tablesById.clear();
    this.tableNamesToId.clear();
    this.measuresById.clear();
    this.measureNamesToId.clear();
    this.registeredAlaSqlTables.clear();
    this.schemaVersion += 1;
    this.visualQueryCache.clear();

    const effectiveTables = this.recomputeCalculatedTables(tables, measures);

    effectiveTables.forEach(t => {
      const computedTable = this.recomputeCalculatedColumns(t);
      this.tablesById.set(computedTable.id, computedTable);
      this.tableNamesToId.set(computedTable.name.toLowerCase().trim(), computedTable.id);
      globalColumnStore.updateTable(computedTable);
    });

    // Normalize relationships: ensure fromTable / toTable use table IDs
    this.relationships = relationships.map(rel => {
      const fromId = this.tableNamesToId.get(rel.fromTable.toLowerCase().trim()) || rel.fromTable;
      const toId = this.tableNamesToId.get(rel.toTable.toLowerCase().trim()) || rel.toTable;
      return {
        ...rel,
        fromTable: fromId,
        toTable: toId,
      };
    });

    measures.forEach(m => {
      this.measuresById.set(m.id, m);
      this.measureNamesToId.set(m.name.toLowerCase().trim(), m.id);
    });

    DaxEngine.setModelAccess(this);
  }

  public getTable(idOrName: string): Table | undefined {
    if (!idOrName) return undefined;
    if (this.tablesById.has(idOrName)) {
      return this.tablesById.get(idOrName);
    }
    const resolvedId = this.tableNamesToId.get(idOrName.toLowerCase().trim());
    return resolvedId ? this.tablesById.get(resolvedId) : undefined;
  }

  public getTables(): Table[] {
    return Array.from(this.tablesById.values());
  }

  public getRelationships(): Relationship[] {
    return this.relationships;
  }

  public getMeasures(): Measure[] {
    return Array.from(this.measuresById.values());
  }

  public getMeasure(idOrName: string): Measure | undefined {
    if (!idOrName) return undefined;
    if (this.measuresById.has(idOrName)) {
      return this.measuresById.get(idOrName);
    }
    const clean = idOrName.replace(/^[Σ\s]+/, '').trim().toLowerCase();
    const resolvedId = this.measureNamesToId.get(clean);
    return resolvedId ? this.measuresById.get(resolvedId) : undefined;
  }

  private ensureTableInAlaSql(_table: Table): void {
    // No-op: SQL queries are handled by DuckDB native in Rust (Tauri) or in-memory fallback
  }

  public executeSql(sql: string): any[] {
    const trimmed = sql.trim().replace(/;+$/, '');
    const m = trimmed.match(
      /^SELECT\s+(.+?)\s+AS\s+Dimensao\s*,\s*(COUNT\(\*\)|(?:SUM|AVG|COUNT|MIN|MAX)\((.+?)\))\s+AS\s+Valor\s+FROM\s+([a-zA-Z0-9_]+)(?:\s+GROUP\s+BY\s+(.+?))?(?:\s+ORDER\s+BY\s+Valor\s+(ASC|DESC))?(?:\s+LIMIT\s+(\d+))?$/i
    );
    if (!m) {
      throw new Error('Use a aba SQL Nativo (DuckDB) para consultas SQL arbitrárias.');
    }
    const groupCol = m[1].trim();
    const funcExpr = m[2].trim().toUpperCase();
    const valCol = (m[3] || '').trim();
    const tableName = m[4].trim();
    const orderDir = (m[6] || 'DESC').toUpperCase();
    const limitN = m[7] ? parseInt(m[7], 10) : 100;

    const table = this.getTable(tableName);
    if (!table) return [];

    const groups = new Map<string, number[]>();
    for (const row of table.rows) {
      const key = String(row[groupCol] ?? 'N/D');
      const arr = groups.get(key) || [];
      if (funcExpr.startsWith('COUNT(*)')) {
        arr.push(1);
      } else {
        const num = Number(row[valCol]);
        if (!isNaN(num)) arr.push(num);
      }
      groups.set(key, arr);
    }

    const out = Array.from(groups.entries()).map(([dim, vals]) => {
      let valor = 0;
      if (funcExpr.startsWith('COUNT')) valor = vals.length;
      else if (funcExpr.startsWith('SUM')) valor = vals.reduce((a, b) => a + b, 0);
      else if (funcExpr.startsWith('AVG')) valor = vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : 0;
      else if (funcExpr.startsWith('MIN')) valor = vals.length ? Math.min(...vals) : 0;
      else if (funcExpr.startsWith('MAX')) valor = vals.length ? Math.max(...vals) : 0;
      return { Dimensao: dim, Valor: valor };
    });

    out.sort((a, b) => (orderDir === 'ASC' ? a.Valor - b.Valor : b.Valor - a.Valor));
    return out.slice(0, limitN);
  }

  /**
   * Applies active filters and cross-table relationship propagation via FilterContext
   */
  public getFilteredRows(targetTableId: string, activeFilters: Filter[]): Record<string, any>[] {
    const targetTable = this.getTable(targetTableId);
    if (!targetTable) return [];

    const filterContext = filterContextFromUIFilters(activeFilters || [], this);
    const activeRole = DaxEngine.getActiveRole();
    if (activeRole) {
      applyRlsRoleToFilterContext(activeRole, filterContext, this);
    } else if (!activeFilters || activeFilters.length === 0) {
      return [...targetTable.rows];
    }

    return filterContext.getFilteredRows(targetTable.id, this);
  }

  public matchesFilter(val: any, operator: Filter['operator'], filterValue: any): boolean {
    if (operator === 'in') {
      const allowed = Array.isArray(filterValue) ? filterValue : [filterValue];
      return allowed.map(String).includes(String(val));
    }
    if (operator === 'notIn') {
      const disallowed = Array.isArray(filterValue) ? filterValue : [filterValue];
      return !disallowed.map(String).includes(String(val));
    }
    if (operator === 'equals') {
      return String(val).toLowerCase() === String(filterValue).toLowerCase();
    }
    if (operator === 'notEquals') {
      return String(val).toLowerCase() !== String(filterValue).toLowerCase();
    }
    if (operator === 'contains') {
      return String(val).toLowerCase().includes(String(filterValue).toLowerCase());
    }
    const numVal = Number(val);
    const numFilter = Number(filterValue);
    if (operator === 'greaterThan') return numVal > numFilter;
    if (operator === 'greaterThanOrEqual') return numVal >= numFilter;
    if (operator === 'lessThan') return numVal < numFilter;
    if (operator === 'lessThanOrEqual') return numVal <= numFilter;
    if (operator === 'between' && Array.isArray(filterValue)) {
      return numVal >= Number(filterValue[0]) && numVal <= Number(filterValue[1]);
    }
    return true;
  }

  /**
   * Queries aggregated visual data based on visual config and active filters via QueryPlanner
   */
  public queryVisualData(
    visual: Visual,
    activeFilters: Filter[] = []
  ): {
    categories: string[];
    series: { name: string; data: number[] }[];
    rawRows: Record<string, any>[];
    summaryValue?: number;
    matrixHierarchy?: any[];
    subtotals?: Record<string, number>;
    grandTotal?: number;
    queryTimeMs?: number;
  } {
    const roleId = DaxEngine.getActiveRole()?.id ?? '';
    const cacheKey = `${this.schemaVersion}|${roleId}|${visual.id}|${visual.type}|${JSON.stringify(visual.dataConfig)}|${JSON.stringify(visual.visualCalculations ?? null)}|${JSON.stringify(activeFilters)}`;
    const cached = this.visualQueryCache.get(cacheKey);
    if (cached) {
      return cached;
    }

    const planner = new QueryPlanner(this);
    const result = planner.query(visual, activeFilters);
    if (this.visualQueryCache.size >= 200) {
      this.visualQueryCache.clear();
    }
    this.visualQueryCache.set(cacheKey, result);
    return result;
  }

  /**
   * Sort spec (sortField/sortOrder/limit + sortByColumn lookup) for a visual,
   * used to sort native (Rust) query results with the shared visualSort rules.
   */
  public getVisualSortSpec(visual: Visual): VisualSortSpec | null {
    return new QueryPlanner(this).buildSortSpec(visual);
  }

  /**
   * Batch evaluates multiple visuals with shared schema and planner
   */
  public batchEvaluateVisuals(
    visuals: Visual[],
    activeFilters: Filter[] = []
  ): Map<string, ReturnType<DataEngine['queryVisualData']>> {
    const results = new Map<string, ReturnType<DataEngine['queryVisualData']>>();
    const planner = new QueryPlanner(this);
    for (const visual of visuals) {
      results.set(visual.id, planner.query(visual, activeFilters));
    }
    return results;
  }
}

export const globalDataEngine = new DataEngine();
