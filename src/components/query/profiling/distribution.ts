// Distribuição da coluna: top valores (mini histograma) + contagem de
// distintos/únicos, sobre as primeiras PROFILE_SAMPLE_SIZE linhas.

import { PROFILE_SAMPLE_SIZE } from './quality';

export interface DistributionBucket {
  value: string;
  count: number;
  pct: number;
}

export interface ColumnDistribution {
  buckets: DistributionBucket[];
  distinctCount: number;
  uniqueCount: number;
  sampleSize: number;
}

function round1(n: number): number {
  return Math.round(n * 10) / 10;
}

export function computeColumnDistribution(rows: Record<string, any>[], column: string, topN = 10): ColumnDistribution {
  const sample = rows.slice(0, PROFILE_SAMPLE_SIZE);
  const counts = new Map<string, number>();
  for (const r of sample) {
    const v = r[column];
    const key = v === null || v === undefined || v === '' ? '(vazio)' : String(v);
    counts.set(key, (counts.get(key) || 0) + 1);
  }
  let uniqueCount = 0;
  counts.forEach(c => {
    if (c === 1) uniqueCount += 1;
  });
  const total = sample.length;
  const buckets = Array.from(counts.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, topN)
    .map(([value, count]) => ({ value, count, pct: total ? round1((count / total) * 100) : 0 }));
  return { buckets, distinctCount: counts.size, uniqueCount, sampleSize: total };
}
