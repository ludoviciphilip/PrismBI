import { describe, it, expect } from 'vitest';
import { planClearSort } from '../sortUtils';
import { EtlStep } from '../../../types/etl';

describe('planClearSort ("Limpar Classificação")', () => {
  it('removes the column from the last sort step when there are other keys', () => {
    const steps: EtlStep[] = [{ id: 's1', name: 'Classificado', op: 'sort', by: [{ column: 'A', descending: false }, { column: 'B', descending: true }] }];
    const plan = planClearSort(steps, 'A');
    expect(plan).toEqual({ stepId: 's1', action: 'update', by: [{ column: 'B', descending: true }] });
  });

  it('deletes the step entirely when it was the only sort key', () => {
    const steps: EtlStep[] = [{ id: 's1', name: 'Classificado', op: 'sort', by: [{ column: 'A', descending: false }] }];
    const plan = planClearSort(steps, 'A');
    expect(plan).toEqual({ stepId: 's1', action: 'remove' });
  });

  it('only touches the most recent sort step, not earlier ones', () => {
    const steps: EtlStep[] = [
      { id: 's1', name: 'Classificado', op: 'sort', by: [{ column: 'A', descending: false }] },
      { id: 's2', name: 'Alterado', op: 'renameColumns', renames: [['A', 'A2']] },
      { id: 's3', name: 'Classificado (2)', op: 'sort', by: [{ column: 'A2', descending: true }] },
    ];
    const plan = planClearSort(steps, 'A2');
    expect(plan).toEqual({ stepId: 's3', action: 'remove' });
  });

  it('returns null when there is no sort step involving the column', () => {
    const steps: EtlStep[] = [{ id: 's1', name: 'Classificado', op: 'sort', by: [{ column: 'B', descending: false }] }];
    expect(planClearSort(steps, 'A')).toBeNull();
  });

  it('returns null when there is no sort step at all', () => {
    expect(planClearSort([], 'A')).toBeNull();
  });
});
