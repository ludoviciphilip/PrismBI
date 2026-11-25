import type { Table } from '../../types/bi';
import type { EtlSource, Scalar } from '../../types/etl';

/** Fonte relida do disco (o raw é recarregável), em vez de um snapshot. */
export function fileEtlSource(table?: Pick<Table, 'sourceConfig'>): EtlSource | null {
  const src = table?.sourceConfig;
  if (!src?.filePath) return null;
  switch (src.type) {
    case 'xlsx':
      return { kind: 'excel', path: src.filePath, sheet: src.sheetName };
    case 'csv':
      return { kind: 'csv', path: src.filePath };
    case 'parquet':
      return { kind: 'parquet', path: src.filePath };
    case 'json':
      return { kind: 'json', path: src.filePath };
    default:
      return null;
  }
}

/** Snapshot congelado das linhas atuais da tabela (fonte `inline`). */
export function inlineEtlSource(table?: Pick<Table, 'columns' | 'rows'>): EtlSource {
  const columns = (table?.columns || []).map(c => c.name);
  const rows: Scalar[][] = (table?.rows || []).map(row =>
    columns.map(c => {
      const v = (row as Record<string, unknown>)[c];
      if (v === undefined || v === null) return null;
      if (typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') return v;
      return v instanceof Date ? v.toISOString() : String(v);
    })
  );
  return { kind: 'inline', columns, rows };
}

/**
 * Fonte bruta de uma consulta nova. Arquivos são relidos do disco; tabelas sem
 * fonte recarregável (exemplo, manual, REST, banco, .pbip sem arquivo) viram
 * um snapshot `inline` das linhas atuais. Nunca uma fonte `table` apontando
 * para a própria tabela: `etl_apply` a sobrescreveria e cada reaplicação
 * partiria do resultado anterior (de volta ao modo destrutivo).
 */
export function tableToEtlSource(table?: Pick<Table, 'columns' | 'rows' | 'sourceConfig'>): EtlSource {
  return fileEtlSource(table) ?? inlineEtlSource(table);
}
