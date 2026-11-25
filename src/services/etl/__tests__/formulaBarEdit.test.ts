// @vitest-environment node
import { describe, it, expect, vi, beforeEach } from 'vitest';

const fullM = `let
    Fonte = Csv.Document(File.Contents("vendas.csv")),
    #"Tipo Alterado" = Table.TransformColumnTypes(Fonte, {{"Valor", type number}}),
    #"Linhas Filtradas" = Table.SelectRows(#"Tipo Alterado", each [Valor] > 0)
in
    #"Linhas Filtradas"`;

describe('spliceStepExpression', () => {
  it('replaces only the targeted step line, keeping the rest of the M untouched', async () => {
    const { spliceStepExpression } = await import('../formulaBarEdit');
    const out = spliceStepExpression(fullM, 'Tipo Alterado', 'Table.TransformColumnTypes(Fonte, {{"Valor", type text}})');
    expect(out).toContain('#"Tipo Alterado" = Table.TransformColumnTypes(Fonte, {{"Valor", type text}}),');
    // Passo anterior e posterior inalterados
    expect(out).toContain('Fonte = Csv.Document(File.Contents("vendas.csv")),');
    expect(out).toContain('#"Linhas Filtradas" = Table.SelectRows(#"Tipo Alterado", each [Valor] > 0)');
  });

  it('accepts a leading "=" as typed in the formula bar', async () => {
    const { spliceStepExpression } = await import('../formulaBarEdit');
    const out = spliceStepExpression(fullM, 'Linhas Filtradas', '= Table.SelectRows(#"Tipo Alterado", each [Valor] > 100)');
    expect(out).toContain('#"Linhas Filtradas" = Table.SelectRows(#"Tipo Alterado", each [Valor] > 100)');
  });

  it('throws when the step name is not present in the M', async () => {
    const { spliceStepExpression, FormulaBarEditError } = await import('../formulaBarEdit');
    expect(() => spliceStepExpression(fullM, 'Não Existe', 'foo')).toThrow(FormulaBarEditError);
  });
});

describe('editStepFormula', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it('splices the full M, reparses it, and replaces only the edited step by index', async () => {
    vi.doMock('../etlClient', () => ({
      etlToM: vi.fn(async () => fullM),
      etlFromM: vi.fn(async (_args: any) => ({
        query: {
          name: 'Vendas',
          source: { kind: 'csv', path: 'vendas.csv' },
          steps: [
            { id: 'new-0', name: 'Tipo Alterado', op: 'changeType', columns: [{ column: 'Valor', type: 'text' }] },
            { id: 'new-1', name: 'Linhas Filtradas', op: 'filterRows', condition: { column: 'Valor', op: 'gt', value: 0 } },
          ],
        },
        warnings: [],
      })),
    }));
    const { editStepFormula } = await import('../formulaBarEdit');

    const query = {
      name: 'Vendas',
      source: { kind: 'csv' as const, path: 'vendas.csv' },
      steps: [
        { id: 'orig-0', name: 'Tipo Alterado', op: 'changeType' as const, columns: [{ column: 'Valor', type: 'decimal' as const }] },
        { id: 'orig-1', name: 'Linhas Filtradas', op: 'filterRows' as const, condition: { column: 'Valor' as const, op: 'gt' as const, value: 0 } },
      ],
    };

    const result = await editStepFormula(query, 0, 'Table.TransformColumnTypes(Fonte, {{"Valor", type text}})');

    expect(result.query.steps).toHaveLength(2);
    expect(result.query.steps[0]).toEqual({ id: 'orig-0', name: 'Tipo Alterado', op: 'changeType', columns: [{ column: 'Valor', type: 'text' }] });
    // Passo não editado preserva id e conteúdo originais
    expect(result.query.steps[1]).toEqual(query.steps[1]);
  });

  it('rejects when etl_from_m fails and does not touch the original query', async () => {
    vi.doMock('../etlClient', () => ({
      etlToM: vi.fn(async () => fullM),
      etlFromM: vi.fn(async () => {
        throw new Error('M inválido');
      }),
    }));
    const { editStepFormula, FormulaBarEditError } = await import('../formulaBarEdit');

    const query = {
      name: 'Vendas',
      source: { kind: 'csv' as const, path: 'vendas.csv' },
      steps: [{ id: 'orig-0', name: 'Tipo Alterado', op: 'changeType' as const, columns: [{ column: 'Valor', type: 'decimal' as const }] }],
    };

    await expect(editStepFormula(query, 0, 'isto não é M válido(')).rejects.toThrow(FormulaBarEditError);
  });
});
