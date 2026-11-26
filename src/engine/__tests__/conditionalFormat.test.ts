import { describe, expect, it } from 'vitest';
import {
  computeCellFormat,
  computeDataBarMetrics,
  computeFormatForValue,
  conditionalFormatsFromPbiMeta,
  fieldValueColor,
  firstMatchingRule,
  gradientColor,
  interpolateColor,
  isBlank,
  percentile,
  resolveStopValue,
} from '../conditionalFormat';
import type { ConditionalFormat } from '../../types/bi';

describe('isBlank', () => {
  it('trata null, undefined, string vazia e NaN como BLANK', () => {
    expect(isBlank(null)).toBe(true);
    expect(isBlank(undefined)).toBe(true);
    expect(isBlank('')).toBe(true);
    expect(isBlank('   ')).toBe(true);
    expect(isBlank(NaN)).toBe(true);
  });
  it('não trata 0, false ou texto não vazio como BLANK', () => {
    expect(isBlank(0)).toBe(false);
    expect(isBlank(false)).toBe(false);
    expect(isBlank('0')).toBe(false);
    expect(isBlank('abc')).toBe(false);
  });
});

describe('percentile (interpolação linear)', () => {
  // [10,20,30,40,50], rank = p/100 * 4
  const values = [50, 10, 30, 20, 40]; // fora de ordem de propósito
  it('p=0 -> mínimo, p=100 -> máximo', () => {
    expect(percentile(values, 0)).toBe(10);
    expect(percentile(values, 100)).toBe(50);
  });
  it('p=50 -> mediana exata (índice 2)', () => {
    expect(percentile(values, 50)).toBe(30);
  });
  it('p=25 -> interpola entre índice 1 (20) e 2 (30): rank=1 -> 20', () => {
    // rank = 0.25*4 = 1 -> exatamente o índice 1
    expect(percentile(values, 25)).toBe(20);
  });
  it('p=10 -> rank=0.4 -> interpola entre 10 e 20: 10 + 0.4*10 = 14', () => {
    expect(percentile(values, 10)).toBeCloseTo(14, 5);
  });
  it('lista vazia -> 0; lista de 1 elemento -> esse elemento', () => {
    expect(percentile([], 50)).toBe(0);
    expect(percentile([7], 0)).toBe(7);
    expect(percentile([7], 100)).toBe(7);
  });
});

describe('resolveStopValue', () => {
  const visible = [5, 1, 9, 3];
  it('lowest/highest usam min/max dos valores visíveis', () => {
    expect(resolveStopValue({ kind: 'lowest', color: '#000' }, visible)).toBe(1);
    expect(resolveStopValue({ kind: 'highest', color: '#000' }, visible)).toBe(9);
  });
  it('number usa o valor fixo', () => {
    expect(resolveStopValue({ kind: 'number', value: 42, color: '#000' }, visible)).toBe(42);
  });
  it('percent usa o percentil dos valores visíveis', () => {
    // sorted [1,3,5,9], p=50 -> rank=1.5 -> 3 + 0.5*(5-3) = 4
    expect(resolveStopValue({ kind: 'percent', value: 50, color: '#000' }, visible)).toBeCloseTo(4, 5);
  });
});

describe('interpolateColor', () => {
  it('t=0 -> cor A; t=1 -> cor B', () => {
    expect(interpolateColor('#000000', '#ffffff', 0)).toBe('#000000');
    expect(interpolateColor('#000000', '#ffffff', 1)).toBe('#ffffff');
  });
  it('t=0.5 entre preto e branco -> cinza médio (#808080, arredondado de 127.5)', () => {
    expect(interpolateColor('#000000', '#ffffff', 0.5)).toBe('#808080');
  });
  it('interpola canais distintos: #ff0000 -> #0000ff em t=0.5 -> #800080', () => {
    expect(interpolateColor('#ff0000', '#0000ff', 0.5)).toBe('#800080');
  });
  it('aceita hex de 3 dígitos', () => {
    expect(interpolateColor('#f00', '#00f', 0)).toBe('#ff0000');
  });
  it('clampa t fora de [0,1]', () => {
    expect(interpolateColor('#000000', '#ffffff', -1)).toBe('#000000');
    expect(interpolateColor('#000000', '#ffffff', 2)).toBe('#ffffff');
  });
});

