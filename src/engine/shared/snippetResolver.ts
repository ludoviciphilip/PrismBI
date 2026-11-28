/**
 * v0.11 Rodada 2 (bugfix): DAX_SNIPPETS used to be static strings that
 * referenced example tables/measures ('d_Calendario', [Medida],
 * 'd_Produtos'...) that don't exist in a real project. Inserted as-is, they
 * "inserted the formula" (Luis's words) but never ran: wrong table/column
 * names, and most of them are bare MEASURE *bodies* (VAR/RETURN, or a
 * single CALCULATE/TOTALYTD call), not DEFINE/EVALUATE *queries*, so the
 * DAX Query View had nothing to execute.
 *
 * This module rebuilds each snippet against the CURRENT model:
 *  - `buildModelParts` picks real stand-ins (first fact table, first
 *    calendar/date dimension + its date column, a year-like group column,
 *    a first non-fact/non-date dimension + a text column on it for
 *    ranking/Pareto snippets, and the first measure).
 *  - `resolveDaxSnippet` fills the snippet's placeholders with those real
 *    names and, for the DAX Query View, wraps the measure body in a
 *    `DEFINE MEASURE ... EVALUATE SUMMARIZECOLUMNS(...)` query grouped by
 *    year/item so it actually returns rows (Power BI's own "quick measure"
 *    insertion does the same: a runnable example, not just the formula).
 *  - When the model doesn't have what a snippet needs (no date table, no
 *    measure...), the snippet comes back `enabled: false` with a
 *    human-readable `disabledReason` for the UI to show in a tooltip
 *    instead of silently inserting something broken.
 */
import { Table, Measure, Column } from '../../types/bi';

export interface ModelSnippetParts {
  factTable: Table | null;
  dateTable: Table | null;
  dateColumn: Column | null;
  /** A "year"-like column on `dateTable` to group by (falls back to `dateColumn` itself). */
  groupColumn: Column | null;
  /** First dimension table that isn't the fact table or the date table (products, categories...). */
  itemTable: Table | null;
  /** A text column on `itemTable` to group/rank by. */
  itemColumn: Column | null;
  measure: Measure | null;
}

export interface ResolvedDaxSnippet {
  /** Full `DEFINE MEASURE ... EVALUATE ...` query, ready to paste into the DAX Query View. */
  query: string;
  /** Bare measure body (no DEFINE/EVALUATE), valid directly in the measure formula editor. */
  measureBody: string;
  /** Name given to the temporary/example measure (`[Teste]`-style alias). */
  measureName: string;
  enabled: boolean;
  /** Set when `enabled` is false: why this snippet can't run against the current model. */
  disabledReason?: string;
}

function quoteTable(name: string): string {
  return `'${name}'`;
}

function col(table: Table, column: Column): string {
  return `${quoteTable(table.name)}[${column.name}]`;
}

function meas(measure: Measure): string {
  return `[${measure.name}]`;
}

function looksLikeFact(name: string): boolean {
  return /^f[_\s-]/i.test(name) || /fato|fact/i.test(name);
}

function looksLikeDateDim(name: string): boolean {
  return /^d[_\s-]/i.test(name) || /calend|date|data/i.test(name);
}

function pickFactTable(tables: Table[]): Table | null {
  if (tables.length === 0) return null;
  const named = tables.find(t => looksLikeFact(t.name));
  if (named) return named;
  // Heuristic fallback: the table with the most rows is usually the fact table.
  return [...tables].sort((a, b) => (b.rows?.length ?? 0) - (a.rows?.length ?? 0))[0];
}

function pickDateTable(tables: Table[]): Table | null {
  const named = tables.find(t => looksLikeDateDim(t.name) && t.columns.some(c => c.dataType === 'date'));
  if (named) return named;
  return tables.find(t => t.columns.some(c => c.dataType === 'date')) || null;
}

function pickDateColumn(table: Table | null): Column | null {
  if (!table) return null;
  return table.columns.find(c => c.dataType === 'date') || null;
}

function pickGroupColumn(table: Table | null, dateColumn: Column | null): Column | null {
  if (!table) return null;
  return table.columns.find(c => /^ano$|^year$/i.test(c.name)) || dateColumn || table.columns[0] || null;
}

function pickItemTable(tables: Table[], factTable: Table | null, dateTable: Table | null): Table | null {
  return tables.find(t => t !== factTable && t !== dateTable) || null;
}

function pickItemColumn(table: Table | null): Column | null {
  if (!table) return null;
  return table.columns.find(c => c.dataType === 'text') || table.columns[0] || null;
}

export function buildModelParts(tables: Table[], measures: Measure[]): ModelSnippetParts {
  const factTable = pickFactTable(tables);
  const dateTable = pickDateTable(tables);
  const dateColumn = pickDateColumn(dateTable);
  const groupColumn = pickGroupColumn(dateTable, dateColumn);
  const itemTable = pickItemTable(tables, factTable, dateTable);
  const itemColumn = pickItemColumn(itemTable);
  const measure = measures[0] || null;
  return { factTable, dateTable, dateColumn, groupColumn, itemTable, itemColumn, measure };
}

function wrapAsQuery(factTable: Table, measureName: string, body: string, groupTable: Table, groupColumn: Column): string {
  const indentedBody = body.replace(/\n/g, '\n    ');
  return (
    `DEFINE\n` +
    `  MEASURE ${quoteTable(factTable.name)}[${measureName}] =\n` +
    `    ${indentedBody}\n\n` +
    `EVALUATE\n` +
    `  SUMMARIZECOLUMNS(\n` +
    `    ${col(groupTable, groupColumn)},\n` +
    `    "${measureName}", [${measureName}]\n` +
    `  )\n` +
    `ORDER BY\n` +
    `  ${col(groupTable, groupColumn)} ASC`
  );
}

