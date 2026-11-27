import { describe, it, expect } from 'vitest';
import {
  DRILL_ALL,
  isDrillAll,
  resolveDrillLevels,
  resolveMatrixRowLevels,
  getDrillState,
  canDrillUp,
  canDrillDown,
  drillUpPatch,
  drillDownOnPointPatch,
  goToNextLevelPatch,
  expandAllLevelsPatch,
  groupingLevels,
  buildDrillPathFilters,
  datePartValue,
  formatDatePartLabel,
  levelValueLabel,
  concatLevelLabels,
  isMatrixPathExpanded,
  matrixPathId,
  expandOneLevel,
  toggleMatrixPath,
  MatrixExpansion,
  buildHoverFilters,
  buildPathFilters,
  describeDrillFilterSpec,
  describeDrillFilterSpecs,
} from '../drill';
import { DataType, VisualDataConfig } from '../../types/bi';

const types: Record<string, DataType> = {
  Data: 'date',
  'dCal.Data': 'date',
  Categoria: 'text',
  Produto: 'text',
  Valor: 'number',
};
const typeOf = (f: string) => types[f];
const dc = (over: Partial<VisualDataConfig> = {}): VisualDataConfig => ({ tableId: 't', ...over });

describe('resolveDrillLevels', () => {
  it('eixo + drillFields explícitos viram a hierarquia na ordem', () => {
    const lv = resolveDrillLevels(dc({ xAxisField: 'Categoria', drillFields: ['Produto'] }), typeOf);
    expect(lv.map(l => l.field)).toEqual(['Categoria', 'Produto']);
    expect(lv.every(l => !l.datePart)).toBe(true);
  });

  it('coluna de data sem drillFields ganha Ano → Trimestre → Mês → Dia automática', () => {
    const lv = resolveDrillLevels(dc({ xAxisField: 'dCal.Data', drillMode: 'drill' }), typeOf);
    expect(lv.map(l => [l.field, l.datePart, l.caption])).toEqual([
      ['dCal.Data', 'year', 'Ano'],
      ['dCal.Data', 'quarter', 'Trimestre'],
      ['dCal.Data', 'month', 'Mês'],
      ['dCal.Data', 'day', 'Dia'],
    ]);
  });

  it('drillFields: [] explícito desliga a hierarquia de data', () => {
    expect(resolveDrillLevels(dc({ xAxisField: 'Data', drillFields: [], drillMode: 'drill' }), typeOf)).toHaveLength(1);
    expect(resolveDrillLevels(dc({ xAxisField: 'Data', drillFields: [], drillMode: 'drill' }), typeOf)[0].datePart).toBeUndefined();
  });

  it('visual salvo antes do v0.10 (sem drillMode) mantém o eixo de data cru, como abria antes', () => {
    const lv = resolveDrillLevels(dc({ xAxisField: 'Data' }), typeOf);
    expect(lv).toHaveLength(1);
    expect(lv[0].datePart).toBeUndefined();
  });

  it('coluna não-data sem drillFields = um nível só; sem eixo = nenhum', () => {
    expect(resolveDrillLevels(dc({ xAxisField: 'Categoria' }), typeOf)).toHaveLength(1);
    expect(resolveDrillLevels(dc(), typeOf)).toEqual([]);
  });
});

describe('resolveMatrixRowLevels', () => {
  it('matrixRowFields (N níveis) tem prioridade', () => {
    const lv = resolveMatrixRowLevels(dc({ xAxisField: 'A', matrixRowFields: ['A', 'B', 'C'], drillFields: ['Z'] }), typeOf);
    expect(lv.map(l => l.field)).toEqual(['A', 'B', 'C']);
  });
  it('o 1º nível sempre espelha xAxisField (trocar o campo das linhas no painel não dessincroniza)', () => {
    const lv = resolveMatrixRowLevels(dc({ xAxisField: 'Z', matrixRowFields: ['A', 'B', 'C'] }), typeOf);
    expect(lv.map(l => l.field)).toEqual(['Z', 'B', 'C']);
  });
  it('legado: xAxisField + drillFields[0] (matriz de 2 níveis do v0.4)', () => {
    const lv = resolveMatrixRowLevels(dc({ xAxisField: 'Categoria', drillFields: ['Produto'] }), typeOf);
    expect(lv.map(l => l.field)).toEqual(['Categoria', 'Produto']);
  });
  it('linha de data única ganha a hierarquia automática (visual v0.10+)', () => {
    expect(resolveMatrixRowLevels(dc({ xAxisField: 'Data', drillMode: 'drill' }), typeOf).map(l => l.datePart)).toEqual([
      'year',
      'quarter',
      'month',
      'day',
    ]);
  });
});