describe('gradientColor', () => {
  const visible = [0, 50, 100];
  it('2 cores: valor no meio do min/max -> 50% de interpolação', () => {
    const g = { min: { kind: 'lowest' as const, color: '#000000' }, max: { kind: 'highest' as const, color: '#ffffff' } };
    expect(gradientColor(g, 50, visible)).toBe('#808080');
    expect(gradientColor(g, 0, visible)).toBe('#000000');
    expect(gradientColor(g, 100, visible)).toBe('#ffffff');
  });
  it('3 cores: valor abaixo do meio interpola min->mid; acima interpola mid->max', () => {
    const g = {
      min: { kind: 'lowest' as const, color: '#ff0000' },
      mid: { kind: 'number' as const, value: 50, color: '#ffff00' },
      max: { kind: 'highest' as const, color: '#00ff00' },
    };
    // valor 25: entre min(0) e mid(50) -> t=0.5 -> interpola #ff0000/#ffff00 -> #ff8000
    expect(gradientColor(g, 25, visible)).toBe('#ff8000');
    // valor 50 exatamente no mid -> cor do mid
    expect(gradientColor(g, 50, visible)).toBe('#ffff00');
    // valor 75: entre mid(50) e max(100) -> t=0.5 -> #ffff00/#00ff00 -> #80ff00
    expect(gradientColor(g, 75, visible)).toBe('#80ff00');
  });
  it('min==max (range degenerado) -> cor do min', () => {
    const g = { min: { kind: 'number' as const, value: 10, color: '#111111' }, max: { kind: 'number' as const, value: 10, color: '#222222' } };
    expect(gradientColor(g, 10, [10])).toBe('#111111');
  });
  it('stops por percentual: min=P10, max=P90 dos visíveis', () => {
    const vals = Array.from({ length: 11 }, (_, i) => i * 10); // 0..100
    const g = { min: { kind: 'percent' as const, value: 10, color: '#000000' }, max: { kind: 'percent' as const, value: 90, color: '#ffffff' } };
    // P10 de 0..100 com 11 pontos: rank = 0.1*10=1 -> 10 ; P90: rank=9 -> 90
    // valor 50 -> t = (50-10)/(90-10) = 0.5 -> cinza médio
    expect(gradientColor(g, 50, vals)).toBe('#808080');
  });
});

describe('fieldValueColor', () => {
  it('hex literal passa direto', () => {
    expect(fieldValueColor('#ff00aa')).toBe('#ff00aa');
    expect(fieldValueColor('#f0a')).toBe('#f0a');
  });
  it('nome CSS passa como está', () => {
    expect(fieldValueColor('steelblue')).toBe('steelblue');
  });
  it('BLANK -> undefined', () => {
    expect(fieldValueColor(null)).toBeUndefined();
    expect(fieldValueColor('')).toBeUndefined();
  });
});

describe('firstMatchingRule (ordem Power BI: primeira que casa vence)', () => {
  const rules = [
    { operator: '>' as const, value: 100, color: '#111111' },
    { operator: '>' as const, value: 50, color: '#222222' },
    { operator: '>' as const, value: 0, color: '#333333' },
  ];
  it('valor 150 casa com a primeira regra (>100)', () => {
    expect(firstMatchingRule(rules, 150, [])?.rule.color).toBe('#111111');
  });
  it('valor 75 pula a primeira e casa na segunda (>50)', () => {
    expect(firstMatchingRule(rules, 75, [])?.rule.color).toBe('#222222');
  });
  it('valor 10 só casa na terceira (>0)', () => {
    expect(firstMatchingRule(rules, 10, [])?.rule.color).toBe('#333333');
  });
  it('valor -5 não casa em nenhuma', () => {
    expect(firstMatchingRule(rules, -5, [])).toBeNull();
  });
  it('operador between com value2', () => {
    const r = [{ operator: 'between' as const, value: 10, value2: 20, color: '#abc' }];
    expect(firstMatchingRule(r, 15, [])?.rule.color).toBe('#abc');
    expect(firstMatchingRule(r, 25, [])).toBeNull();
    // ordem invertida (value > value2) ainda funciona (min/max internos)
    const r2 = [{ operator: 'between' as const, value: 20, value2: 10, color: '#abc' }];
    expect(firstMatchingRule(r2, 15, [])?.rule.color).toBe('#abc');
  });
  it('valueKind percent: limiar é percentil dos valores visíveis', () => {
    const visible = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    // regra ">= P90" -> limiar = percentile(visible,90) = 90
    const r = [{ operator: '>=' as const, value: 90, color: '#top', valueKind: 'percent' as const }];
    expect(firstMatchingRule(r, 95, visible)?.rule.color).toBe('#top');
    expect(firstMatchingRule(r, 85, visible)).toBeNull();
  });
  it('operadores == e <=/>=', () => {
    expect(firstMatchingRule([{ operator: '==' as const, value: 7, color: '#x' }], 7, [])).not.toBeNull();
    expect(firstMatchingRule([{ operator: '==' as const, value: 7, color: '#x' }], 7.0001, [])).toBeNull();
    expect(firstMatchingRule([{ operator: '<=' as const, value: 7, color: '#x' }], 7, [])).not.toBeNull();
    expect(firstMatchingRule([{ operator: '>=' as const, value: 7, color: '#x' }], 7, [])).not.toBeNull();
  });
});

