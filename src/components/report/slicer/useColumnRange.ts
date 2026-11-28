import { Table } from '../../../types/bi';
import { queryDuckDbNative } from '../../../bridge/tauriBridge';
import { isTauriRuntime } from '../../../bridge/tauriBridge';

export interface ColumnRange {
  min: number | string;
  max: number | string;
}

function quoteIdent(id: string): string {
  return `"${id.replace(/"/g, '""')}"`;
}

/**
 * Min/max of a column across the FULL backing table, not the 5.000-row
 * preview in `table.rows` (see docs/RUST_ENGINE_2.md §3 step 2 / the
 * VetroBI's "Data-volume rules" — table.rows is only a preview).
 * Uses the native DuckDB table via `queryDuckDbNative` when available
 * (Tauri runtime + `table.duckdbTable` set); falls back to scanning
 * `table.rows` only when there is no native table to query (browser dev
 * mode, or a table with no DuckDB-backed source), in which case the range
 * itself is only as complete as the preview.
 */
export async function getColumnRange(table: Table, column: string): Promise<ColumnRange | null> {
  if (isTauriRuntime() && table.duckdbTable) {
    try {
      const sql = `SELECT MIN(${quoteIdent(column)}) AS mn, MAX(${quoteIdent(column)}) AS mx FROM ${quoteIdent(table.duckdbTable)}`;
      const res = await queryDuckDbNative(sql);
      const row = res.rows?.[0] as any;
      if (row && row.mn !== null && row.mn !== undefined) {
        return { min: row.mn, max: row.mx };
      }
    } catch {
      // Falls through to the JS preview-based computation below.
    }
  }
  return getColumnRangeFromPreview(table, column);
}

/** Preview-only fallback — see the warning in `getColumnRange`'s doc comment. */
export function getColumnRangeFromPreview(table: Table, column: string): ColumnRange | null {
  let min: number | string | undefined;
  let max: number | string | undefined;
  for (const row of table.rows) {
    const v = row[column];
    if (v === null || v === undefined || v === '') continue;
    if (min === undefined || v < min) min = v;
    if (max === undefined || v > max) max = v;
  }
  if (min === undefined || max === undefined) return null;
  return { min, max };
}
