// Qualidade da coluna: % Válido / Erro / Vazio, calculado só sobre as primeiras
// PROFILE_SAMPLE_SIZE linhas do preview (como o Power Query: "criando perfil com
// base nas primeiras N linhas").

export const PROFILE_SAMPLE_SIZE = 200;

export interface ColumnQuality {
  total: number;
  valid: number;
  error: number;
  empty: number;
  validPct: number;
  errorPct: number;
  emptyPct: number;
}

function round1(n: number): number {
  return Math.round(n * 10) / 10;
}

export function computeColumnQuality(rows: Record<string, any>[], column: string): ColumnQuality {
  const sample = rows.slice(0, PROFILE_SAMPLE_SIZE);
  const total = sample.length;
  let error = 0;
  let empty = 0;
  for (const r of sample) {
    if (r[`__err__${column}`]) {
      error += 1;
      continue;
    }
    const v = r[column];
    if (v === null || v === undefined || v === '') {
      empty += 1;
    }
  }
  const valid = total - error - empty;
  const pct = (n: number) => (total ? round1((n / total) * 100) : 0);
  return { total, valid, error, empty, validPct: pct(valid), errorPct: pct(error), emptyPct: pct(empty) };
}
