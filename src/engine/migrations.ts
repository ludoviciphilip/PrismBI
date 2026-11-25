import { ProjectState } from './storage';
import { Table, Relationship, Measure, WhatIfParameter, RlsRole, CalculationGroup, PowerQueryStep, DataType, TableSourceConfig } from '../types/bi';
import { EtlQuery, EtlStep, EtlType, Cond } from '../types/etl';
import { fileEtlSource, inlineEtlSource } from '../services/etl/etlSource';

export const CURRENT_ENGINE_VERSION = '2.2.0';

function mapDataTypeToEtlType(dt: DataType | undefined): EtlType {
  switch (dt) {
    case 'number':
      return 'decimal';
    case 'percentage':
      return 'decimal';
    case 'currency':
      return 'currency';
    case 'date':
      return 'date';
    case 'boolean':
      return 'boolean';
    case 'text':
    default:
      return 'text';
  }
}

function filterOperatorToCond(step: PowerQueryStep): Cond | null {
  const { column, operator, value } = step.params || {};
  if (!column) return null;
  const map: Record<string, any> = {
    equals: 'eq',
    notEquals: 'neq',
    greaterThan: 'gt',
    lessThan: 'lt',
    contains: 'contains',
  };
  const op = map[operator];
  if (!op) return null;
  return { column, op, value } as Cond;
}


/**
 * Converte um único PowerQueryStep (schema legado, destrutivo) para um EtlStep
 * do novo contrato (docs/ETL_V09.md). Retorna null quando não há equivalente,
 * emitindo console.warn para que o passo seja descartado (e não execute mais).
 */
export function migratePowerQueryStep(step: PowerQueryStep): EtlStep | null {
  const id = step.id;
  const name = step.description || step.type;
  switch (step.type) {
    case 'source':
      return null; // vira EtlSource, não é um EtlStep
    case 'rename':
      return { id, name, op: 'renameColumns', renames: [[step.params.oldName, step.params.newName]] };
    case 'changeType':
      return {
        id,
        name,
        op: 'changeType',
        columns: [{ column: step.params.columnName, type: mapDataTypeToEtlType(step.params.newType) }],
      };
    case 'filter': {
      const cond = filterOperatorToCond(step);
      if (!cond) break;
      return { id, name, op: 'filterRows', condition: cond };
    }
    case 'removeColumn':
    case 'removeColumns': {
      const columns: string[] = step.params.columnNames || (step.params.columnName ? [step.params.columnName] : [step.params.column]);
      if (!columns || !columns.length) break;
      return { id, name, op: 'removeColumns', columns };
    }
    case 'removeOtherColumns': {
      const columns: string[] = step.params.keepColumns || (step.params.column ? [step.params.column] : []);
      if (!columns.length) break;
      return { id, name, op: 'selectColumns', columns };
    }
    case 'removeTopRows':
      return { id, name, op: 'removeTopRows', count: Number(step.params.count) || 1 };
    case 'removeBottomRows':
      return { id, name, op: 'removeBottomRows', count: Number(step.params.count) || 1 };
    case 'keepTopRows':
      return { id, name, op: 'keepTopRows', count: Number(step.params.count) || 10 };
    case 'keepBottomRows':
      return { id, name, op: 'keepBottomRows', count: Number(step.params.count) || 10 };
    case 'removeBlankRows':
      return { id, name, op: 'removeBlankRows' };
    case 'useFirstRowAsHeaders':
      return { id, name, op: 'promoteHeaders' };
    case 'duplicateColumn':
      return {
        id,
        name,
        op: 'duplicateColumn',
        column: step.params.column,
        newColumn: step.params.newColumnName || `${step.params.column} - Cópia`,
      };
    case 'replaceValues':
      return {
        id,
        name,
        op: 'replaceValues',
        columns: [step.params.column],
        find: step.params.oldValue ?? null,
        replace: step.params.newValue ?? null,
        matchEntireCell: step.params.matchExact !== false,
      };
    case 'splitColumn':
      return {
        id,
        name,
        op: 'splitColumn',
        column: step.params.column,
        mode: 'delimiterEach',
        delimiter: step.params.delimiter,
        into: [`${step.params.column}.1`, `${step.params.column}.2`],
      };
    case 'sort':
      return { id, name, op: 'sort', by: [{ column: step.params.column, descending: step.params.order === 'desc' }] };
    case 'deduplicate':
      return { id, name, op: 'removeDuplicates' };
    case 'fillDown':
      return { id, name, op: 'fillDown', columns: [step.params.column] };
    case 'fillUp':
      return { id, name, op: 'fillUp', columns: [step.params.column] };
    case 'addIndexColumn':
      return {
        id,
        name,
        op: 'addIndexColumn',
        newColumn: step.params.newColumnName || 'Índice',
        start: Number(step.params.startIndex) || 0,
        step: 1,
      };
    case 'textTransform': {
      const fnMap: Record<string, 'upper' | 'lower' | 'proper' | 'trim'> = {
        uppercase: 'upper',
        lowercase: 'lower',
        capitalize: 'proper',
        trim: 'trim',
      };
      const fn = fnMap[step.params.mode];
      if (!fn) break;
      return { id, name, op: 'textTransform', columns: [step.params.column], fn };
    }
    case 'addCustomColumn':
      return {
        id,
        name,
        op: 'addCustomColumn',
        newColumn: step.params.newColumnName,
        expression: `each ${step.params.expression}`,
      };
    case 'detectDataTypes':
    case 'useHeadersAsFirstRow':
    case 'textExtract':
    case 'dateExtract':
    case 'conditionalColumn':
    case 'groupBy':
    case 'pivot':
    case 'unpivot':
      break;
    default:
      break;
  }
  console.warn(`[migração ETL] Passo "${step.type}" (${step.id}) não tem equivalente no novo contrato e foi descartado.`);
  return null;
}