describe('computeDataBarMetrics', () => {
  it('somente valores positivos: eixo em 0%, barra cresce da esquerda', () => {
    const m = computeDataBarMetrics(50, [0, 50, 100], { positiveColor: '#0f0', negativeColor: '#f00' });
    expect(m.axisPercent).toBe(0);
    expect(m.isNegative).toBe(false);
    expect(m.startPercent).toBe(0);
    expect(m.widthPercent).toBeCloseTo(50, 5); // 50/100 * 100%
    expect(m.color).toBe('#0f0');
  });
  it('faixa com negativos e positivos: eixo no ponto proporcional a |min|/(max-min)', () => {
    // min=-50, max=100, range=150 -> axis = 50/150*100 = 33.33%
    const visible = [-50, 0, 100];
    const pos = computeDataBarMetrics(100, visible, { positiveColor: '#0f0', negativeColor: '#f00' });
    expect(pos.axisPercent).toBeCloseTo(33.333, 2);
    expect(pos.isNegative).toBe(false);
    expect(pos.startPercent).toBeCloseTo(33.333, 2);
    // largura positiva: value/posMax * (100-axis) = 100/100 * (100-33.33) = 66.67
    expect(pos.widthPercent).toBeCloseTo(66.667, 2);

    const neg = computeDataBarMetrics(-50, visible, { positiveColor: '#0f0', negativeColor: '#f00' });
    expect(neg.isNegative).toBe(true);
    // largura negativa: |value|/|min| * axis = 50/50 * 33.33 = 33.33 (barra inteira para a esquerda do eixo)
    expect(neg.widthPercent).toBeCloseTo(33.333, 2);
    expect(neg.startPercent).toBeCloseTo(0, 2);
    expect(neg.color).toBe('#f00');
  });
  it('range degenerado (todos os valores iguais) -> largura 0', () => {
    const m = computeDataBarMetrics(5, [5, 5, 5], { positiveColor: '#0f0', negativeColor: '#f00' });
    expect(m.widthPercent).toBe(0);
  });
});

