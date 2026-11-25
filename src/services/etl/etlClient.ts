// Adaptador para os 5 comandos Tauri do ETL (docs/ETL_V09.md).
// Fora do Tauri (navegador), cai para uma implementação local limitada,
// construída sobre o motor legado, com aviso de recursos reduzidos.
import { invoke } from '@tauri-apps/api/core';
import { isTauriRuntime } from '../../bridge/tauriBridge';
import {
  Agg,
  Cond,
  EtlColumnSchema,
  EtlFromMResult,
  EtlLoadRawResult,
  EtlPreviewResult,
  EtlQuery,
  EtlSource,
  EtlStep,
  EtlType,
  ImportedTablePayload,
  Scalar,
} from '../../types/etl';

export const ETL_BROWSER_FALLBACK_NOTICE =
  'Executando sem o Tauri: usando o motor ETL limitado do navegador. Alguns passos (mesclagens entre consultas, M complexo) podem ter suporte parcial.';

export interface BrowserRawTable {
  columns: EtlColumnSchema[];
  rows: Record<string, any>[];
}

export type BrowserRawProvider = (name: string) => BrowserRawTable | undefined;

let rawProvider: BrowserRawProvider | null = null;

/** Usado pela store para expor as tabelas atuais do modelo ao fallback de navegador. */
export function setBrowserRawProvider(fn: BrowserRawProvider | null): void {
  rawProvider = fn;
}

export async function etlLoadRaw(args: { query: string; source: EtlSource; pbipRoot?: string }): Promise<EtlLoadRawResult> {
  if (isTauriRuntime()) return invoke('etl_load_raw', args);
  return browserLoadRaw(args.query);
}

export async function etlPreview(args: { query: EtlQuery; uptoStep?: number; limit?: number }): Promise<EtlPreviewResult> {
  if (isTauriRuntime()) return invoke('etl_preview', args);
  return browserPreview(args.query, args.uptoStep, args.limit ?? 200);
}

export async function etlApply(args: { query: EtlQuery; targetTable: string }): Promise<ImportedTablePayload> {
  if (isTauriRuntime()) return invoke('etl_apply', args);
  return browserApply(args.query, args.targetTable);
}

export async function etlToM(args: { query: EtlQuery }): Promise<string> {
  if (isTauriRuntime()) return invoke('etl_to_m', args);
  return browserToM(args.query);
}

export async function etlFromM(args: { name: string; m: string }): Promise<EtlFromMResult> {
  if (isTauriRuntime()) return invoke('etl_from_m', args);
  return browserFromM(args.name, args.m);
}

// ---------------------------------------------------------------------------
// Fallback de navegador
// ---------------------------------------------------------------------------

function browserLoadRaw(queryName: string): EtlLoadRawResult {
  const raw = rawProvider?.(queryName);
  return {
    rawTable: `raw__${queryName}`,
    columns: raw?.columns ?? [],
    rowCount: raw?.rows.length ?? 0,
  };
}

function scalarsEqual(a: any, b: Scalar): boolean {
  if (a === null || a === undefined) return b === null;
  // eslint-disable-next-line eqeqeq
  return a == b;
}

function isBlankValue(v: any): boolean {
  return v === null || v === undefined || String(v).trim() === '';
}

export function evalCond(cond: Cond, row: Record<string, any>): boolean {
  if ('and' in cond) return cond.and.every(c => evalCond(c, row));
  if ('or' in cond) return cond.or.some(c => evalCond(c, row));
  if ('not' in cond) return !evalCond(cond.not, row);
  const val = row[cond.column];
  switch (cond.op) {
    case 'eq':
      return scalarsEqual(val, cond.value);
    case 'neq':
      return !scalarsEqual(val, cond.value);
    case 'gt':
      return Number(val) > Number(cond.value);
    case 'gte':
      return Number(val) >= Number(cond.value);
    case 'lt':
      return Number(val) < Number(cond.value);
    case 'lte':
      return Number(val) <= Number(cond.value);
    case 'contains':
      return String(val ?? '').toLowerCase().includes(String(cond.value ?? '').toLowerCase());
    case 'notContains':
      return !String(val ?? '').toLowerCase().includes(String(cond.value ?? '').toLowerCase());
    case 'startsWith':
      return String(val ?? '').toLowerCase().startsWith(String(cond.value ?? '').toLowerCase());
    case 'endsWith':
      return String(val ?? '').toLowerCase().endsWith(String(cond.value ?? '').toLowerCase());
    case 'isNull':
      return val === null || val === undefined;
    case 'isNotNull':
      return !(val === null || val === undefined);
    case 'isBlank':
      return isBlankValue(val);
    case 'isNotBlank':
      return !isBlankValue(val);
    case 'in':
      return cond.values.some(v => scalarsEqual(val, v));
    case 'notIn':
      return !cond.values.some(v => scalarsEqual(val, v));
    default:
      return true;
  }
}

