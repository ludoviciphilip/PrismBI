import { describe, it, expect, beforeEach, vi } from 'vitest';

// Bug do teste no desktop: a tabela "Medidas" (só hospeda medidas DAX; a
// consulta termina sem colunas) ficava eternamente "pendente de aplicação".
const apply = vi.fn();
vi.mock('../../services/etl/etlClient', async importOriginal => {
  const actual = await importOriginal<typeof import('../../services/etl/etlClient')>();
  return { ...actual, etlApply: (...a: unknown[]) => apply(...a) };
});

const { useBiStore } = await import('../useBiStore');

describe('measures host table (query without columns)', () => {
  beforeEach(() => {
    apply.mockReset();
    apply.mockResolvedValue(null); // etl_apply: a consulta não produz colunas
    useBiStore.setState({
      tables: [{ id: 't_med', name: 'Medidas', originalName: 'Medidas', columns: [], rows: [] }] as any,
      measures: [{ id: 'm1', name: 'Faturamento', tableName: 'Medidas', formula: 'SUM(f[Valor])' }] as any,
      appliedSteps: {
        t_med: {
          name: 'Medidas',
          source: { kind: 'inline', columns: ['Column1'], rows: [['Medidas=']] },
          steps: [
            { id: 's1', name: 'Removed Bottom Rows', op: 'removeBottomRows', count: 1 },
            { id: 's2', name: 'Removed Columns', op: 'removeColumns', columns: ['Column1'] },
          ],
        },
      },
      appliedQuerySnapshot: {},
    } as any);
  });

  it('applies cleanly and is no longer pending', async () => {
    expect(useBiStore.getState().hasPendingQueryChanges()).toBe(true);
    const res = await useBiStore.getState().applyAllDirtyQueries();
    expect(res.errors).toBeNull();
    expect(res.applied).toEqual([expect.objectContaining({ name: 'Medidas', measuresOnly: true })]);
    expect(useBiStore.getState().hasPendingQueryChanges()).toBe(false);
    // A tabela e as medidas continuam lá.
    expect(useBiStore.getState().tables.map(t => t.name)).toEqual(['Medidas']);
    expect(useBiStore.getState().measures.map(m => m.name)).toEqual(['Faturamento']);
  });
});
