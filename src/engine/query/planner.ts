import { Visual, Filter, Table, Measure } from '../../types/bi';
import { ModelAccess } from '../model/modelAccess';
import { FilterContext, filterContextFromUIFilters } from '../model/filterContext';
import { RelationshipGraph } from '../model/relationships';
import { DaxEngine } from '../daxEngine';
import { parseNumber } from '../shared/valueParser';
import { isBlank, DAX_BLANK } from '../expr/types';
import { Evaluator } from '../expr/evaluator';
import {
  applyVisualSort,
  buildSortByLookup,
  fieldMatches,
  sortCategoryLabels,
  sortRows,
  VisualSortSpec,
} from '../visualSort';

export interface VisualQueryResult {
  categories: string[];
  series: { name: string; data: (number | null)[] }[];
  rawRows: Record<string, any>[];
  summaryValue?: number;
  matrixHierarchy?: any[];
  subtotals?: Record<string, number>;
  grandTotal?: number;
  queryTimeMs?: number;
}

export class QueryPlanner {
  private model: ModelAccess;

  constructor(model: ModelAccess) {
    this.model = model;
  }

  /**
   * Main entry point for visual data queries in InsightHub V0.3
   */
  public query(visual: Visual, activeFilters: Filter[] = []): VisualQueryResult {
    const startTime = typeof performance !== 'undefined' ? performance.now() : Date.now();

    const tableId = visual.dataConfig.tableId;
    const table = this.model.getTable(tableId);

    if (!table) {
      return { categories: [], series: [], rawRows: [] };
    }

    // Convert UI filters to structured FilterContext
    const baseFilterContext = filterContextFromUIFilters(activeFilters, this.model, visual.id);
    const visibleRowIndices = baseFilterContext.getVisibleRowIndices(table.id, this.model);
    const filteredRows = visibleRowIndices.map(i => table.rows[i]);

    // 1. Single Card, KPI, or Gauge Visual
    if (visual.type === 'card' || visual.type === 'kpi' || visual.type === 'gauge') {
      const yField = visual.dataConfig.yAxisFields?.[0] || table.columns.find(c => c.dataType === 'number' || c.dataType === 'currency')?.name || '';
      const agg = visual.dataConfig.aggregation || 'SUM';
      const measure = this.model.getMeasure ? this.model.getMeasure(yField) : undefined;

      let summary: number | null = null;

      if (measure) {
        const val = DaxEngine.evaluateWithContext(measure.formula, this.model, baseFilterContext);
        summary = typeof val === 'number' && !isNaN(val) ? val : (isBlank(val) ? 0 : Number(val) || 0);
      } else if (yField) {
        summary = this.calculateImplicitAggregation(filteredRows, yField, agg);
      }

      const totalTime = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - startTime;

      return {
        categories: [visual.title || yField],
        series: [{ name: yField, data: [summary !== null ? summary : 0] }],
        rawRows: filteredRows,
        summaryValue: summary !== null ? summary : undefined,
        grandTotal: summary !== null ? summary : undefined,
        queryTimeMs: Number(totalTime.toFixed(1)),
      };
    }

    // 1b. Multi-Card or Measure-Only Chart (no xAxisField, multiple measures/fields)
    const yFieldsForMulti = visual.dataConfig.yAxisFields || [];
    if (
      visual.type === 'multi-card' ||
      (!visual.dataConfig.xAxisField && yFieldsForMulti.length > 0 && visual.type !== 'slicer' && visual.type !== 'table')
    ) {
      const agg = visual.dataConfig.aggregation || 'SUM';
      const scalarCategories: string[] = [];
      const scalarData: number[] = [];

      for (const yField of yFieldsForMulti) {
        const measure = this.model.getMeasure ? this.model.getMeasure(yField) : undefined;
        let valNum = 0;
        if (measure) {
          const val = DaxEngine.evaluateWithContext(measure.formula, this.model, baseFilterContext);
          valNum = typeof val === 'number' && !isNaN(val) ? val : (isBlank(val) ? 0 : Number(val) || 0);
          scalarCategories.push(measure.name);
        } else {
          valNum = this.calculateImplicitAggregation(filteredRows, yField, agg);
          scalarCategories.push(yField);
        }
        scalarData.push(valNum);
      }

      const totalTime = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - startTime;
      return {
        categories: scalarCategories,
        series: [{ name: visual.title || 'Valores', data: scalarData }],
        rawRows: filteredRows,
        summaryValue: scalarData[0],
        grandTotal: scalarData.reduce((a, b) => a + b, 0),
        queryTimeMs: Number(totalTime.toFixed(1)),
      };
    }

    // 2. Slicer Visual (exclude the slicer's own column filter so all options stay selectable)
    if (visual.type === 'slicer') {
      const rawSlicerCol = visual.dataConfig.xAxisField || table.columns[0]?.name || '';
      const { categoryTable: slicerTable, cleanColName: targetCol } = this.resolveColumnLocation(table, rawSlicerCol);
      const slicerFilterContext = filterContextFromUIFilters(
        (activeFilters || []).filter(f => {
          const sameTable =
            f.tableId === slicerTable.id ||
            f.tableId === slicerTable.name ||
            f.tableId === table.id ||
            f.tableId === table.name;
          const sameCol = f.column.toLowerCase() === targetCol.toLowerCase() || f.column.toLowerCase() === rawSlicerCol.toLowerCase();
          return !(sameTable && sameCol);
        }),
        this.model,
        visual.id
      );
      const slicerRows = slicerFilterContext.getFilteredRows(slicerTable.id, this.model);
      const distinctSet = new Set<string>();

      slicerRows.forEach(r => {
        const val = r[targetCol];
        if (!isBlank(val) && val !== null && val !== undefined && val !== '') {
          distinctSet.add(String(val));
        }
      });

      const slicerColMeta = slicerTable.columns.find(c => c.name.toLowerCase() === targetCol.toLowerCase());
      const slicerSortKeys = slicerColMeta?.sortByColumn
        ? buildSortByLookup(slicerTable.rows, targetCol, slicerColMeta.sortByColumn)
        : undefined;
      const categories = sortCategoryLabels(
        Array.from(distinctSet),
        visual.dataConfig.sortOrder === 'desc' ? 'desc' : 'asc',
        slicerSortKeys
      );
      const totalTime = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - startTime;

      return {
        categories,
        series: [{ name: targetCol, data: categories.map(() => 1) }],
        rawRows: slicerRows,
        queryTimeMs: Number(totalTime.toFixed(1)),
      };
    }

    // 3. Multi-Field / Standard Charts (Column, Bar, Line, Area, Pie, Donut, Treemap, Funnel, Matrix, Table)
    // Multi-campo drill level resolution
    const effectiveXField = this.resolveEffectiveXField(visual, table);

    // Resolve column & table for X axis
    const { categoryTable, cleanColName } = this.resolveColumnLocation(table, effectiveXField);

    // Grouping & Categories extraction in batch
    const categories = this.extractCategories(table, categoryTable, cleanColName, baseFilterContext, filteredRows);

    // Check if legendField is provided for series breakdown
    const legendField = visual.dataConfig.legendField;
    let series: { name: string; data: (number | null)[] }[] = [];

    const yFields = visual.dataConfig.yAxisFields && visual.dataConfig.yAxisFields.length > 0
      ? visual.dataConfig.yAxisFields
      : [table.columns.find(c => c.dataType === 'number' || c.dataType === 'currency')?.name || 'ValorTotal'];

    const evalMeasureOpt = (formula: string, sliceCtx: FilterContext): number | null => {
      const val = DaxEngine.evaluateWithContext(formula, this.model, sliceCtx);
      if (isBlank(val) || val === null || val === undefined || val === '') return null;
      const n = typeof val === 'number' ? val : Number(val);
      return Number.isFinite(n) ? n : null;
    };

    if (legendField) {
      // Legend breakdown: One series per distinct value of legendField
      const { categoryTable: legendTable, cleanColName: legendCol } = this.resolveColumnLocation(table, legendField);
      const rawLegendValues = this.extractCategories(table, legendTable, legendCol, baseFilterContext, filteredRows);
      const legendColMeta = legendTable.columns.find(c => c.name.toLowerCase() === legendCol.toLowerCase());
      const legendSortKeys = legendColMeta?.sortByColumn
        ? buildSortByLookup(legendTable.rows, legendCol, legendColMeta.sortByColumn)
        : undefined;
      const legendValues = sortCategoryLabels(rawLegendValues, 'asc', legendSortKeys);
      const targetMetric = yFields[0];
      const measureObj = this.model.getMeasure ? this.model.getMeasure(targetMetric) : undefined;
      const isMeasure = !!measureObj;
      const agg = visual.dataConfig.aggregation || (isMeasure ? 'NONE' : 'SUM');

      series = legendValues.map(legVal => {
        const data = categories.map(cat => {
          const sliceContext = baseFilterContext.clone();
          sliceContext.setColumnFilter(categoryTable.id, cleanColName, {
            tableId: categoryTable.id,
            column: cleanColName,
            allowedValues: new Set([cat]),
          });
          sliceContext.setColumnFilter(legendTable.id, legendCol, {
            tableId: legendTable.id,
            column: legendCol,
            allowedValues: new Set([legVal]),
          });

          if (isMeasure && measureObj) {
            return evalMeasureOpt(measureObj.formula, sliceContext);
          } else {
            const sliceRows = sliceContext.getFilteredRows(table.id, this.model);
            return this.calculateImplicitAggregationOpt(sliceRows, targetMetric, agg);
          }
        });

        return {
          name: legVal,
          data,
        };
      });
    } else {
      // Standard metrics: One series per yAxisField
      series = yFields.map(yField => {
        const measureObj = this.model.getMeasure ? this.model.getMeasure(yField) : undefined;
        const isMeasure = !!measureObj;
        const agg = visual.dataConfig.aggregation || (isMeasure ? 'NONE' : 'SUM');

        const data = categories.map(cat => {
          const sliceContext = baseFilterContext.clone();
          sliceContext.setColumnFilter(categoryTable.id, cleanColName, {
            tableId: categoryTable.id,
            column: cleanColName,
            allowedValues: new Set([cat]),
          });

          if (isMeasure && measureObj) {
            return evalMeasureOpt(measureObj.formula, sliceContext);
          } else {
            const sliceRows = sliceContext.getFilteredRows(table.id, this.model);
            return this.calculateImplicitAggregationOpt(sliceRows, yField, agg);
          }
        });

        return {
          name: isMeasure ? measureObj!.name : yField,
          data,
        };
      });
    }

    // Power BI drops rows where every measure/column evaluates to BLANK, and drops
    // legend columns that have no non-blank cells on any kept row.
    const keptRowIndices = categories
      .map((_, rIdx) => rIdx)
      .filter(rIdx => series.some(s => s.data[rIdx] !== null));
    const nonEmptyCategories = keptRowIndices.map(rIdx => categories[rIdx]);
    let nonEmptySeries = series.map(s => ({
      ...s,
      data: keptRowIndices.map(rIdx => s.data[rIdx]),
    }));
    if (legendField) {
      nonEmptySeries = nonEmptySeries.filter(s => s.data.some(v => v !== null));
    }

    // Sorting (shared with the native Rust path) — TopN/limit is applied AFTER sorting.
    const sorted = applyVisualSort(
      { categories: nonEmptyCategories, series: nonEmptySeries },
      this.buildSortSpecFor(visual, table, effectiveXField, categoryTable, cleanColName, yFields)
    );
    const finalCategories = sorted.categories;
    const finalSeries = sorted.series;

    // Apply Visual Calculations (V0.6)
    const visualRows = this.applyVisualCalculations(
      visual,
      finalCategories,
      finalSeries,
      effectiveXField,
      baseFilterContext
    );

    // Calculate Grand Total using own context without category filters
    let grandTotal: number | undefined;
    if (yFields.length > 0) {
      const primaryMetric = yFields[0];
      const measureObj = this.model.getMeasure ? this.model.getMeasure(primaryMetric) : undefined;
      if (measureObj) {
        const val = DaxEngine.evaluateWithContext(measureObj.formula, this.model, baseFilterContext);
        grandTotal = typeof val === 'number' && !isNaN(val) ? val : undefined;
      } else {
        grandTotal = this.calculateImplicitAggregation(filteredRows, primaryMetric, visual.dataConfig.aggregation || 'SUM');
      }
    }

    const totalTime = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - startTime;

    const hasVisualCalcs = (visual.dataConfig?.visualCalculations?.length ?? 0) > 0 || (visual.visualCalculations?.length ?? 0) > 0;

    let detailRows = (hasVisualCalcs && visualRows.length > 0) ? visualRows : filteredRows;
    if (visual.type === 'table' && visual.dataConfig.sortField) {
      const sortCol = table.columns.find(c => fieldMatches(c.name, visual.dataConfig.sortField));
      detailRows = sortRows(detailRows, visual.dataConfig.sortField, visual.dataConfig.sortOrder === 'desc' ? 'desc' : 'asc', {
        sortByColumn: sortCol?.sortByColumn,
      });
    }

    return {
      categories: finalCategories,
      series: finalSeries,
      rawRows: detailRows,
      grandTotal,
      queryTimeMs: Number(totalTime.toFixed(1)),
    };
  }