function disabled(reason: string): ResolvedDaxSnippet {
  return { query: '', measureBody: '', measureName: '', enabled: false, disabledReason: reason };
}

const NO_MEASURE = 'O modelo não tem nenhuma medida para basear este snippet.';
const NO_DATE_DIM = 'O modelo não tem uma tabela de calendário com coluna de data para inteligência de tempo.';
const NO_ITEM_DIM = 'O modelo não tem uma tabela de dimensão (além do fato e do calendário) para agrupar/ranquear.';
const NO_FACT = 'O modelo não tem nenhuma tabela para hospedar a medida de exemplo.';

/** Resolves a `DAX_SNIPPETS` entry (by id) against the current model. */
export function resolveDaxSnippet(snippetId: string, parts: ModelSnippetParts): ResolvedDaxSnippet {
  const { factTable, dateTable, dateColumn, groupColumn, itemTable, itemColumn, measure } = parts;

  switch (snippetId) {
    case 'snippet_yoy': {
      if (!factTable) return disabled(NO_FACT);
      if (!measure) return disabled(NO_MEASURE);
      if (!dateTable || !dateColumn || !groupColumn) return disabled(NO_DATE_DIM);
      const measureName = 'Crescimento YoY %';
      const body =
        `VAR ValorAtual = ${meas(measure)}\n` +
        `VAR AnoAnterior = CALCULATE(${meas(measure)}, SAMEPERIODLASTYEAR(${col(dateTable, dateColumn)}))\n` +
        `RETURN DIVIDE(ValorAtual - AnoAnterior, AnoAnterior, 0)`;
      return { query: wrapAsQuery(factTable, measureName, body, dateTable, groupColumn), measureBody: body, measureName, enabled: true };
    }

    case 'snippet_ytd': {
      if (!factTable) return disabled(NO_FACT);
      if (!measure) return disabled(NO_MEASURE);
      if (!dateTable || !dateColumn || !groupColumn) return disabled(NO_DATE_DIM);
      const measureName = 'Acumulado no Ano (YTD)';
      const body = `TOTALYTD(${meas(measure)}, ${col(dateTable, dateColumn)})`;
      return { query: wrapAsQuery(factTable, measureName, body, dateTable, groupColumn), measureBody: body, measureName, enabled: true };
    }

    case 'snippet_mom': {
      if (!factTable) return disabled(NO_FACT);
      if (!measure) return disabled(NO_MEASURE);
      if (!dateTable || !dateColumn || !groupColumn) return disabled(NO_DATE_DIM);
      const measureName = 'Crescimento MoM %';
      const body =
        `VAR MesAtual = ${meas(measure)}\n` +
        `VAR MesAnterior = CALCULATE(${meas(measure)}, DATEADD(${col(dateTable, dateColumn)}, -1, MONTH))\n` +
        `RETURN DIVIDE(MesAtual - MesAnterior, MesAnterior, 0)`;
      return { query: wrapAsQuery(factTable, measureName, body, dateTable, groupColumn), measureBody: body, measureName, enabled: true };
    }

    case 'snippet_pareto': {
      if (!factTable) return disabled(NO_FACT);
      if (!measure) return disabled(NO_MEASURE);
      if (!itemTable || !itemColumn) return disabled(NO_ITEM_DIM);
      const measureName = 'Acumulado % (Pareto)';
      const body =
        `VAR TotalGeral = CALCULATE(${meas(measure)}, ALL(${quoteTable(itemTable.name)}))\n` +
        `VAR ValorItem = ${meas(measure)}\n` +
        `VAR Acumulado =\n` +
        `  CALCULATE(\n` +
        `    ${meas(measure)},\n` +
        `    FILTER(ALL(${quoteTable(itemTable.name)}), ${meas(measure)} >= ValorItem)\n` +
        `  )\n` +
        `RETURN DIVIDE(Acumulado, TotalGeral, 0)`;
      return { query: wrapAsQuery(factTable, measureName, body, itemTable, itemColumn), measureBody: body, measureName, enabled: true };
    }

    case 'snippet_topn_others': {
      if (!factTable) return disabled(NO_FACT);
      if (!measure) return disabled(NO_MEASURE);
      if (!itemTable || !itemColumn) return disabled(NO_ITEM_DIM);
      const measureName = 'Top 5 ou Outros';
      const body =
        `VAR Ranking = RANKX(ALL(${col(itemTable, itemColumn)}), ${meas(measure)}, , DESC)\n` +
        `RETURN\n` +
        `  IF(\n` +
        `    Ranking <= 5,\n` +
        `    ${meas(measure)},\n` +
        `    CALCULATE(${meas(measure)}, ALL(${quoteTable(itemTable.name)})) -\n` +
        `      CALCULATE(${meas(measure)}, TOPN(5, ALL(${col(itemTable, itemColumn)}), ${meas(measure)}, DESC))\n` +
        `  )`;
      return { query: wrapAsQuery(factTable, measureName, body, itemTable, itemColumn), measureBody: body, measureName, enabled: true };
    }

    case 'snippet_query_template': {
      if (!factTable) return disabled(NO_FACT);
      if (!measure) return disabled(NO_MEASURE);
      if (!dateTable || !dateColumn || !groupColumn) return disabled(NO_DATE_DIM);
      const measureName = 'Resultado da Consulta';
      const body = `${meas(measure)} * 1`;
      return { query: wrapAsQuery(factTable, measureName, body, dateTable, groupColumn), measureBody: body, measureName, enabled: true };
    }

    default:
      return disabled(`Snippet desconhecido: ${snippetId}.`);
  }
}