describe('estado e transições de drill', () => {
  const levels = resolveDrillLevels(dc({ xAxisField: 'Data', drillMode: 'drill' }), typeOf);

  it('estado padrão = nível 0, modo drill, caminho vazio; clamp de nível inválido', () => {
    expect(getDrillState(dc(), levels)).toEqual({ level: 0, mode: 'drill', path: [] });
    expect(getDrillState(dc({ currentDrillLevel: 99 }), levels).level).toBe(3);
    expect(getDrillState(dc({ currentDrillLevel: -2 }), levels).level).toBe(0);
  });

  it('drill down num ponto empilha o valor e desce um nível', () => {
    const p1 = drillDownOnPointPatch(dc(), levels, [2024]);
    expect(p1).toEqual({ currentDrillLevel: 1, drillPath: [2024], drillMode: 'drill' });
    const p2 = drillDownOnPointPatch(dc(p1!), levels, [1]);
    expect(p2).toEqual({ currentDrillLevel: 2, drillPath: [2024, 1], drillMode: 'drill' });
  });

  it('não desce além do último nível', () => {
    const last = dc({ currentDrillLevel: 3, drillPath: [2024, 1, 1] });
    expect(canDrillDown(getDrillState(last, levels), levels)).toBe(false);
    expect(drillDownOnPointPatch(last, levels, [15])).toBeNull();
    expect(goToNextLevelPatch(last, levels)).toBeNull();
    expect(expandAllLevelsPatch(last, levels)).toBeNull();
  });

  it('⇊ próximo nível empilha DRILL_ALL (sem filtrar)', () => {
    const p = goToNextLevelPatch(dc(), levels)!;
    expect(p.currentDrillLevel).toBe(1);
    expect(isDrillAll(p.drillPath![0])).toBe(true);
    expect(buildDrillPathFilters(getDrillState(dc(p), levels), levels)).toEqual([]);
  });

  it('drill up desempilha; no nível 0 não há drill up', () => {
    const at2 = dc({ currentDrillLevel: 2, drillPath: [2024, 3] });
    expect(drillUpPatch(at2, levels)).toEqual({ currentDrillLevel: 1, drillPath: [2024], drillMode: 'drill' });
    expect(canDrillUp(getDrillState(dc(), levels))).toBe(false);
    expect(drillUpPatch(dc(), levels)).toBeNull();
  });

  it('expandir todos os níveis desce em modo expand e limpa o caminho', () => {
    const p = expandAllLevelsPatch(dc({ currentDrillLevel: 1, drillPath: [2024] }), levels)!;
    expect(p).toEqual({ currentDrillLevel: 2, drillPath: [], drillMode: 'expand' });
    const st = getDrillState(dc(p), levels);
    expect(groupingLevels(st, levels).map(l => l.datePart)).toEqual(['year', 'quarter', 'month']);
    expect(buildDrillPathFilters(st, levels)).toEqual([]);
    // drill up no modo expand volta um nível; ao chegar em 0, volta ao modo drill
    expect(drillUpPatch(dc(p), levels)).toEqual({ currentDrillLevel: 1, drillPath: [], drillMode: 'expand' });
    expect(drillUpPatch(dc({ ...p, currentDrillLevel: 1 }), levels)).toEqual({
      currentDrillLevel: 0,
      drillPath: [],
      drillMode: 'drill',
    });
  });

  it('drill down num ponto em modo expand usa a tupla do ponto como caminho', () => {
    const exp = dc({ currentDrillLevel: 1, drillMode: 'expand', drillPath: [] });
    expect(drillDownOnPointPatch(exp, levels, [2024, 2])).toEqual({
      currentDrillLevel: 2,
      drillPath: [2024, 2],
      drillMode: 'drill',
    });
  });
});

describe('buildDrillPathFilters', () => {
  it('monta um filtro por nível acima do atual, com a parte da data', () => {
    const levels = resolveDrillLevels(dc({ xAxisField: 'Data', drillMode: 'drill' }), typeOf);
    const st = getDrillState(dc({ currentDrillLevel: 2, drillPath: [2024, 3] }), levels);
    expect(buildDrillPathFilters(st, levels)).toEqual([
      { field: 'Data', datePart: 'year', value: 2024 },
      { field: 'Data', datePart: 'quarter', value: 3 },
    ]);
    expect(groupingLevels(st, levels)).toEqual([levels[2]]);
  });

  it('campos simples (sem datePart) e DRILL_ALL no meio é ignorado', () => {
    const levels = resolveDrillLevels(dc({ xAxisField: 'Categoria', drillFields: ['Produto', 'Valor'] }), typeOf);
    const st = getDrillState(dc({ currentDrillLevel: 2, drillPath: [DRILL_ALL, 'Caneta'] }), levels);
    expect(buildDrillPathFilters(st, levels)).toEqual([{ field: 'Produto', value: 'Caneta' }]);
  });

  it('caminho mais curto que o nível (projeto editado à mão) não quebra', () => {
    const levels = resolveDrillLevels(dc({ xAxisField: 'Categoria', drillFields: ['Produto'] }), typeOf);
    const st = getDrillState(dc({ currentDrillLevel: 1 }), levels);
    expect(buildDrillPathFilters(st, levels)).toEqual([]);
  });
});