  private resolveEffectiveXField(visual: Visual, table: Table): string {
    let effectiveXField = visual.dataConfig.xAxisField || table.columns[0]?.name || 'Categoria';
    if (
      visual.dataConfig.drillFields &&
      visual.dataConfig.drillFields.length > 0 &&
      visual.dataConfig.currentDrillLevel !== undefined &&
      visual.dataConfig.currentDrillLevel > 0
    ) {
      const drillIdx = visual.dataConfig.currentDrillLevel - 1;
      if (drillIdx < visual.dataConfig.drillFields.length) {
        effectiveXField = visual.dataConfig.drillFields[drillIdx];
      }
    }
    return effectiveXField;
  }

  private buildSortSpecFor(
    visual: Visual,
    table: Table,
    effectiveXField: string,
    categoryTable: Table,
    cleanColName: string,
    yFields: string[]
  ): VisualSortSpec {
    const xColMeta = categoryTable.columns.find(c => c.name.toLowerCase() === cleanColName.toLowerCase());
    const categorySortKeys = xColMeta?.sortByColumn
      ? buildSortByLookup(categoryTable.rows, cleanColName, xColMeta.sortByColumn)
      : undefined;
    let legendSortKeys: Map<string, unknown> | undefined;
    if (visual.dataConfig.legendField) {
      const { categoryTable: legendTable, cleanColName: legendCol } = this.resolveColumnLocation(table, visual.dataConfig.legendField);
      const legendColMeta = legendTable.columns.find(c => c.name.toLowerCase() === legendCol.toLowerCase());
      if (legendColMeta?.sortByColumn) {
        legendSortKeys = buildSortByLookup(legendTable.rows, legendCol, legendColMeta.sortByColumn);
      }
    }
    const valueFields = [...yFields];
    for (const y of yFields) {
      const m = this.model.getMeasure ? this.model.getMeasure(y) : undefined;
      if (m) valueFields.push(m.name);
    }
    return {
      sortField: visual.dataConfig.sortField,
      sortOrder: visual.dataConfig.sortOrder,
      limit: visual.dataConfig.limit,
      categoryFields: [
        visual.dataConfig.xAxisField || '',
        effectiveXField,
        cleanColName,
        `${categoryTable.name}.${cleanColName}`,
        `${table.name}.${cleanColName}`,
      ].filter(Boolean),
      valueFields,
      categorySortKeys,
      legendSortKeys,
    };
  }