/**
 * Migra o mapa legado tableId -> PowerQueryStep[] para tableId -> EtlQuery.
 * Projetos já no novo formato (objetos com `source`/`steps`) passam direto.
 */
export function migrateAppliedSteps(
  raw: Record<string, any> | undefined,
  tables: Table[]
): Record<string, EtlQuery> {
  if (!raw || typeof raw !== 'object') return {};
  const result: Record<string, EtlQuery> = {};
  for (const [tableId, value] of Object.entries(raw)) {
    if (!value) continue;
    // Já no formato novo (EtlQuery)
    if (!Array.isArray(value) && typeof value === 'object' && 'steps' in value && 'source' in value) {
      result[tableId] = value as EtlQuery;
      continue;
    }
    if (!Array.isArray(value)) continue;
    const table = tables.find(t => t.id === tableId);
    const legacySteps = value as PowerQueryStep[];
    const steps: EtlStep[] = [];
    for (const step of legacySteps) {
      const mapped = migratePowerQueryStep(step);
      if (mapped) steps.push(mapped);
    }
    // Os passos legados já foram aplicados (de forma destrutiva) às linhas
    // salvas. Com fonte em arquivo, reexecutá-los a partir do arquivo bruto
    // reproduz o resultado; sem ela, as linhas atuais viram o snapshot e os
    // passos são descartados para não serem aplicados duas vezes.
    const fileSource = fileEtlSource(table);
    if (!fileSource && steps.length) {
      console.warn(`[migração] ${table?.name || tableId}: etapas legadas já aplicadas aos dados; consulta recomeça do estado atual.`);
    }
    result[tableId] = {
      name: table?.originalName || table?.name || tableId,
      source: fileSource ?? inlineEtlSource(table),
      steps: fileSource ? steps : [],
    };
  }
  return result;
}

/**
 * Migrates legacy RLS filter expressions (e.g. "Regiao = 'Sudeste'") to standard DAX syntax ("[Regiao] = \"Sudeste\"").
 */