describe('partes de data e rótulos', () => {
  it('datePartValue aceita ISO, Date e serial do Excel', () => {
    expect(datePartValue('2024-05-17', 'year')).toBe(2024);
    expect(datePartValue('2024-05-17', 'quarter')).toBe(2);
    expect(datePartValue('2024-05-17', 'month')).toBe(5);
    expect(datePartValue('2024-05-17T00:00:00', 'day')).toBe(17);
    expect(datePartValue(new Date(Date.UTC(2023, 11, 31)), 'quarter')).toBe(4);
    expect(datePartValue(45292, 'year')).toBe(2024); // 2024-01-01
    expect(datePartValue('', 'year')).toBeNull();
    expect(datePartValue('não é data', 'year')).toBeNull();
  });

  it('formatDatePartLabel em PT-BR', () => {
    expect(formatDatePartLabel('year', 2024)).toBe('2024');
    expect(formatDatePartLabel('quarter', 1)).toBe('Trim 1');
    expect(formatDatePartLabel('month', 1)).toBe('janeiro');
    expect(formatDatePartLabel('month', 12)).toBe('dezembro');
    expect(formatDatePartLabel('day', 7)).toBe('7');
  });

  it('levelValueLabel: vazio vira "(Em branco)", data usa a parte', () => {
    expect(levelValueLabel({ field: 'X', caption: 'X' }, null)).toBe('(Em branco)');
    expect(levelValueLabel({ field: 'X', caption: 'X' }, 'Loja')).toBe('Loja');
    expect(levelValueLabel({ field: 'D', caption: 'Mês', datePart: 'month' }, 3)).toBe('março');
  });

  it('concatLevelLabels: "expandir todos os níveis" ex. "2024 Trim 1"', () => {
    const levels = resolveDrillLevels(dc({ xAxisField: 'Data', drillMode: 'drill' }), typeOf);
    expect(concatLevelLabels(levels.slice(0, 2), [2024, 1])).toBe('2024 Trim 1');
    expect(concatLevelLabels(levels.slice(0, 3), [2024, 1, 2])).toBe('2024 Trim 1 fevereiro');
  });
});

describe('buildHoverFilters (seams F↔G): dica de página respeita o nível/caminho de drill', () => {
  it('nível 0 (Ano): filtra pelo campo de data com datePart year, não pelo xAxisField cru', () => {
    const levels = resolveDrillLevels(dc({ xAxisField: 'Data', drillMode: 'drill' }), typeOf);
    const state = getDrillState(dc({ xAxisField: 'Data', drillMode: 'drill', currentDrillLevel: 0 }), levels);
    expect(buildHoverFilters(state, levels, [2024])).toEqual([{ field: 'Data', datePart: 'year', value: 2024 }]);
  });

  it('"2024 Q1" no modo drill (nível trimestre): filtra ano=2024 E trimestre=1 (ancestral + ponto atual)', () => {
    const levels = resolveDrillLevels(dc({ xAxisField: 'Data', drillMode: 'drill' }), typeOf);
    const dcState = dc({ xAxisField: 'Data', drillMode: 'drill', currentDrillLevel: 1, drillPath: [2024] });
    const state = getDrillState(dcState, levels);
    expect(buildHoverFilters(state, levels, [1])).toEqual([
      { field: 'Data', datePart: 'year', value: 2024 },
      { field: 'Data', datePart: 'quarter', value: 1 },
    ]);
  });

  it('"2024 Trim 1" no modo expand (rótulo concatenado): mesmo par ano+trimestre, sem ancestrais duplicados', () => {
    const levels = resolveDrillLevels(dc({ xAxisField: 'Data', drillMode: 'drill' }), typeOf);
    const dcState = dc({ xAxisField: 'Data', drillMode: 'expand', currentDrillLevel: 1, drillPath: [] });
    const state = getDrillState(dcState, levels);
    expect(buildHoverFilters(state, levels, [2024, 1])).toEqual([
      { field: 'Data', datePart: 'year', value: 2024 },
      { field: 'Data', datePart: 'quarter', value: 1 },
    ]);
  });

  it('campo plano (sem hierarquia de data): filtra pelo campo do nível atual, sem datePart', () => {
    const levels = resolveDrillLevels(dc({ xAxisField: 'Categoria', drillFields: ['Produto'] }), typeOf);
    const state = getDrillState(dc({ xAxisField: 'Categoria', drillFields: ['Produto'], currentDrillLevel: 1, drillPath: ['B'] }), levels);
    expect(buildHoverFilters(state, levels, ['b1'])).toEqual([
      { field: 'Categoria', value: 'B' },
      { field: 'Produto', value: 'b1' },
    ]);
  });

  it('"próximo nível" ($all) no caminho acima não gera filtro para aquele ancestral', () => {
    const levels = resolveDrillLevels(dc({ xAxisField: 'Data', drillMode: 'drill' }), typeOf);
    const state = getDrillState(dc({ xAxisField: 'Data', drillMode: 'drill', currentDrillLevel: 1, drillPath: [DRILL_ALL] }), levels);
    expect(buildHoverFilters(state, levels, [2])).toEqual([{ field: 'Data', datePart: 'quarter', value: 2 }]);
  });
});