function convertValue(val: any, type: EtlType): any {
  if (val === null || val === undefined || val === '') return null;
  switch (type) {
    case 'integer':
      return Number.isNaN(Number(val)) ? null : Math.trunc(Number(val));
    case 'decimal':
    case 'currency': {
      const n = Number(String(val).replace(/[^0-9.,-]/g, '').replace(',', '.'));
      return Number.isNaN(n) ? null : n;
    }
    case 'boolean': {
      if (typeof val === 'boolean') return val;
      const s = String(val).trim().toLowerCase();
      if (['true', '1', 'sim', 'verdadeiro'].includes(s)) return true;
      if (['false', '0', 'não', 'nao', 'falso'].includes(s)) return false;
      return null;
    }
    case 'date':
    case 'datetime': {
      const d = new Date(val);
      return Number.isNaN(d.getTime()) ? null : d.toISOString();
    }
    case 'text':
    default:
      return String(val);
  }
}

function textFn(val: any, fn: 'upper' | 'lower' | 'proper' | 'trim' | 'clean'): string {
  const s = String(val ?? '');
  switch (fn) {
    case 'upper':
      return s.toUpperCase();
    case 'lower':
      return s.toLowerCase();
    case 'proper':
      return s.replace(/\w\S*/g, w => w[0].toUpperCase() + w.slice(1).toLowerCase());
    case 'trim':
      return s.trim();
    case 'clean':
      return s.replace(/[\x00-\x1F\x7F]/g, '');
    default:
      return s;
  }
}

function applyAgg(agg: Agg, rows: Record<string, any>[]): any {
  const vals = agg.column ? rows.map(r => r[agg.column as string]) : [];
  const nums = vals.map(Number).filter(n => !Number.isNaN(n));
  switch (agg.op) {
    case 'sum':
      return nums.reduce((a, b) => a + b, 0);
    case 'avg':
      return nums.length ? nums.reduce((a, b) => a + b, 0) / nums.length : null;
    case 'min':
      return nums.length ? Math.min(...nums) : null;
    case 'max':
      return nums.length ? Math.max(...nums) : null;
    case 'count':
      return vals.filter(v => v !== null && v !== undefined && v !== '').length;
    case 'countDistinct':
      return new Set(vals.map(v => JSON.stringify(v))).size;
    case 'countRows':
      return rows.length;
    case 'median': {
      if (!nums.length) return null;
      const sorted = [...nums].sort((a, b) => a - b);
      const mid = Math.floor(sorted.length / 2);
      return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
    }
    default:
      return null;
  }
}

/** Avaliador simples da expressão `each …` de addCustomColumn/addConditionalColumn. */
function evalCustomExpression(expression: string, row: Record<string, any>): any {
  let expr = expression.trim();
  if (expr.startsWith('each ')) expr = expr.slice(5);
  // Substitui [Coluna] por acesso seguro ao objeto `row`.
  const jsExpr = expr
    .replace(/\[([^\]]+)\]/g, (_m, col) => `__row[${JSON.stringify(col)}]`)
    .replace(/\btrue\b/g, 'true')
    .replace(/\bfalse\b/g, 'false')
    .replace(/&/g, '+')
    .replace(/\bnull\b/g, 'null')
    .replace(/\band\b/g, '&&')
    .replace(/\bor\b/g, '||')
    .replace(/\bnot\b/g, '!');
  try {
    // eslint-disable-next-line no-new-func
    const fn = new Function('__row', `"use strict"; return (${jsExpr});`);
    return fn(row);
  } catch {
    throw new Error(`Falha ao avaliar expressão: ${expression}`);
  }
}

function resolveThen(then: Scalar | { column: string }, row: Record<string, any>): any {
  if (then && typeof then === 'object' && 'column' in then) return row[then.column];
  return then;
}