  /**
   * Sort spec for a category/series visual, so results coming from another
   * engine (the Rust native query) can be sorted with the exact same rules.
   */
  public buildSortSpec(visual: Visual): VisualSortSpec | null {
    const table = this.model.getTable(visual.dataConfig.tableId);
    if (!table) return null;
    const effectiveXField = this.resolveEffectiveXField(visual, table);
    const { categoryTable, cleanColName } = this.resolveColumnLocation(table, effectiveXField);
    const yFields = visual.dataConfig.yAxisFields && visual.dataConfig.yAxisFields.length > 0
      ? visual.dataConfig.yAxisFields
      : [table.columns.find(c => c.dataType === 'number' || c.dataType === 'currency')?.name || 'ValorTotal'];
    return this.buildSortSpecFor(visual, table, effectiveXField, categoryTable, cleanColName, yFields);
  }

  /**
   * Applies Visual Calculations (Cálculos Visuais) over visual rows
   */
  private applyVisualCalculations(
    visual: Visual,
    categories: string[],
    series: { name: string; data: (number | null)[] }[],
    effectiveXField: string,
    filterContext: FilterContext
  ): Record<string, any>[] {
    const calcs = visual.dataConfig?.visualCalculations || visual.visualCalculations;
    if (!calcs || calcs.length === 0 || categories.length === 0) return [];

    // Synthesize visual rows representing the visual's current layout
    const visualRows: Record<string, any>[] = categories.map((cat, idx) => {
      const row: Record<string, any> = {
        [effectiveXField]: cat,
        Category: cat,
      };
      for (const s of series) {
        row[s.name] = s.data[idx];
      }
      return row;
    });

    for (const vc of calcs) {
      if (!vc.name || !vc.formula) continue;
      const calcData: number[] = [];

      for (let rIdx = 0; rIdx < visualRows.length; rIdx++) {
        try {
          const evalContext = {
            model: this.model,
            filterContext,
            isVisualCalculation: true,
            visualRows,
            visualRowIndex: rIdx,
            row: visualRows[rIdx],
            rowIndex: rIdx,
            tables: this.model.getTables(),
            measures: this.model.getMeasures ? this.model.getMeasures() : [],
          };
          const evaluator = new Evaluator(evalContext);
          const rawResult = evaluator.evaluate(vc.formula);
          const num = typeof rawResult === 'number' && !isNaN(rawResult)
            ? rawResult
            : (isBlank(rawResult) ? 0 : Number(rawResult) || 0);

          calcData.push(num);
          visualRows[rIdx][vc.name] = rawResult;
        } catch (err) {
          console.warn(`[VisualCalculation] Erro ao calcular "${vc.name}":`, err);
          calcData.push(0);
          visualRows[rIdx][vc.name] = 0;
        }
      }

      series.push({
        name: vc.name,
        data: calcData,
      });
    }

    return visualRows;
  }

