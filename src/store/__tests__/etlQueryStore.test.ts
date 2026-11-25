import { describe, it, expect, beforeEach } from 'vitest';
import { useBiStore } from '../useBiStore';
import { EtlStep } from '../../types/etl';

function resetTable() {
  const { addTable } = useBiStore.getState();
  addTable({
    id: 'tbl_test_etl',
    name: 'Teste',
    originalName: 'Teste',
    columns: [
      { id: 'c1', name: 'Nome', originalName: 'Nome', dataType: 'text' },
      { id: 'c2', name: 'Valor', originalName: 'Valor', dataType: 'number' },
    ],
    rows: [{ Nome: 'A', Valor: 1 }, { Nome: 'B', Valor: 2 }],
  });
}

const TABLE_ID = 'tbl_test_etl';

describe('store ETL query actions', () => {
  beforeEach(() => {
    resetTable();
    useBiStore.setState({ appliedSteps: {}, selectedStepByTable: {} });
  });

  it('addQueryStep appends without touching table rows/columns (non-destructive)', () => {
    const { addQueryStep, tables } = useBiStore.getState();
    const before = tables.find(t => t.id === TABLE_ID)!;
    const beforeRows = JSON.stringify(before.rows);
    const beforeCols = JSON.stringify(before.columns);

    const step: EtlStep = { id: 'st1', name: 'Removido', op: 'removeColumns', columns: ['Valor'] };
    addQueryStep(TABLE_ID, step);

    const after = useBiStore.getState().tables.find(t => t.id === TABLE_ID)!;
    expect(JSON.stringify(after.rows)).toBe(beforeRows);
    expect(JSON.stringify(after.columns)).toBe(beforeCols);

    const query = useBiStore.getState().appliedSteps[TABLE_ID];
    expect(query.steps).toHaveLength(1);
    expect(query.steps[0]).toEqual(step);
  });

  it('updateQueryStep patches a step by id without mutating others', () => {
    const { addQueryStep, updateQueryStep } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 's1', name: 'A', op: 'removeColumns', columns: ['X'] });
    addQueryStep(TABLE_ID, { id: 's2', name: 'B', op: 'removeColumns', columns: ['Y'] });

    updateQueryStep(TABLE_ID, 's1', { columns: ['Z'] } as any);

    const query = useBiStore.getState().appliedSteps[TABLE_ID];
    expect((query.steps[0] as any).columns).toEqual(['Z']);
    expect((query.steps[1] as any).columns).toEqual(['Y']);
  });

  it('removeQueryStep removes only the targeted step', () => {
    const { addQueryStep, removeQueryStep } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 's1', name: 'A', op: 'promoteHeaders' });
    addQueryStep(TABLE_ID, { id: 's2', name: 'B', op: 'removeBlankRows' });

    removeQueryStep(TABLE_ID, 's1');

    const query = useBiStore.getState().appliedSteps[TABLE_ID];
    expect(query.steps.map(s => s.id)).toEqual(['s2']);
  });

  it('moveQueryStepUp/Down reorder steps', () => {
    const { addQueryStep, moveQueryStepUp, moveQueryStepDown } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 's1', name: 'A', op: 'promoteHeaders' });
    addQueryStep(TABLE_ID, { id: 's2', name: 'B', op: 'removeBlankRows' });
    addQueryStep(TABLE_ID, { id: 's3', name: 'C', op: 'removeDuplicates' });

    moveQueryStepUp(TABLE_ID, 's3');
    expect(useBiStore.getState().appliedSteps[TABLE_ID].steps.map(s => s.id)).toEqual(['s1', 's3', 's2']);

    moveQueryStepDown(TABLE_ID, 's1');
    expect(useBiStore.getState().appliedSteps[TABLE_ID].steps.map(s => s.id)).toEqual(['s3', 's1', 's2']);

    // no-op at boundaries
    moveQueryStepUp(TABLE_ID, 's3');
    expect(useBiStore.getState().appliedSteps[TABLE_ID].steps.map(s => s.id)).toEqual(['s3', 's1', 's2']);
  });

  it('renameQueryStep only changes the name field', () => {
    const { addQueryStep, renameQueryStep } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 's1', name: 'Original', op: 'promoteHeaders' });
    renameQueryStep(TABLE_ID, 's1', 'Novo Nome');
    expect(useBiStore.getState().appliedSteps[TABLE_ID].steps[0]).toEqual({ id: 's1', name: 'Novo Nome', op: 'promoteHeaders' });
  });

  it('selectQueryStep tracks UI selection per table', () => {
    const { selectQueryStep } = useBiStore.getState();
    selectQueryStep(TABLE_ID, 'abc');
    expect(useBiStore.getState().selectedStepByTable[TABLE_ID]).toBe('abc');
    selectQueryStep(TABLE_ID, undefined);
    expect(useBiStore.getState().selectedStepByTable[TABLE_ID]).toBeUndefined();
  });

  it('setQuery replaces the whole query (used by the advanced editor)', () => {
    const { setQuery } = useBiStore.getState();
    setQuery(TABLE_ID, { name: 'Teste', source: { kind: 'table' }, steps: [{ id: 'x', name: 'Y', op: 'promoteHeaders' }] });
    expect(useBiStore.getState().appliedSteps[TABLE_ID].steps).toHaveLength(1);
  });

  it('persists the query via getProjectState and restores it via loadStateFromProject', () => {
    const { addQueryStep, getProjectState, loadStateFromProject } = useBiStore.getState();
    addQueryStep(TABLE_ID, { id: 's1', name: 'A', op: 'promoteHeaders' });

    const state = getProjectState();
    expect(state.appliedSteps?.[TABLE_ID].steps).toHaveLength(1);

    useBiStore.setState({ appliedSteps: {} });
    loadStateFromProject(state);
    expect(useBiStore.getState().appliedSteps[TABLE_ID]?.steps).toHaveLength(1);
  });
});