interface StepResult {
  columns: EtlColumnSchema[];
  rows: Record<string, any>[];
  warning?: string;
}

function applyFallbackStep(columns: EtlColumnSchema[], rows: Record<string, any>[], step: EtlStep): StepResult {
  let cols = columns.map(c => ({ ...c }));
  let out = rows.map(r => ({ ...r }));

  switch (step.op) {
    case 'promoteHeaders': {
      if (out.length > 0) {
        const header = out[0];
        const renameMap: Record<string, string> = {};
        cols = cols.map((c, i) => {
          const newName = String(header[c.name] ?? `Coluna${i + 1}`);
          renameMap[c.name] = newName;
          return { ...c, name: newName };
        });
        out = out.slice(1).map(r => {
          const nr: Record<string, any> = {};
          Object.entries(renameMap).forEach(([oldN, newN]) => (nr[newN] = r[oldN]));
          return nr;
        });
      }
      break;
    }
    case 'changeType': {
      for (const { column, type } of step.columns) {
        cols = cols.map(c => (c.name === column ? { ...c, type } : c));
        out = out.map(r => ({ ...r, [column]: convertValue(r[column], type) }));
      }
      break;
    }
    case 'renameColumns': {
      for (const [from, to] of step.renames) {
        cols = cols.map(c => (c.name === from ? { ...c, name: to } : c));
        out = out.map(r => {
          const nr = { ...r };
          if (from in nr) {
            nr[to] = nr[from];
            if (from !== to) delete nr[from];
          }
          return nr;
        });
      }
      break;
    }
    case 'removeColumns': {
      cols = cols.filter(c => !step.columns.includes(c.name));
      out = out.map(r => {
        const nr = { ...r };
        step.columns.forEach(c => delete nr[c]);
        return nr;
      });
      break;
    }
    case 'selectColumns': {
      cols = cols.filter(c => step.columns.includes(c.name));
      out = out.map(r => {
        const nr: Record<string, any> = {};
        step.columns.forEach(c => (nr[c] = r[c]));
        return nr;
      });
      break;
    }
    case 'duplicateColumn': {
      const src = cols.find(c => c.name === step.column);
      cols = [...cols, { name: step.newColumn, type: src?.type ?? 'text' }];
      out = out.map(r => ({ ...r, [step.newColumn]: r[step.column] }));
      break;
    }
    case 'filterRows': {
      out = out.filter(r => evalCond(step.condition, r));
      break;
    }
    case 'removeBlankRows': {
      out = out.filter(r => Object.values(r).some(v => !isBlankValue(v)));
      break;
    }
    case 'removeRowsWithErrors': {
      // Fallback simplificado: não detecta erros de tipo sem reexecução do DuckDB.
      return { columns: cols, rows: out, warning: 'removeRowsWithErrors: não suportado no fallback de navegador' };
    }
    case 'keepTopRows':
      out = out.slice(0, step.count);
      break;
    case 'removeTopRows':
      out = out.slice(step.count);
      break;
    case 'keepBottomRows':
      out = out.slice(Math.max(0, out.length - step.count));
      break;
    case 'removeBottomRows':
      out = out.slice(0, Math.max(0, out.length - step.count));
      break;
    case 'removeDuplicates': {
      const seen = new Set<string>();
      const keyCols = step.columns && step.columns.length ? step.columns : cols.map(c => c.name);
      out = out.filter(r => {
        const key = JSON.stringify(keyCols.map(c => r[c]));
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });
      break;
    }
    case 'sort': {
      out = [...out].sort((a, b) => {
        for (const { column, descending } of step.by) {
          const av = a[column];
          const bv = b[column];
          if (av === bv) continue;
          const cmp = av < bv ? -1 : 1;
          return descending ? -cmp : cmp;
        }
        return 0;
      });
      break;
    }
    case 'replaceValues': {
      out = out.map(r => {
        const nr = { ...r };
        step.columns.forEach(c => {
          const cell = nr[c];
          if (step.matchEntireCell) {
            if (scalarsEqual(cell, step.find)) nr[c] = step.replace;
          } else if (typeof cell === 'string') {
            nr[c] = cell.split(String(step.find ?? '')).join(String(step.replace ?? ''));
          }
        });
        return nr;
      });
      break;
    }
    case 'fillDown': {
      const last: Record<string, any> = {};
      out = out.map(r => {
        const nr = { ...r };
        step.columns.forEach(c => {
          if (!isBlankValue(nr[c])) last[c] = nr[c];
          else nr[c] = last[c] ?? null;
        });
        return nr;
      });
      break;
    }
    case 'fillUp': {
      const next: Record<string, any> = {};
      for (let i = out.length - 1; i >= 0; i--) {
        const nr = { ...out[i] };
        step.columns.forEach(c => {
          if (!isBlankValue(nr[c])) next[c] = nr[c];
          else nr[c] = next[c] ?? null;
        });
        out[i] = nr;
      }
      break;
    }
    case 'splitColumn': {
      cols = [...cols, ...step.into.map(n => ({ name: n, type: 'text' }))];
      out = out.map(r => {
        const nr = { ...r };
        const raw = String(r[step.column] ?? '');
        let parts: string[] = [];
        if (step.mode === 'delimiterEach' || step.mode === 'delimiterLeftmost') {
          parts = raw.split(step.delimiter ?? ',');
        } else if (step.mode === 'delimiterRightmost') {
          const d = step.delimiter ?? ',';
          const idx = raw.lastIndexOf(d);
          parts = idx >= 0 ? [raw.slice(0, idx), raw.slice(idx + d.length)] : [raw];
        } else if (step.mode === 'positions' && step.positions) {
          let prev = 0;
          parts = [...step.positions, raw.length].map(p => {
            const piece = raw.slice(prev, p);
            prev = p;
            return piece;
          });
        } else if (step.mode === 'fixedLength' && step.length) {
          for (let i = 0; i < raw.length; i += step.length) parts.push(raw.slice(i, i + step.length));
        }
        step.into.forEach((name, i) => (nr[name] = parts[i] ?? null));
        return nr;
      });
      break;
    }
    case 'mergeColumns': {
      cols = [...cols, { name: step.newColumn, type: 'text' }];
      out = out.map(r => ({ ...r, [step.newColumn]: step.columns.map(c => r[c] ?? '').join(step.separator) }));
      break;
    }
    case 'textTransform': {
      out = out.map(r => {
        const nr = { ...r };
        step.columns.forEach(c => (nr[c] = textFn(nr[c], step.fn)));
        return nr;
      });
      break;
    }
    case 'unpivotOtherColumns':
    case 'unpivotColumns': {
      const attrCols = step.op === 'unpivotColumns' ? step.columns : cols.map(c => c.name).filter(n => !step.keep.includes(n));
      const keep = step.op === 'unpivotColumns' ? cols.map(c => c.name).filter(n => !step.columns.includes(n)) : step.keep;
      const nrs: Record<string, any>[] = [];
      out.forEach(r => {
        attrCols.forEach(col => {
          const nr: Record<string, any> = {};
          keep.forEach(k => (nr[k] = r[k]));
          nr[step.attributeColumn] = col;
          nr[step.valueColumn] = r[col];
          nrs.push(nr);
        });
      });
      out = nrs;
      cols = [...keep.map(k => cols.find(c => c.name === k)!), { name: step.attributeColumn, type: 'text' }, { name: step.valueColumn, type: 'text' }];
      break;
    }
    case 'pivot': {
      const groupKeys = cols.map(c => c.name).filter(n => n !== step.column && n !== step.valuesColumn);
      const groups = new Map<string, Record<string, any>[]>();
      out.forEach(r => {
        const key = JSON.stringify(groupKeys.map(k => r[k]));
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key)!.push(r);
      });
      const pivotValues = Array.from(new Set(out.map(r => String(r[step.column]))));
      const nrs: Record<string, any>[] = [];
      groups.forEach(groupRows => {
        const nr: Record<string, any> = {};
        groupKeys.forEach(k => (nr[k] = groupRows[0][k]));
        pivotValues.forEach(pv => {
          const matching = groupRows.filter(r => String(r[step.column]) === pv);
          nr[pv] = applyAgg({ name: pv, op: step.aggregation, column: step.valuesColumn }, matching);
        });
        nrs.push(nr);
      });
      out = nrs;
      cols = [...groupKeys.map(k => cols.find(c => c.name === k)!), ...pivotValues.map(v => ({ name: v, type: 'decimal' }))];
      break;
    }
    case 'groupBy': {
      const groups = new Map<string, Record<string, any>[]>();
      out.forEach(r => {
        const key = JSON.stringify(step.keys.map(k => r[k]));
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key)!.push(r);
      });
      const nrs: Record<string, any>[] = [];
      groups.forEach(groupRows => {
        const nr: Record<string, any> = {};
        step.keys.forEach(k => (nr[k] = groupRows[0][k]));
        step.aggregations.forEach(agg => (nr[agg.name] = applyAgg(agg, groupRows)));
        nrs.push(nr);
      });
      out = nrs;
      cols = [...step.keys.map(k => cols.find(c => c.name === k) ?? { name: k, type: 'text' }), ...step.aggregations.map(a => ({ name: a.name, type: 'decimal' }))];
      break;
    }
    case 'addConditionalColumn': {
      cols = [...cols, { name: step.newColumn, type: step.type ?? 'text' }];
      out = out.map(r => {
        const match = step.rules.find(rule => evalCond(rule.when, r));
        const value = match ? resolveThen(match.then, r) : resolveThen(step.otherwise, r);
        return { ...r, [step.newColumn]: value };
      });
      break;
    }
    case 'addCustomColumn': {
      cols = [...cols, { name: step.newColumn, type: step.type ?? 'text' }];
      out = out.map(r => ({ ...r, [step.newColumn]: evalCustomExpression(step.expression, r) }));
      break;
    }
    case 'addIndexColumn': {
      cols = [...cols, { name: step.newColumn, type: 'integer' }];
      out = out.map((r, i) => ({ ...r, [step.newColumn]: step.start + i * step.step }));
      break;
    }
    case 'mergeQueries': {
      const right = rawProvider?.(step.rightQuery);
      if (!right) {
        return { columns: cols, rows: out, warning: `mergeQueries: consulta "${step.rightQuery}" não encontrada no fallback de navegador` };
      }
      const rightRows = right.rows;
      const nrs: Record<string, any>[] = [];
      out.forEach(lr => {
        const matches = rightRows.filter(rr => step.leftKeys.every((lk, i) => scalarsEqual(lr[lk], rr[step.rightKeys[i]])));
        if (matches.length === 0 && (step.joinKind === 'leftOuter' || step.joinKind === 'fullOuter')) {
          const nr = { ...lr };
          step.expand.forEach(e => (nr[e.as] = null));
          nrs.push(nr);
        } else if (step.joinKind === 'leftAnti') {
          if (matches.length === 0) nrs.push({ ...lr });
        } else {
          matches.forEach(rr => {
            const nr = { ...lr };
            step.expand.forEach(e => (nr[e.as] = rr[e.column]));
            nrs.push(nr);
          });
        }
      });
      out = nrs;
      cols = [...cols, ...step.expand.map(e => ({ name: e.as, type: 'text' }))];
      break;
    }
    case 'appendQueries': {
      const extra = step.queries.map(q => rawProvider?.(q)).filter(Boolean) as BrowserRawTable[];
      extra.forEach(t => (out = [...out, ...t.rows]));
      break;
    }
    case 'opaqueM': {
      return { columns: cols, rows: out, warning: 'Passo M não suportado (opaqueM): mantido sem execução.' };
    }
    default:
      break;
  }
  return { columns: cols, rows: out };
}

