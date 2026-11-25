/**
 * Corpus de conformidade DAX compartilhado entre os motores TS e Rust.
 *
 * Os casos ficam em `tests/dax-corpus/` (formato descrito no README de lá).
 * Este arquivo roda o corpus no motor TypeScript; `src-tauri/tests/dax_corpus.rs`
 * roda o mesmo corpus no motor Rust. Ambos usam uma "catraca"
 * (`tests/dax-corpus/baseline.json`): um caso que passava não pode voltar a
 * falhar; casos novos que passam são informados para atualizar a baseline com
 * `UPDATE_DAX_BASELINE=1 pnpm test dax_corpus`.
 */
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { DaxEngine } from '../daxEngine';
import { InMemoryModelAccess } from '../model/modelAccess';
import { FilterContext } from '../model/filterContext';
import type { Table, Relationship, Measure, DataType } from '../../types/bi';

const CORPUS_DIR = path.resolve(__dirname, '../../../tests/dax-corpus');
const ENGINE = 'ts';

interface CorpusColumn { name: string; type: DataType }
interface CorpusTable {
  name: string;
  columns: CorpusColumn[];
  rows?: unknown[][];
  calendar?: { start: string; end: string };
}
interface CorpusModel {
  tables: CorpusTable[];
  relationships: { from: string; to: string; crossFilter: 'single' | 'both'; active: boolean }[];
  measures: { table: string; name: string; formula: string }[];
}
interface CorpusFilter { table: string; column: string; values: unknown[] }
interface CorpusCase {
  id: string;
  expr: string;
  filters?: CorpusFilter[];
  expected: unknown;
  tolerance?: number;
  notes?: string;
}
interface CorpusFile { model: string; category: string; cases: CorpusCase[] }

function splitRef(ref: string): [string, string] {
  const m = /^(.+)\[(.+)\]$/.exec(ref.trim());
  if (!m) throw new Error(`Referência inválida: ${ref}`);
  return [m[1], m[2]];
}

/** Calendário diário: Data (ISO), Ano, MesNum, Trimestre, AnoMes. Igual ao gerador Rust. */
export function generateCalendarRows(start: string, end: string): Record<string, unknown>[] {
  const rows: Record<string, unknown>[] = [];
  const d = new Date(`${start}T00:00:00Z`);
  const last = new Date(`${end}T00:00:00Z`);
  while (d <= last) {
    const y = d.getUTCFullYear();
    const m = d.getUTCMonth() + 1;
    rows.push({
      Data: d.toISOString().slice(0, 10),
      Ano: y,
      MesNum: m,
      Trimestre: Math.floor((m - 1) / 3) + 1,
      AnoMes: `${y}-${String(m).padStart(2, '0')}`,
    });
    d.setUTCDate(d.getUTCDate() + 1);
  }
  return rows;
}

function buildModel(model: CorpusModel): InMemoryModelAccess {
  const tables: Table[] = model.tables.map(t => {
    const rows = t.calendar
      ? generateCalendarRows(t.calendar.start, t.calendar.end)
      : (t.rows ?? []).map(r => Object.fromEntries(t.columns.map((c, i) => [c.name, r[i] ?? null])));
    return {
      id: t.name,
      name: t.name,
      originalName: t.name,
      columns: t.columns.map(c => ({ id: `${t.name}.${c.name}`, name: c.name, originalName: c.name, dataType: c.type })),
      rows,
      ...(t.calendar ? { isCalendar: true, calendarDateColumn: 'Data' } : {}),
    } as Table;
  });
  const relationships: Relationship[] = model.relationships.map((r, i) => {
    const [ft, fc] = splitRef(r.from);
    const [tt, tc] = splitRef(r.to);
    return {
      id: `rel_${i}`,
      fromTable: ft,
      fromColumn: fc,
      toTable: tt,
      toColumn: tc,
      cardinality: 'N:1',
      crossFilterDirection: r.crossFilter,
      isActive: r.active,
    };
  });
  const measures: Measure[] = model.measures.map((m, i) => ({
    id: `m_${i}`,
    name: m.name,
    tableName: m.table,
    tableId: m.table,
    formula: m.formula,
    format: 'number',
  }));
  return new InMemoryModelAccess(tables, relationships, measures);
}

function buildFilterContext(filters: CorpusFilter[] = []): FilterContext {
  const fc = new FilterContext();
  for (const f of filters) {
    fc.setColumnFilter(f.table, f.column, {
      tableId: f.table,
      column: f.column,
      allowedValues: new Set(f.values),
    });
  }
  return fc;
}