describe('computeFormatForValue', () => {
  const visible = [0, 50, 100];

  it('BLANK nunca recebe formato', () => {
    const fmt: ConditionalFormat = { id: '1', target: 'background', applyTo: 'Vendas', mode: 'rules', rules: [{ operator: '>', value: 0, color: '#f00' }] };
    expect(computeFormatForValue(fmt, null, visible)).toBeNull();
    expect(computeFormatForValue(fmt, undefined, visible)).toBeNull();
    expect(computeFormatForValue(fmt, '', visible)).toBeNull();
  });

  it('mode=rules + target=background', () => {
    const fmt: ConditionalFormat = {
      id: '1', target: 'background', applyTo: 'Vendas', mode: 'rules',
      rules: [{ operator: '>=', value: 80, color: '#16a34a' }, { operator: '>=', value: 0, color: '#dc2626' }],
    };
    expect(computeFormatForValue(fmt, 90, visible)).toEqual({ background: '#16a34a' });
    expect(computeFormatForValue(fmt, 10, visible)).toEqual({ background: '#dc2626' });
  });

  it('mode=rules + target=fontColor', () => {
    const fmt: ConditionalFormat = {
      id: '1', target: 'fontColor', applyTo: 'Vendas', mode: 'rules',
      rules: [{ operator: '<', value: 0, color: '#dc2626' }],
    };
    expect(computeFormatForValue(fmt, -5, visible)).toEqual({ color: '#dc2626' });
    expect(computeFormatForValue(fmt, 5, visible)).toBeNull();
  });

  it('mode=gradient + target=background', () => {
    const fmt: ConditionalFormat = {
      id: '1', target: 'background', applyTo: 'Vendas', mode: 'gradient',
      gradient: { min: { kind: 'lowest', color: '#000000' }, max: { kind: 'highest', color: '#ffffff' } },
    };
    expect(computeFormatForValue(fmt, 50, visible)).toEqual({ background: '#808080' });
  });

  it('mode=fieldValue + target=fontColor usa a cor do próprio valor', () => {
    const fmt: ConditionalFormat = { id: '1', target: 'fontColor', applyTo: 'Status', mode: 'fieldValue' };
    expect(computeFormatForValue(fmt, '#00ff00', visible)).toEqual({ color: '#00ff00' });
    expect(computeFormatForValue(fmt, 'red', visible)).toEqual({ color: 'red' });
  });

  it('target=dataBar com cores explícitas', () => {
    const fmt: ConditionalFormat = {
      id: '1', target: 'dataBar', applyTo: 'Vendas', mode: 'rules',
      dataBar: { positiveColor: '#3b82f6', negativeColor: '#ef4444' },
    };
    const r = computeFormatForValue(fmt, 100, visible);
    expect(r?.dataBar?.color).toBe('#3b82f6');
    expect(r?.dataBar?.widthPercent).toBeCloseTo(100, 5);
  });

  it('target=icon escolhe o ícone pela primeira regra que casa', () => {
    const fmt: ConditionalFormat = {
      id: '1', target: 'icon', applyTo: 'Var', mode: 'rules',
      icons: {
        set: 'arrows',
        rules: [
          { operator: '>', value: 0, color: '' },
          { operator: '==', value: 0, color: '' },
          { operator: '<', value: 0, color: '' },
        ],
      },
    };
    expect(computeFormatForValue(fmt, 10, visible)?.icon?.key).toBe('arrow-down'); // índice 0 do set 'arrows'
    expect(computeFormatForValue(fmt, 0, visible)?.icon?.key).toBe('arrow-right'); // índice 1
    // -10: falha em >0 e ==0, casa na 3a regra (<0) -> índice 2 do set
    expect(computeFormatForValue(fmt, -10, [-10, 0, 10])?.icon?.key).toBe('arrow-up');
  });

  it('target=icon: regra na 3a posição usa o 3o ícone do set, e cor custom da regra sobrescreve', () => {
    const fmt: ConditionalFormat = {
      id: '1', target: 'icon', applyTo: 'Var', mode: 'rules',
      icons: {
        set: 'traffic',
        rules: [
          { operator: '>', value: 100, color: '' },
          { operator: '>', value: 50, color: '' },
          { operator: '<=', value: 50, color: '#custom' },
        ],
      },
    };
    const r = computeFormatForValue(fmt, 10, visible);
    expect(r?.icon?.key).toBe('circle-green'); // índice 2 do set 'traffic'
    expect(r?.icon?.color).toBe('#custom');
  });

  it('target=icon sem regra casando -> null', () => {
    const fmt: ConditionalFormat = {
      id: '1', target: 'icon', applyTo: 'Var', mode: 'rules',
      icons: { set: 'flags', rules: [{ operator: '>', value: 1000, color: '' }] },
    };
    expect(computeFormatForValue(fmt, 1, visible)).toBeNull();
  });
});