function runPipeline(query: EtlQuery, uptoStep?: number): { columns: EtlColumnSchema[]; rows: Record<string, any>[]; warnings: string[]; failedStep?: { index: number; message: string } } {
  const raw = rawProvider?.(query.name) ?? rawProvider?.(query.source.table ?? '') ?? { columns: [], rows: [] };
  let columns = raw.columns;
  let rows = raw.rows;
  const warnings: string[] = [];
  const lastIndex = uptoStep !== undefined ? uptoStep : query.steps.length - 1;

  for (let i = 0; i <= lastIndex && i < query.steps.length; i++) {
    const step = query.steps[i];
    try {
      const result = applyFallbackStep(columns, rows, step);
      columns = result.columns;
      rows = result.rows;
      if (result.warning) warnings.push(result.warning);
    } catch (e: any) {
      return { columns, rows, warnings, failedStep: { index: i, message: e?.message ?? String(e) } };
    }
  }
  return { columns, rows, warnings };
}

function browserPreview(query: EtlQuery, uptoStep: number | undefined, limit: number): EtlPreviewResult {
  const result = runPipeline(query, uptoStep);
  return {
    columns: result.columns,
    rows: result.rows.slice(0, limit),
    rowCount: result.rows.length,
    warnings: [ETL_BROWSER_FALLBACK_NOTICE, ...result.warnings],
    failedStep: result.failedStep,
  };
}

