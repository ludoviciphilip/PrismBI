// Perfil da coluna ativa: estatísticas completas (painel inferior do Power Query).

import { EtlType } from '../../../types/etl';
import { computeColumnQuality } from './quality';
import { computeColumnDistribution, ColumnDistribution } from './distribution';

export const NUMERIC_TYPES: EtlType[] = ['integer', 'decimal', 'currency'];
export const DATE_TYPES: EtlType[] = ['date', 'datetime'];

export interface ColumnProfile {
  count: number;
  errors: number;
  empty: number;
  distinct: number;
  unique: number;
  min?: number;
  max?: number;
  mean?: number;
  stdDev?: number;
  emptyStrings?: number;
  minLength?: number;
  maxLength?: number;
  minDate?: string;
  maxDate?: string;
  distribution: ColumnDistribution;
}

function round2(n: number): number {
  return Math.round(n * 100) / 100;
}

export function computeColumnProfile(rows: Record<string, any>[], column: string, type: EtlType | string): ColumnProfile {
  const quality = computeColumnQuality(rows, column);
  const distribution = computeColumnDistribution(rows, column);
  const sample = rows.slice(0, 200);
  const base: ColumnProfile = {
    count: quality.total,
    errors: quality.error,
    empty: quality.empty,
    distinct: distribution.distinctCount,
    unique: distribution.uniqueCount,
    distribution,
  };

  const nonEmpty = sample.filter(r => !r[`__err__${column}`] && r[column] !== null && r[column] !== undefined && r[column] !== '');

  if (DATE_TYPES.includes(type as EtlType)) {
    // Datas/datetimes usam mín/máx de data, não comprimento de string (Power Query mostra assim).
    const timestamps = nonEmpty
      .map(r => ({ raw: r[column], t: Date.parse(String(r[column])) }))
      .filter(x => !Number.isNaN(x.t));
    if (timestamps.length) {
      const minEntry = timestamps.reduce((a, b) => (b.t < a.t ? b : a));
      const maxEntry = timestamps.reduce((a, b) => (b.t > a.t ? b : a));
      base.minDate = String(minEntry.raw);
      base.maxDate = String(maxEntry.raw);
    }
  } else if (NUMERIC_TYPES.includes(type as EtlType)) {
    const nums = nonEmpty.map(r => Number(r[column])).filter(n => !Number.isNaN(n));
    if (nums.length) {
      const min = Math.min(...nums);
      const max = Math.max(...nums);
      const mean = nums.reduce((a, b) => a + b, 0) / nums.length;
      const variance = nums.reduce((a, b) => a + (b - mean) ** 2, 0) / nums.length;
      base.min = min;
      base.max = max;
      base.mean = round2(mean);
      base.stdDev = round2(Math.sqrt(variance));
    }
  } else {
    const strs = nonEmpty.map(r => String(r[column]));
    base.emptyStrings = sample.filter(r => r[column] === '').length;
    if (strs.length) {
      const lens = strs.map(s => s.length);
      base.minLength = Math.min(...lens);
      base.maxLength = Math.max(...lens);
    }
  }
  return base;
}