export function migrateRlsFilterExpression(expr: string): string {
  if (!expr) return '';
  const trimmed = expr.trim();
  // Match unbracketed "Column = 'Value'" or "Column == 'Value'"
  const simpleMatch = trimmed.match(/^([a-zA-Z_][a-zA-Z0-9_]*)\s*(==|=|!=|<>)\s*['"]([^'"]*)['"]$/);
  if (simpleMatch) {
    const col = simpleMatch[1];
    const op = simpleMatch[2] === '==' ? '=' : (simpleMatch[2] === '!=' ? '<>' : simpleMatch[2]);
    const val = simpleMatch[3];
    return `[${col}] ${op} "${val}"`;
  }
  return trimmed;
}

/**
 * Normalizes RLS roles and their filter expressions.
 */
export function migrateRlsRoles(rawRoles: any[]): RlsRole[] {
  if (!Array.isArray(rawRoles)) return [];
  return rawRoles.map(r => ({
    id: r.id || `role_${Math.random().toString(36).substring(2, 9)}`,
    name: r.name || 'Papel RLS',
    rules: Array.isArray(r.rules)
      ? r.rules.map((rule: any) => ({
          tableId: rule.tableId,
          filterExpression: migrateRlsFilterExpression(rule.filterExpression || ''),
        }))
      : [],
    olsRules: Array.isArray(r.olsRules) ? r.olsRules : [],
    members: Array.isArray(r.members) ? r.members : [],
  }));
}

/**
 * Synchronizes WhatIfParameters with backing calculated tables (GENERATESERIES)
 * and measures (SELECTEDVALUE), ensuring existing projects don't break.
 */
export function syncWhatIfWithModel(
  params: WhatIfParameter[],
  tables: Table[],
  measures: Measure[]
): { tables: Table[]; measures: Measure[] } {
  let updatedTables = [...tables];
  let updatedMeasures = [...measures];

  for (const p of params) {
    const tableId = `tbl_whatif_${p.id}`;
    const tableName = p.name;
    const measureName = `${p.name} Valor`;

    // 1. Ensure calculated table exists with GENERATESERIES
    const existingTableIdx = updatedTables.findIndex(
      t => t.id === tableId || t.name.toLowerCase() === tableName.toLowerCase()
    );
    const tableExpr = `GENERATESERIES(${p.min}, ${p.max}, ${p.step})`;

    if (existingTableIdx === -1) {
      updatedTables.push({
        id: tableId,
        name: tableName,
        originalName: tableName,
        isCalculated: true,
        expression: tableExpr,
        columns: [
          {
            id: 'col_Value',
            name: 'Value',
            originalName: 'Value',
            dataType: 'number',
            isCalculated: false,
          },
        ],
        rows: [],
      });
    } else {
      updatedTables[existingTableIdx] = {
        ...updatedTables[existingTableIdx],
        isCalculated: true,
        expression: tableExpr,
      };
    }

    // 2. Ensure parameter measure exists with SELECTEDVALUE
    const existingMsrIdx = updatedMeasures.findIndex(
      m => m.id === `msr_whatif_${p.id}` || m.name.toLowerCase() === measureName.toLowerCase()
    );
    const val = p.currentValue !== undefined ? p.currentValue : p.defaultValue;
    const msrFormula = `SELECTEDVALUE('${tableName}'[Value], ${val})`;

    if (existingMsrIdx === -1) {
      updatedMeasures.push({
        id: `msr_whatif_${p.id}`,
        name: measureName,
        formula: msrFormula,
        tableName,
        format: p.unit === '%' ? 'percentage' : 'number',
        description: `Medida gerada automaticamente para o parâmetro What-If ${p.name}`,
      });
    } else {
      updatedMeasures[existingMsrIdx] = {
        ...updatedMeasures[existingMsrIdx],
        formula: msrFormula,
      };
    }
  }

  return { tables: updatedTables, measures: updatedMeasures };
}

/**
 * Normalizes and migrates any older project state (v1 or legacy) to the v2 schema.
 * Ensures:
 * 1. Relationships fromTable/toTable always point to table IDs instead of names.
 * 2. All required fields are present with correct defaults.
 * 3. Preserves backwards compatibility without dropping user data.
 */