function browserApply(query: EtlQuery, targetTable: string): ImportedTablePayload {
  const result = runPipeline(query);
  return {
    tableName: targetTable,
    originalName: targetTable,
    columns: result.columns.map(c => ({ name: c.name, originalName: c.name, dataType: c.type })),
    rows: result.rows,
    totalRows: result.rows.length,
    truncated: false,
  };
}

// --- M mínimo (apenas para exibição/edição básica fora do Tauri) ---

function stepToM(step: EtlStep): string {
  switch (step.op) {
    case 'promoteHeaders':
      return 'Table.PromoteHeaders(Fonte, [PromoteAllScalars=true])';
    case 'changeType':
      return `Table.TransformColumnTypes(Prev, {${step.columns.map(c => `{"${c.column}", type ${c.type}}`).join(', ')}})`;
    case 'renameColumns':
      return `Table.RenameColumns(Prev, {${step.renames.map(([a, b]) => `{"${a}", "${b}"}`).join(', ')}})`;
    case 'removeColumns':
      return `Table.RemoveColumns(Prev, {${step.columns.map(c => `"${c}"`).join(', ')}})`;
    case 'selectColumns':
      return `Table.SelectColumns(Prev, {${step.columns.map(c => `"${c}"`).join(', ')}})`;
    case 'filterRows':
      return `Table.SelectRows(Prev, each ${condToM(step.condition)})`;
    case 'addCustomColumn':
      return `Table.AddColumn(Prev, "${step.newColumn}", ${step.expression})`;
    case 'addIndexColumn':
      return `Table.AddIndexColumn(Prev, "${step.newColumn}", ${step.start}, ${step.step})`;
    case 'sort':
      return `Table.Sort(Prev, {${step.by.map(b => `{"${b.column}", Order.${b.descending ? 'Descending' : 'Ascending'}}`).join(', ')}})`;
    case 'groupBy':
      return `Table.Group(Prev, {${step.keys.map(k => `"${k}"`).join(', ')}}, {${step.aggregations
        .map(a => `{"${a.name}", each List.${aggFn(a.op)}(_[${a.column ?? ''}]), type nullable number}`)
        .join(', ')}})`;
    case 'opaqueM':
      return step.m;
    default:
      return `/* ${step.op} não representável no fallback de navegador */ Prev`;
  }
}