describe('computeCellFormat (combinação por target, primeiro formato de cada target vence)', () => {
  it('combina background e fontColor de dois ConditionalFormat diferentes', () => {
    const formats: ConditionalFormat[] = [
      { id: 'bg', target: 'background', applyTo: 'Vendas', mode: 'rules', rules: [{ operator: '>', value: 0, color: '#eee' }] },
      { id: 'fc', target: 'fontColor', applyTo: 'Vendas', mode: 'rules', rules: [{ operator: '>', value: 0, color: '#111' }] },
      { id: 'other', target: 'background', applyTo: 'Custo', mode: 'rules', rules: [{ operator: '>', value: 0, color: '#f00' }] },
    ];
    const result = computeCellFormat(formats, 'Vendas', () => 10, () => [10]);
    expect(result).toEqual({ background: '#eee', color: '#111' });
  });

  it('quando dois formatos disputam o mesmo target, o primeiro da lista vence', () => {
    const formats: ConditionalFormat[] = [
      { id: 'first', target: 'background', applyTo: 'Vendas', mode: 'rules', rules: [{ operator: '>', value: -999, color: '#first' }] },
      { id: 'second', target: 'background', applyTo: 'Vendas', mode: 'rules', rules: [{ operator: '>', value: -999, color: '#second' }] },
    ];
    const result = computeCellFormat(formats, 'Vendas', () => 1, () => [1]);
    expect(result).toEqual({ background: '#first' });
  });

  it('basedOn: resolveValue pode usar outro campo para decidir a cor', () => {
    const formats: ConditionalFormat[] = [
      {
        id: '1', target: 'background', applyTo: 'Produto',
        basedOn: { field: 'Margem', kind: 'measure' }, mode: 'gradient',
        gradient: { min: { kind: 'lowest', color: '#dc2626' }, max: { kind: 'highest', color: '#16a34a' } },
      },
    ];
    // simula: campo Produto mas a cor decide pela Margem (valor 50 dentro de [0,100])
    const result = computeCellFormat(formats, 'Produto', () => 50, () => [0, 50, 100]);
    // dc2626 (220,38,38) -> 16a34a (22,163,74), t=0.5: (121,100.5,56) -> round -> (121,101,56) = #796538
    expect(result.background).toBe('#796538');
  });

  it('BLANK no valor base não produz formato para aquele target', () => {
    const formats: ConditionalFormat[] = [
      { id: '1', target: 'background', applyTo: 'Vendas', mode: 'rules', rules: [{ operator: '>', value: -1, color: '#eee' }] },
    ];
    const result = computeCellFormat(formats, 'Vendas', () => null, () => []);
    expect(result).toEqual({});
  });
});

describe('conditionalFormatsFromPbiMeta (importação PBIR, v0.10 Frente B)', () => {
  it('lista vazia/undefined -> []', () => {
    expect(conditionalFormatsFromPbiMeta(undefined, 'p')).toEqual([]);
    expect(conditionalFormatsFromPbiMeta([], 'p')).toEqual([]);
  });

  it('converte gradiente (min lowest, max highest)', () => {
    const out = conditionalFormatsFromPbiMeta(
      [
        {
          target: 'background',
          applyTo: 'Receita',
          mode: 'gradient',
          gradientMin: { kind: 'lowest', color: '#FF0000' },
          gradientMax: { kind: 'highest', color: '#00FF00' },
        },
      ],
      'pbi_vis_1'
    );
    expect(out).toEqual([
      {
        id: 'pbi_vis_1_0',
        target: 'background',
        applyTo: 'Receita',
        mode: 'gradient',
        gradient: {
          min: { kind: 'lowest', value: undefined, color: '#FF0000' },
          mid: undefined,
          max: { kind: 'highest', value: undefined, color: '#00FF00' },
        },
      },
    ]);
  });

  it('converte regras (rules, ordem preservada)', () => {
    const out = conditionalFormatsFromPbiMeta(
      [
        {
          target: 'fontColor',
          applyTo: 'Margem',
          mode: 'rules',
          rules: [
            { operator: '>', value: 1000, color: '#16A34A' },
            { operator: '<', value: 0, color: '#DC2626' },
          ],
        },
      ],
      'pbi_vis_1'
    );
    expect(out[0].rules).toEqual([
      { operator: '>', value: 1000, value2: undefined, color: '#16A34A' },
      { operator: '<', value: 0, value2: undefined, color: '#DC2626' },
    ]);
  });

  it('converte barras de dados', () => {
    const out = conditionalFormatsFromPbiMeta(
      [{ target: 'dataBar', applyTo: 'Saldo', mode: 'rules', dataBarPositive: '#3B82F6', dataBarNegative: '#EF4444' }],
      'pbi_vis_1'
    );
    expect(out[0]).toEqual({
      id: 'pbi_vis_1_0',
      target: 'dataBar',
      applyTo: 'Saldo',
      mode: 'rules',
      dataBar: { positiveColor: '#3B82F6', negativeColor: '#EF4444' },
    });
  });

  it('formato sem rules/gradiente/dataBar reconhecível é descartado com segurança', () => {
    const out = conditionalFormatsFromPbiMeta([{ target: 'background', applyTo: 'X', mode: 'rules' }], 'p');
    expect(out).toEqual([]);
  });
});
