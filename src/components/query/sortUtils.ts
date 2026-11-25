// "Limpar Classificação": remove a coluna da etapa de sort mais recente da
// consulta (ou apaga a etapa, se for a única chave) — lógica pura e testável.

import { EtlStep } from '../../types/etl';

export type ClearSortPlan = { stepId: string; action: 'update'; by: { column: string; descending: boolean }[] } | { stepId: string; action: 'remove' } | null;

export function planClearSort(steps: EtlStep[], column: string): ClearSortPlan {
  for (let i = steps.length - 1; i >= 0; i--) {
    const step = steps[i];
    if (step.op !== 'sort') continue;
    const by = step.by.filter(b => b.column !== column);
    if (by.length === step.by.length) return null; // última etapa de sort não envolve esta coluna
    return by.length === 0 ? { stepId: step.id, action: 'remove' } : { stepId: step.id, action: 'update', by };
  }
  return null;
}