function aggFn(op: Agg['op']): string {
  const map: Record<string, string> = { sum: 'Sum', avg: 'Average', min: 'Min', max: 'Max', count: 'Count', countDistinct: 'Distinct', countRows: 'Count', median: 'Median' };
  return map[op] ?? 'Sum';
}

function condToM(cond: Cond): string {
  if ('and' in cond) return `(${cond.and.map(condToM).join(' and ')})`;
  if ('or' in cond) return `(${cond.or.map(condToM).join(' or ')})`;
  if ('not' in cond) return `not (${condToM(cond.not)})`;
  const col = `[${cond.column}]`;
  switch (cond.op) {
    case 'eq':
      return `${col} = ${JSON.stringify(cond.value)}`;
    case 'neq':
      return `${col} <> ${JSON.stringify(cond.value)}`;
    case 'gt':
      return `${col} > ${JSON.stringify(cond.value)}`;
    case 'gte':
      return `${col} >= ${JSON.stringify(cond.value)}`;
    case 'lt':
      return `${col} < ${JSON.stringify(cond.value)}`;
    case 'lte':
      return `${col} <= ${JSON.stringify(cond.value)}`;
    case 'contains':
      return `Text.Contains(${col}, ${JSON.stringify(cond.value)})`;
    case 'notContains':
      return `not Text.Contains(${col}, ${JSON.stringify(cond.value)})`;
    case 'startsWith':
      return `Text.StartsWith(${col}, ${JSON.stringify(cond.value)})`;
    case 'endsWith':
      return `Text.EndsWith(${col}, ${JSON.stringify(cond.value)})`;
    case 'isNull':
      return `${col} = null`;
    case 'isNotNull':
      return `${col} <> null`;
    case 'isBlank':
      return `${col} = null or ${col} = ""`;
    case 'isNotBlank':
      return `${col} <> null and ${col} <> ""`;
    case 'in':
      return `List.Contains({${cond.values.map(v => JSON.stringify(v)).join(', ')}}, ${col})`;
    case 'notIn':
      return `not List.Contains({${cond.values.map(v => JSON.stringify(v)).join(', ')}}, ${col})`;
    default:
      return 'true';
  }
}