  private resolveColumnLocation(
    baseTable: Table,
    rawColName: string
  ): { categoryTable: Table; cleanColName: string } {
    let categoryTable = baseTable;
    let cleanColName = rawColName;

    if (rawColName.includes('.')) {
      const [tPrefix, cName] = rawColName.split('.', 2);
      const foundTable = this.model.getTable(tPrefix);
      if (foundTable) {
        categoryTable = foundTable;
        cleanColName = cName;
      }
    } else if (!baseTable.columns.some(c => c.name.toLowerCase() === rawColName.toLowerCase())) {
      // Find in related tables
      for (const t of this.model.getTables()) {
        if (t.id !== baseTable.id && t.columns.some(c => c.name.toLowerCase() === rawColName.toLowerCase())) {
          categoryTable = t;
          cleanColName = t.columns.find(c => c.name.toLowerCase() === rawColName.toLowerCase())!.name;
          break;
        }
      }
    }

    return { categoryTable, cleanColName };
  }

  private extractCategories(
    baseTable: Table,
    categoryTable: Table,
    colName: string,
    filterContext: FilterContext,
    baseFilteredRows: Record<string, any>[]
  ): string[] {
    const categoriesSet = new Set<string>();

    if (categoryTable.id === baseTable.id) {
      baseFilteredRows.forEach(r => {
        const val = r[colName];
        if (!isBlank(val) && val !== null && val !== undefined && val !== '') {
          categoriesSet.add(String(val));
        }
      });
    } else {
      // Related table: resolve active relationship from baseTable to categoryTable
      const graph = new RelationshipGraph(this.model);
      const path = graph.findActivePath(baseTable.id, categoryTable.id);
      if (path && path.length > 0 && baseFilteredRows.length > 0) {
        const activeKeys = new Set<string>();
        baseFilteredRows.forEach(r => {
          const k = String(r[path[0].fromColumn] ?? '').trim();
          if (k) activeKeys.add(k);
        });

        const catRows = filterContext.getFilteredRows(categoryTable.id, this.model);
        const toCol = path[path.length - 1].toColumn;
        catRows.forEach(cr => {
          const joinVal = String(cr[toCol] ?? '').trim();
          if (activeKeys.has(joinVal)) {
            const val = cr[colName];
            if (!isBlank(val) && val !== null && val !== undefined && val !== '') {
              categoriesSet.add(String(val));
            }
          }
        });
      } else {
        const catRows = filterContext.getFilteredRows(categoryTable.id, this.model);
        catRows.forEach(cr => {
          const val = cr[colName];
          if (!isBlank(val) && val !== null && val !== undefined && val !== '') {
            categoriesSet.add(String(val));
          }
        });
      }
    }

    return Array.from(categoriesSet);
  }

