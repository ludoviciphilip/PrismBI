import { describe, it, expect } from 'vitest';
import {
  TEXT_EXTRACT_OPS,
  NUMBER_STANDARD_OPS,
  NUMBER_STANDARD_COMBINE_OPS,
  NUMBER_SCIENTIFIC_OPS,
  NUMBER_ROUNDING_OPS,
  NUMBER_INFO_OPS,
  DATE_OP_FLAT,
  buildTransformColumnsStep,
  buildAddCustomColumnStep,
  buildCombineStep,
} from '../opDefs';

describe('buildTransformColumnsStep / buildAddCustomColumnStep (table-driven)', () => {
  // Cada OpDef deve virar exatamente o step do contrato (docs/ETL_V09.md §Rodada 2).
  const cases: { label: string; id: string; inplaceExpr: string; newExpr: string; type: string }[] = [
    { label: 'Comprimento', id: 'length', inplaceExpr: 'Text.Length(_)', newExpr: 'Text.Length([Nome])', type: 'integer' },
    { label: 'Ano', id: 'year', inplaceExpr: 'Date.Year(_)', newExpr: 'Date.Year([Nome])', type: 'integer' },
    { label: 'Nome do Mês', id: 'monthName', inplaceExpr: 'Date.MonthName(_, "pt-BR")', newExpr: 'Date.MonthName([Nome], "pt-BR")', type: 'text' },
    { label: 'Idade', id: 'age', inplaceExpr: 'Duration.Days(DateTime.Date(DateTime.LocalNow()) - _)', newExpr: 'Duration.Days(DateTime.Date(DateTime.LocalNow()) - [Nome])', type: 'integer' },
  ];

  const allOps = [...TEXT_EXTRACT_OPS, ...DATE_OP_FLAT];

  it.each(cases)('$label builds the documented expression for both inplace and new-column modes', ({ id, inplaceExpr, newExpr, type }) => {
    const op = allOps.find(o => o.id === id)!;
    expect(op).toBeTruthy();

    const inplace = buildTransformColumnsStep(op, ['Nome']);
    expect(inplace).toMatchObject({ op: 'transformColumns', columns: ['Nome'], expression: inplaceExpr, type });

    const added = buildAddCustomColumnStep(op, 'Nome');
    expect(added).toMatchObject({ op: 'addCustomColumn', expression: newExpr, type });
    expect((added as any).newColumn).toBeTruthy();
  });

  it('passes collected params into the expression (e.g. Primeiros Caracteres)', () => {
    const op = TEXT_EXTRACT_OPS.find(o => o.id === 'firstChars')!;
    const step = buildAddCustomColumnStep(op, 'Nome', { count: 5 });
    expect(step).toMatchObject({ op: 'addCustomColumn', newColumn: 'Primeiros Caracteres', expression: 'Text.Start([Nome], 5)', type: 'text' });
  });

  it.each(NUMBER_STANDARD_OPS)('standard number op "$label" applies a value against a single column', op => {
    const step = buildTransformColumnsStep(op, ['Valor'], { value: 2 });
    expect(step.op).toBe('transformColumns');
    expect((step as any).columns).toEqual(['Valor']);
    expect((step as any).expression).toContain('_');
  });

  it.each(Object.values(NUMBER_STANDARD_COMBINE_OPS))('combine op "$label" references both selected columns by name', op => {
    const step = buildCombineStep(op, 'A', 'B');
    expect(step.op).toBe('addCustomColumn');
    expect((step as any).expression).toContain('[A]');
    expect((step as any).expression).toContain('[B]');
  });

  it.each(NUMBER_SCIENTIFIC_OPS.flatMap(g => g.ops))('scientific op "$label" produces a valid decimal transformColumns step', op => {
    const step = buildTransformColumnsStep(op, ['Valor']);
    expect(step).toMatchObject({ op: 'transformColumns', columns: ['Valor'], type: 'decimal' });
  });

  it.each(NUMBER_ROUNDING_OPS)('rounding op "$label" produces a decimal step', op => {
    const step = buildTransformColumnsStep(op, ['Valor'], { digits: 2 });
    expect(step).toMatchObject({ op: 'transformColumns', columns: ['Valor'], type: 'decimal' });
  });

  it.each(NUMBER_INFO_OPS)('information op "$label" produces the documented step', op => {
    const step = buildTransformColumnsStep(op, ['Valor']);
    expect(step.op).toBe('transformColumns');
    expect(['boolean', 'integer']).toContain((step as any).type);
  });
});