function browserToM(query: EtlQuery): string {
  const lines = ['let', `    Fonte = ${sourceToM(query.source)},`];
  let prev = 'Fonte';
  query.steps.forEach((step, i) => {
    const varName = `#"${step.name}"`;
    const body = stepToM(step).replace(/\bPrev\b/g, prev);
    lines.push(`    ${varName} = ${body}${i < query.steps.length - 1 ? ',' : ''}`);
    prev = varName;
  });
  lines.push('in', `    ${prev}`);
  return lines.join('\n');
}

function sourceToM(source: EtlSource): string {
  switch (source.kind) {
    case 'csv':
      return `Csv.Document(File.Contents("${source.path ?? ''}"))`;
    case 'excel':
      return `Excel.Workbook(File.Contents("${source.path ?? ''}")){[Item="${source.sheet ?? ''}"]}[Data]`;
    case 'parquet':
      return `Parquet.Document(File.Contents("${source.path ?? ''}"))`;
    case 'json':
      return `Json.Document(File.Contents("${source.path ?? ''}"))`;
    case 'table':
      return `#"${source.table ?? ''}"`;
    case 'opaqueM':
      return source.m ?? 'null';
    default:
      return 'null';
  }
}

function browserFromM(name: string, m: string): EtlFromMResult {
  // Suporte mínimo de round-trip: reconhece o formato gerado por browserToM
  // para os passos mais comuns. Qualquer trecho não reconhecido vira opaqueM.
  const warnings: string[] = [ETL_BROWSER_FALLBACK_NOTICE];
  const steps: EtlStep[] = [];
  const stepLineRe = /#"([^"]+)"\s*=\s*(.+?),?\s*$/;
  const bodyLines = m
    .split('\n')
    .map(l => l.trim())
    .filter(l => l && l !== 'let' && l !== 'in' && !l.startsWith('Fonte ='));

  bodyLines.forEach((line, idx) => {
    const match = line.match(stepLineRe);
    if (!match) return;
    const [, stepName, body] = match;
    const id = `step_${idx}_${Math.random().toString(36).slice(2, 8)}`;
    const parsed = parseMBody(id, stepName, body);
    if (parsed) steps.push(parsed);
    else {
      warnings.push(`Passo "${stepName}" não reconhecido pelo parser do navegador; preservado como M opaco.`);
      steps.push({ id, name: stepName, op: 'opaqueM', m: body });
    }
  });

  return {
    query: { name, source: { kind: 'opaqueM', m: 'Fonte' }, steps },
    warnings,
  };
}

function parseMBody(id: string, name: string, body: string): EtlStep | null {
  let m: RegExpMatchArray | null;
  if ((m = body.match(/^Table\.RemoveColumns\([^,]+,\s*\{(.+)\}\)$/))) {
    const columns = [...m[1].matchAll(/"([^"]+)"/g)].map(x => x[1]);
    return { id, name, op: 'removeColumns', columns };
  }
  if ((m = body.match(/^Table\.SelectColumns\([^,]+,\s*\{(.+)\}\)$/))) {
    const columns = [...m[1].matchAll(/"([^"]+)"/g)].map(x => x[1]);
    return { id, name, op: 'selectColumns', columns };
  }
  if ((m = body.match(/^Table\.RenameColumns\([^,]+,\s*\{(.+)\}\)$/))) {
    const pairs = [...m[1].matchAll(/\{"([^"]+)",\s*"([^"]+)"\}/g)].map(x => [x[1], x[2]] as [string, string]);
    return { id, name, op: 'renameColumns', renames: pairs };
  }
  if (body.match(/^Table\.PromoteHeaders/)) {
    return { id, name, op: 'promoteHeaders' };
  }
  if ((m = body.match(/^Table\.AddIndexColumn\([^,]+,\s*"([^"]+)",\s*(-?\d+),\s*(-?\d+)\)$/))) {
    return { id, name, op: 'addIndexColumn', newColumn: m[1], start: Number(m[2]), step: Number(m[3]) };
  }
  return null;
}