describe('buildPathFilters (seams F↔G): caminho completo da linha de uma matriz', () => {
  it('matriz Regiao > Cidade > Loja: a linha "POA" (depth 1) filtra Regiao=Sul e Cidade=POA', () => {
    const levels = [
      { field: 'Regiao', caption: 'Regiao' },
      { field: 'Cidade', caption: 'Cidade' },
      { field: 'Loja', caption: 'Loja' },
    ];
    expect(buildPathFilters(levels, ['Sul', 'POA'])).toEqual([
      { field: 'Regiao', value: 'Sul' },
      { field: 'Cidade', value: 'POA' },
    ]);
  });
});

describe('expansão da matriz', () => {
  const base: MatrixExpansion = { depth: 0, expanded: [], collapsed: [] };

  it('toggle expande/recolhe uma linha específica', () => {
    const e1 = toggleMatrixPath(base, ['A']);
    expect(isMatrixPathExpanded(e1, ['A'])).toBe(true);
    expect(isMatrixPathExpanded(e1, ['B'])).toBe(false);
    const e2 = toggleMatrixPath(e1, ['A']);
    expect(isMatrixPathExpanded(e2, ['A'])).toBe(false);
  });

  it('expandir um nível desce todas as linhas visíveis; recolher uma linha depois funciona', () => {
    const e1 = expandOneLevel(base, 3);
    expect(e1.depth).toBe(1);
    expect(isMatrixPathExpanded(e1, ['A'])).toBe(true);
    expect(isMatrixPathExpanded(e1, ['A', 'x'])).toBe(false);
    const e2 = toggleMatrixPath(e1, ['A']);
    expect(isMatrixPathExpanded(e2, ['A'])).toBe(false);
    expect(isMatrixPathExpanded(e2, ['B'])).toBe(true);
    // nunca passa do penúltimo nível (o último não tem filhos)
    expect(expandOneLevel(expandOneLevel(e1, 3), 3).depth).toBe(2);
  });

  it('matrixPathId é estável e distingue número de texto', () => {
    expect(matrixPathId([2024, 'Q1'])).toBe(matrixPathId([2024, 'Q1']));
    expect(matrixPathId([2024])).not.toBe(matrixPathId(['2024']));
  });
});

// v0.11 Frente D: descrição legível de um filtro de drill (cross-filter por data
// drillada / rótulo concatenado) para o painel de Filtros.
describe('describeDrillFilterSpec/describeDrillFilterSpecs', () => {
  it('descreve um nível de data com a legenda e o rótulo formatado da parte', () => {
    const levels = [{ field: 'Data', datePart: 'quarter' as const, caption: 'Trimestre' }];
    expect(describeDrillFilterSpec(levels, { field: 'Data', datePart: 'quarter', value: 2 })).toBe('Trimestre: Trim 2');
  });

  it('descreve um campo comum (sem parte de data) usando a legenda do nível', () => {
    const levels = [{ field: 'Regiao', caption: 'Região' }];
    expect(describeDrillFilterSpec(levels, { field: 'Regiao', value: 'Sul' })).toBe('Região: Sul');
  });

  it('cai no próprio nome do campo quando não acha o nível (caso defensivo)', () => {
    expect(describeDrillFilterSpec([], { field: 'Produto', value: 'X' })).toBe('Produto: X');
  });

  it('concatena ancestrais + nível atual com " · " (caminho completo de um ponto drillado)', () => {
    const levels = [
      { field: 'Data', datePart: 'year' as const, caption: 'Ano' },
      { field: 'Data', datePart: 'quarter' as const, caption: 'Trimestre' },
    ];
    const specs = [
      { field: 'Data', datePart: 'year' as const, value: 2024 },
      { field: 'Data', datePart: 'quarter' as const, value: 2 },
    ];
    expect(describeDrillFilterSpecs(levels, specs)).toBe('Ano: 2024 · Trimestre: Trim 2');
  });
});