export function migrateProjectState(rawState: any): ProjectState {
  if (!rawState || typeof rawState !== 'object') {
    throw new Error('Estado do projeto inválido para migração');
  }

  const tables: Table[] = Array.isArray(rawState.tables) ? rawState.tables : [];

  // Build name-to-id lookup for table normalization
  const nameToIdMap = new Map<string, string>();
  tables.forEach(t => {
    if (t.name && t.id) {
      nameToIdMap.set(t.name.toLowerCase().trim(), t.id);
    }
  });

  // Migrate relationships: normalize fromTable/toTable to IDs
  const rawRels: any[] = Array.isArray(rawState.relationships) ? rawState.relationships : [];
  const normalizedRelationships: Relationship[] = rawRels.map(r => {
    const fromId = nameToIdMap.get(String(r.fromTable || '').toLowerCase().trim()) || r.fromTable;
    const toId = nameToIdMap.get(String(r.toTable || '').toLowerCase().trim()) || r.toTable;
    return {
      ...r,
      fromTable: fromId,
      toTable: toId,
    };
  });

  // Migrate measures: map legacy format enum to formatString if missing
  const rawMeasures: any[] = Array.isArray(rawState.measures) ? rawState.measures : [];
  const normalizedMeasures = rawMeasures.map(m => {
    let formatString = m.formatString;
    if (!formatString && m.format) {
      switch (m.format) {
        case 'currency':
          formatString = 'Currency';
          break;
        case 'percentage':
          formatString = '0.0%';
          break;
        case 'integer':
          formatString = '#,##0';
          break;
        case 'number':
        default:
          formatString = '#,##0.00';
          break;
      }
    }
    return {
      ...m,
      formatString: formatString || '#,##0.00',
    };
  });

  const rawWhatIf: WhatIfParameter[] = Array.isArray(rawState.whatIfParameters) ? rawState.whatIfParameters : [];
  const synced = syncWhatIfWithModel(rawWhatIf, tables, normalizedMeasures);

  return {
    version: CURRENT_ENGINE_VERSION,
    projectName: rawState.projectName || 'Projeto InsightHub',
    tables: synced.tables,
    relationships: normalizedRelationships,
    measures: synced.measures,
    pages: Array.isArray(rawState.pages) ? rawState.pages : [],
    activePageId: rawState.activePageId || rawState.pages?.[0]?.id || 'page_executivo',
    filters: Array.isArray(rawState.filters) ? rawState.filters : [],
    bookmarks: Array.isArray(rawState.bookmarks) ? rawState.bookmarks : [],
    dashboardTiles: Array.isArray(rawState.dashboardTiles) ? rawState.dashboardTiles : [],
    goals: Array.isArray(rawState.goals) ? rawState.goals : [],
    rlsRoles: migrateRlsRoles(rawState.rlsRoles),
    whatIfParameters: rawWhatIf,
    calculationGroups: Array.isArray(rawState.calculationGroups) ? rawState.calculationGroups : [],
    userDefinedFunctions: Array.isArray(rawState.userDefinedFunctions) ? rawState.userDefinedFunctions : [],
    daxQueries: Array.isArray(rawState.daxQueries) ? rawState.daxQueries : [],
    theme: rawState.theme || 'powerbi-light',
    activeThemeId: rawState.activeThemeId || 'powerbi-classic',
    customThemes: Array.isArray(rawState.customThemes) ? rawState.customThemes : [],
    gridSize: typeof rawState.gridSize === 'number' ? rawState.gridSize : 20,
    showGrid: rawState.showGrid !== undefined ? Boolean(rawState.showGrid) : true,
    snapToGrid: rawState.snapToGrid !== undefined ? Boolean(rawState.snapToGrid) : true,
    appliedSteps: migrateAppliedSteps(rawState.appliedSteps, synced.tables),
    appliedQuerySnapshot: migrateAppliedSteps(rawState.appliedQuerySnapshot, synced.tables),
    updatedAt: rawState.updatedAt || new Date().toISOString(),
  };
}
