import { describe, expect, it } from 'vitest';
import { styleForTotalCell, totalFormats } from '../apply';
import type { ConditionalFormat } from '../../../../types/bi';

describe('totalFormats / styleForTotalCell (v0.10 Frente H: totais/subtotais da matriz)', () => {
  const gradient: ConditionalFormat = {
    id: 'cf_total',
    target: 'background',
    applyTo: 'Receita',
    mode: 'gradient',
    applyToTotals: true,
    gradient: { min: { kind: 'lowest', color: '#ff0000' }, max: { kind: 'highest', color: '#00ff00' } },
  };
  const valuesOnly: ConditionalFormat = {
    id: 'cf_values',
    target: 'fontColor',
    applyTo: 'Receita',
    mode: 'rules',
    rules: [{ operator: '>', value: 0, color: '#123456' }],
    // applyToTotals ausente = "Somente valores"
  };

  it('totalFormats só devolve formatos com applyToTotals: true', () => {
    expect(totalFormats([gradient, valuesOnly])).toEqual([gradient]);
    expect(totalFormats(undefined)).toEqual([]);
  });

  it('styleForTotalCell ignora formatos "Somente valores"', () => {
    const style = styleForTotalCell([valuesOnly], 'Receita', 10, [10]);
    expect(style).toEqual({});
  });

  it('styleForTotalCell aplica o gradiente usando a escala dos TOTAIS, não a das células', () => {
    // Escala de totais: [0, 100] -> valor 100 deve bater no máximo (verde).
    const style = styleForTotalCell([gradient], 'Receita', 100, [0, 100]);
    expect(style.backgroundColor).toBe('#00ff00');
  });

  it('a mesma célula de valor, fora da escala de totais, dá uma cor diferente', () => {
    // Mesmo campo/valor, mas a escala de totais é [100, 200] -> 100 é o mínimo (vermelho).
    const style = styleForTotalCell([gradient], 'Receita', 100, [100, 200]);
    expect(style.backgroundColor).toBe('#ff0000');
  });

  it('retorna {} quando não há formatos aplicáveis a totais', () => {
    expect(styleForTotalCell(undefined, 'Receita', 10, [10])).toEqual({});
    expect(styleForTotalCell([], 'Receita', 10, [10])).toEqual({});
  });
});
