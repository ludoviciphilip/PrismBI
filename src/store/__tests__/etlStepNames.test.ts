import { describe, it, expect, beforeEach } from 'vitest';
import { useBiStore, uniqueStepName } from '../useBiStore';

describe('step names are unique (they are M identifiers)', () => {
  beforeEach(() => {
    useBiStore.setState({
      appliedSteps: { t: { name: 'T', source: { kind: 'inline', columns: ['A'], rows: [] }, steps: [] } },
      selectedStepByTable: {},
    } as any);
  });

  it('uniqueStepName appends 1, 2… like Power BI', () => {
    expect(uniqueStepName('Linhas Filtradas', [])).toBe('Linhas Filtradas');
    expect(uniqueStepName('Linhas Filtradas', ['Linhas Filtradas'])).toBe('Linhas Filtradas1');
    expect(uniqueStepName('Linhas Filtradas', ['linhas filtradas', 'Linhas Filtradas1'])).toBe('Linhas Filtradas2');
  });

  it('adding the same step twice does not create a self-referencing name', () => {
    const add = useBiStore.getState().addQueryStep;
    const cond = { column: 'A', op: 'eq', value: 'x' } as const;
    add('t', { id: 'a', name: 'Linhas Filtradas', op: 'filterRows', condition: cond } as any);
    add('t', { id: 'b', name: 'Linhas Filtradas', op: 'filterRows', condition: cond } as any);
    expect(useBiStore.getState().appliedSteps.t.steps.map(s => s.name)).toEqual(['Linhas Filtradas', 'Linhas Filtradas1']);
  });

  it('renaming to an existing name is made unique', () => {
    const { addQueryStep, renameQueryStep } = useBiStore.getState();
    addQueryStep('t', { id: 'a', name: 'X', op: 'removeBlankRows' } as any);
    addQueryStep('t', { id: 'b', name: 'Y', op: 'removeBlankRows' } as any);
    renameQueryStep('t', 'b', 'X');
    expect(useBiStore.getState().appliedSteps.t.steps.map(s => s.name)).toEqual(['X', 'X1']);
  });
});
