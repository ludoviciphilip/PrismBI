// @vitest-environment node
import { describe, it, expect } from 'vitest';
import { uniqueStepName, uniqueColumnName, dedupeImportedSteps } from '../stepNaming';
import { EtlStep } from '../../../types/etl';

describe('uniqueColumnName', () => {
  it('leaves a column name alone when not taken', () => {
    expect(uniqueColumnName('Mesclado', ['Ano', 'Mês'])).toBe('Mesclado');
  });

  it('appends .1, .2… like Power BI (dot suffix, distinct from step-name suffix)', () => {
    expect(uniqueColumnName('Mesclado', ['Mesclado'])).toBe('Mesclado.1');
    expect(uniqueColumnName('Mesclado', ['mesclado', 'Mesclado.1'])).toBe('Mesclado.2');
  });
});

describe('dedupeImportedSteps — reproduces the Observação bug from M import', () => {
  // Reproduz o cenário do print: duas etapas "Mesclado Adicionado" (addCustomColumn),
  // a segunda feita com nome repetido e coluna "Mesclado" repetida, como viria
  // de um M de .pbip onde o usuário mesclou duas vezes sem perceber a colisão.
  const baseColumns = ['Data', 'Valor', 'Ano', 'Nome do Mês'];

  function mkSteps(): EtlStep[] {
    return [
      {
        id: 's1',
        name: 'Mesclado Adicionado',
        op: 'addCustomColumn',
        newColumn: 'Mesclado',
        expression: 'Text.Combine({Text.From([Ano]), [Nome do Mês]}, "/")',
        type: 'text',
      },
      {
        id: 's2',
        name: 'Mesclado Adicionado',
        op: 'addCustomColumn',
        newColumn: 'Mesclado',
        expression: 'Text.Combine({[Nome do Mês], Text.From([Ano])}, "-")',
        type: 'text',
      },
    ];
  }

  it('RED: without dedup, two steps keep the identical name (self-reference bug)', () => {
    const steps = mkSteps();
    expect(steps[0].name).toBe(steps[1].name);
    expect((steps[0] as any).newColumn).toBe((steps[1] as any).newColumn);
  });

  it('gives the second step and its column a unique, Power-BI-style suffix', () => {
    const deduped = dedupeImportedSteps(mkSteps(), baseColumns);
    expect(deduped[0].name).toBe('Mesclado Adicionado');
    expect(deduped[1].name).toBe('Mesclado Adicionado1');
    expect((deduped[0] as any).newColumn).toBe('Mesclado');
    expect((deduped[1] as any).newColumn).toBe('Mesclado.1');
    // A segunda etapa agora referencia a PRIMEIRA (nome distinto), nunca a si mesma.
    expect(deduped[1].name).not.toBe(deduped[0].name);
  });

  it('is a no-op when names are already unique', () => {
    const steps: EtlStep[] = [
      { id: 's1', name: 'Linhas Filtradas', op: 'filterRows', condition: { column: 'Valor', op: 'gt', value: 0 } },
      { id: 's2', name: 'Tipo Alterado', op: 'changeType', columns: [{ column: 'Ano', type: 'integer' }] },
    ];
    const deduped = dedupeImportedSteps(steps, ['Ano', 'Valor']);
    expect(deduped).toEqual(steps);
  });

  it('three merge attempts in a row get 0/1/2-style suffixes for both step and column', () => {
    const steps: EtlStep[] = [0, 1, 2].map(i => ({
      id: `s${i}`,
      name: 'Mesclado Adicionado',
      op: 'addCustomColumn' as const,
      newColumn: 'Mesclado',
      expression: `Text.Combine({[A], [B]}, "${i}")`,
      type: 'text' as const,
    }));
    const deduped = dedupeImportedSteps(steps, ['A', 'B']);
    expect(deduped.map(s => s.name)).toEqual(['Mesclado Adicionado', 'Mesclado Adicionado1', 'Mesclado Adicionado2']);
    expect(deduped.map(s => (s as any).newColumn)).toEqual(['Mesclado', 'Mesclado.1', 'Mesclado.2']);
  });
});

describe('uniqueStepName (unchanged contract, re-exported from this module)', () => {
  it('appends 1, 2… like Power BI', () => {
    expect(uniqueStepName('Linhas Filtradas', [])).toBe('Linhas Filtradas');
    expect(uniqueStepName('Linhas Filtradas', ['Linhas Filtradas'])).toBe('Linhas Filtradas1');
  });
});