  private calculateImplicitAggregationOpt(
    rows: Record<string, any>[],
    columnName: string,
    agg: Visual['dataConfig']['aggregation']
  ): number | null {
    if (rows.length === 0) return null;

    const values = rows.map(r => r[columnName]);
    const nonBlankValues = values.filter(v => !isBlank(v) && v !== null && v !== undefined && v !== '');
    if (nonBlankValues.length === 0) return null;

    switch (agg) {
      case 'COUNT':
        return nonBlankValues.length;
      case 'DISTINCTCOUNT':
        return new Set(nonBlankValues).size;
      default: {
        const numValues = nonBlankValues
          .map(v => parseNumber(v).value)
          .filter((n): n is number => typeof n === 'number' && Number.isFinite(n));
        if (numValues.length === 0) return null;
        if (agg === 'AVG') return numValues.reduce((a, b) => a + b, 0) / numValues.length;
        if (agg === 'MIN') return Math.min(...numValues);
        if (agg === 'MAX') return Math.max(...numValues);
        if (agg === 'NONE') return numValues[0];
        return numValues.reduce((a, b) => a + b, 0);
      }
    }
  }

  private calculateImplicitAggregation(
    rows: Record<string, any>[],
    columnName: string,
    agg: Visual['dataConfig']['aggregation']
  ): number {
    return this.calculateImplicitAggregationOpt(rows, columnName, agg) ?? 0;
  }
}