function normalizeActual(v: unknown): unknown {
  if (v === undefined || v === null) return null;
  if (v instanceof Date) return { date: v.toISOString().slice(0, 10) };
  if (typeof v === 'object' && v && 'value' in (v as any) && Object.keys(v as any).length <= 2) {
    return normalizeActual((v as any).value);
  }
  return v;
}

export function valuesMatch(expected: unknown, actual: unknown, tolerance = 1e-9): boolean {
  const a = normalizeActual(actual);
  if (expected === null) return a === null;
  if (typeof expected === 'number') {
    if (typeof a !== 'number' || !Number.isFinite(a)) return false;
    return Math.abs(a - expected) <= tolerance * Math.max(1, Math.abs(expected));
  }
  if (typeof expected === 'object' && expected && 'date' in (expected as any)) {
    const want = (expected as any).date as string;
    if (typeof a === 'string') return a.slice(0, 10) === want;
    if (a && typeof a === 'object' && 'date' in (a as any)) return (a as any).date === want;
    return false;
  }
  if (typeof expected === 'object' && expected && 'error' in (expected as any)) return false;
  return a === expected;
}

function loadCorpus() {
  const modelsDir = path.join(CORPUS_DIR, 'models');
  const models = new Map<string, CorpusModel>();
  for (const f of fs.readdirSync(modelsDir).filter(f => f.endsWith('.json'))) {
    models.set(f.replace(/\.json$/, ''), JSON.parse(fs.readFileSync(path.join(modelsDir, f), 'utf8')));
  }
  const casesDir = path.join(CORPUS_DIR, 'cases');
  const files: CorpusFile[] = fs
    .readdirSync(casesDir)
    .filter(f => f.endsWith('.json'))
    .sort()
    .map(f => JSON.parse(fs.readFileSync(path.join(casesDir, f), 'utf8')));
  return { models, files };
}

describe('Corpus de conformidade DAX (motor TS)', () => {
  const { models, files } = loadCorpus();
  const baselinePath = path.join(CORPUS_DIR, 'baseline.json');
  const baseline: Record<string, string[]> = fs.existsSync(baselinePath)
    ? JSON.parse(fs.readFileSync(baselinePath, 'utf8'))
    : {};

  it('ids de casos são únicos e todo arquivo referencia um modelo existente', () => {
    const ids = files.flatMap(f => f.cases.map(c => c.id));
    expect(new Set(ids).size).toBe(ids.length);
    for (const f of files) expect(models.has(f.model), `modelo ${f.model}`).toBe(true);
  });

  it('não regride casos da baseline e registra o relatório', () => {
    const built = new Map<string, InMemoryModelAccess>();
    const results: { id: string; category: string; pass: boolean; actual: unknown; error?: string }[] = [];

    for (const file of files) {
      if (!built.has(file.model)) built.set(file.model, buildModel(models.get(file.model)!));
      const model = built.get(file.model)!;
      for (const c of file.cases) {
        const expectError = typeof c.expected === 'object' && c.expected !== null && 'error' in (c.expected as any);
        try {
          const actual = DaxEngine.evaluateWithContext(c.expr, model, buildFilterContext(c.filters));
          const isErr = typeof actual === 'string' && /^#ERROR|^Erro/i.test(actual);
          const pass = expectError ? isErr : valuesMatch(c.expected, actual, c.tolerance);
          results.push({ id: c.id, category: file.category, pass, actual: normalizeActual(actual) });
        } catch (err: any) {
          results.push({ id: c.id, category: file.category, pass: expectError, actual: null, error: String(err?.message ?? err) });
        }
      }
    }

    const passing = results.filter(r => r.pass).map(r => r.id).sort();
    fs.writeFileSync(
      path.join(CORPUS_DIR, `.report-${ENGINE}.json`),
      JSON.stringify({ engine: ENGINE, total: results.length, passing: passing.length, results }, null, 2)
    );

    if (process.env.UPDATE_DAX_BASELINE === '1') {
      const next = { ...baseline, [ENGINE]: passing };
      fs.writeFileSync(baselinePath, JSON.stringify(next, null, 2) + '\n');
    }

    const known = new Set(baseline[ENGINE] ?? []);
    const regressions = results.filter(r => known.has(r.id) && !r.pass);
    const newlyPassing = passing.filter(id => !known.has(id));
    if (newlyPassing.length && process.env.UPDATE_DAX_BASELINE !== '1') {
      console.info(`[dax-corpus:${ENGINE}] ${newlyPassing.length} caso(s) novo(s) passando — rode com UPDATE_DAX_BASELINE=1 para registrar.`);
    }
    expect(
      regressions.map(r => `${r.id}: obtido ${JSON.stringify(r.actual)}${r.error ? ` (${r.error})` : ''}`),
      'casos que passavam na baseline e agora falham'
    ).toEqual([]);
  });
});
