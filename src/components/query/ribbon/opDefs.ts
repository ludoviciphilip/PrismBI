// Tabelas de operações de texto/número/data da Rodada 2: cada OpDef gera uma
// expressão M a partir de uma referência de coluna (`_` para transformColumns,
// `[Coluna]` para addCustomColumn) e, opcionalmente, de parâmetros coletados
// no ParamDialog. Mantém o menu data-driven: um único lugar define rótulo,
// tipo resultante e fórmula, usado tanto em "Transformar" (in-place) quanto em
// "Adicionar Coluna" (nova coluna).

import { EtlType } from '../../../types/etl';

export interface ParamFieldSpec {
  key: string;
  label: string;
  type: 'text' | 'number' | 'integer';
  defaultValue?: string | number;
}

export interface OpDef {
  id: string;
  label: string;
  type: EtlType;
  expr: (ref: string, params: Record<string, any>) => string;
  params?: ParamFieldSpec[];
  /** nome sugerido da nova coluna em "Adicionar Coluna" (padrão: label). */
  newColumnName?: string | ((params: Record<string, any>) => string);
}

export interface OpGroup {
  group: string;
  ops: OpDef[];
}

const num = (key: string, label: string, def: number): ParamFieldSpec => ({ key, label, type: 'number', defaultValue: def });
const int = (key: string, label: string, def: number): ParamFieldSpec => ({ key, label, type: 'integer', defaultValue: def });
const text = (key: string, label: string, def = ''): ParamFieldSpec => ({ key, label, type: 'text', defaultValue: def });

// ---------------------------------------------------------------------------
// Texto — "Extrair"
// ---------------------------------------------------------------------------
export const TEXT_EXTRACT_OPS: OpDef[] = [
  { id: 'length', label: 'Comprimento', type: 'integer', expr: ref => `Text.Length(${ref})`, newColumnName: 'Comprimento' },
  {
    id: 'firstChars',
    label: 'Primeiros Caracteres',
    type: 'text',
    params: [int('count', 'Número de caracteres', 3)],
    expr: (ref, p) => `Text.Start(${ref}, ${p.count})`,
    newColumnName: 'Primeiros Caracteres',
  },
  {
    id: 'lastChars',
    label: 'Últimos Caracteres',
    type: 'text',
    params: [int('count', 'Número de caracteres', 3)],
    expr: (ref, p) => `Text.End(${ref}, ${p.count})`,
    newColumnName: 'Últimos Caracteres',
  },
  {
    id: 'range',
    label: 'Intervalo',
    type: 'text',
    params: [int('start', 'Posição inicial', 0), int('count', 'Número de caracteres', 3)],
    expr: (ref, p) => `Text.Middle(${ref}, ${p.start}, ${p.count})`,
    newColumnName: 'Intervalo',
  },
  {
    id: 'beforeDelimiter',
    label: 'Texto Antes do Delimitador',
    type: 'text',
    params: [text('delimiter', 'Delimitador', '-')],
    expr: (ref, p) => `Text.BeforeDelimiter(${ref}, "${p.delimiter}")`,
    newColumnName: 'Texto Antes do Delimitador',
  },
  {
    id: 'afterDelimiter',
    label: 'Texto Depois do Delimitador',
    type: 'text',
    params: [text('delimiter', 'Delimitador', '-')],
    expr: (ref, p) => `Text.AfterDelimiter(${ref}, "${p.delimiter}")`,
    newColumnName: 'Texto Depois do Delimitador',
  },
  {
    id: 'betweenDelimiters',
    label: 'Texto Entre Delimitadores',
    type: 'text',
    params: [text('start', 'Delimitador inicial', '('), text('end', 'Delimitador final', ')')],
    expr: (ref, p) => `Text.BetweenDelimiters(${ref}, "${p.start}", "${p.end}")`,
    newColumnName: 'Texto Entre Delimitadores',
  },
];

// ---------------------------------------------------------------------------
// Texto — "Formato" (além de upper/lower/proper/trim/clean, que usam o op
// dedicado `textTransform` por já existir no schema).
// ---------------------------------------------------------------------------
export const TEXT_FORMAT_EXTRA_OPS: OpDef[] = [
  { id: 'prefix', label: 'Adicionar Prefixo', type: 'text', params: [text('prefix', 'Prefixo')], expr: (ref, p) => `"${p.prefix}" & ${ref}`, newColumnName: 'Prefixo Adicionado' },
  { id: 'suffix', label: 'Adicionar Sufixo', type: 'text', params: [text('suffix', 'Sufixo')], expr: (ref, p) => `${ref} & "${p.suffix}"`, newColumnName: 'Sufixo Adicionado' },
];

// ---------------------------------------------------------------------------
// Número — "Padrão" (opera com um valor digitado; combina 2 colunas quando
// exatamente 2 estiverem selecionadas — resolvido no builder, não aqui).
// ---------------------------------------------------------------------------
export const NUMBER_STANDARD_OPS: OpDef[] = [
  { id: 'add', label: 'Somar', type: 'decimal', params: [num('value', 'Valor', 0)], expr: (ref, p) => `${ref} + ${p.value}`, newColumnName: 'Adição' },
  { id: 'multiply', label: 'Multiplicar', type: 'decimal', params: [num('value', 'Valor', 1)], expr: (ref, p) => `${ref} * ${p.value}`, newColumnName: 'Multiplicação' },
  { id: 'subtract', label: 'Subtrair', type: 'decimal', params: [num('value', 'Valor', 0)], expr: (ref, p) => `${ref} - ${p.value}`, newColumnName: 'Subtração' },
  { id: 'divide', label: 'Dividir', type: 'decimal', params: [num('value', 'Valor', 1)], expr: (ref, p) => `${ref} / ${p.value}`, newColumnName: 'Divisão' },
  { id: 'integerDivide', label: 'Divisão Inteira', type: 'integer', params: [num('value', 'Valor', 1)], expr: (ref, p) => `Number.IntegerDivide(${ref}, ${p.value})`, newColumnName: 'Divisão Inteira' },
  { id: 'modulo', label: 'Módulo', type: 'decimal', params: [num('value', 'Valor', 1)], expr: (ref, p) => `Number.Mod(${ref}, ${p.value})`, newColumnName: 'Módulo' },
  { id: 'percentOf', label: 'Percentual de', type: 'decimal', params: [num('value', 'Valor', 100)], expr: (ref, p) => `${ref} / ${p.value} * 100`, newColumnName: 'Percentual' },
];

/** Combina duas colunas (usado quando exatamente 2 colunas estão selecionadas). */
export const NUMBER_STANDARD_COMBINE_OPS: Record<string, OpDef> = {
  add: { id: 'add', label: 'Somar', type: 'decimal', expr: (_r, p) => `[${p.a}] + [${p.b}]`, newColumnName: 'Adição' },
  multiply: { id: 'multiply', label: 'Multiplicar', type: 'decimal', expr: (_r, p) => `[${p.a}] * [${p.b}]`, newColumnName: 'Multiplicação' },
  subtract: { id: 'subtract', label: 'Subtrair', type: 'decimal', expr: (_r, p) => `[${p.a}] - [${p.b}]`, newColumnName: 'Subtração' },
  divide: { id: 'divide', label: 'Dividir', type: 'decimal', expr: (_r, p) => `[${p.a}] / [${p.b}]`, newColumnName: 'Divisão' },
  integerDivide: { id: 'integerDivide', label: 'Divisão Inteira', type: 'integer', expr: (_r, p) => `Number.IntegerDivide([${p.a}], [${p.b}])`, newColumnName: 'Divisão Inteira' },
  modulo: { id: 'modulo', label: 'Módulo', type: 'decimal', expr: (_r, p) => `Number.Mod([${p.a}], [${p.b}])`, newColumnName: 'Módulo' },
  percentOf: { id: 'percentOf', label: 'Percentual de', type: 'decimal', expr: (_r, p) => `[${p.a}] / [${p.b}] * 100`, newColumnName: 'Percentual' },
};

export const NUMBER_SCIENTIFIC_OPS: OpGroup[] = [
  { group: 'Científico', ops: [{ id: 'abs', label: 'Valor Absoluto', type: 'decimal', expr: ref => `Number.Abs(${ref})`, newColumnName: 'Valor Absoluto' }] },
  {
    group: 'Potência',
    ops: [
      { id: 'square', label: 'Quadrado', type: 'decimal', expr: ref => `Number.Power(${ref}, 2)`, newColumnName: 'Quadrado' },
      { id: 'cube', label: 'Cubo', type: 'decimal', expr: ref => `Number.Power(${ref}, 3)`, newColumnName: 'Cubo' },
      { id: 'power', label: 'Potência', type: 'decimal', params: [num('exponent', 'Expoente', 2)], expr: (ref, p) => `Number.Power(${ref}, ${p.exponent})`, newColumnName: 'Potência' },
    ],
  },
  { group: 'Raiz', ops: [{ id: 'sqrt', label: 'Raiz Quadrada', type: 'decimal', expr: ref => `Number.Sqrt(${ref})`, newColumnName: 'Raiz Quadrada' }] },
  {
    group: 'Logaritmo',
    ops: [
      { id: 'log10', label: 'Base 10', type: 'decimal', expr: ref => `Number.Log10(${ref})`, newColumnName: 'Logaritmo Base 10' },
      { id: 'ln', label: 'Natural', type: 'decimal', expr: ref => `Number.Ln(${ref})`, newColumnName: 'Logaritmo Natural' },
    ],
  },
  { group: 'Exponencial', ops: [{ id: 'exp', label: 'Exponencial', type: 'decimal', expr: ref => `Number.Exp(${ref})`, newColumnName: 'Exponencial' }] },
];

export const NUMBER_ROUNDING_OPS: OpDef[] = [
  { id: 'round', label: 'Arredondar', type: 'decimal', params: [int('digits', 'Casas decimais', 0)], expr: (ref, p) => `Number.Round(${ref}, ${p.digits})`, newColumnName: 'Arredondado' },
  { id: 'roundUp', label: 'Para Cima', type: 'decimal', expr: ref => `Number.RoundUp(${ref}, 0)`, newColumnName: 'Arredondado para Cima' },
  { id: 'roundDown', label: 'Para Baixo', type: 'decimal', expr: ref => `Number.RoundDown(${ref}, 0)`, newColumnName: 'Arredondado para Baixo' },
];

export const NUMBER_INFO_OPS: OpDef[] = [
  { id: 'isEven', label: 'É Par', type: 'boolean', expr: ref => `Number.IsEven(${ref})`, newColumnName: 'É Par' },
  { id: 'isOdd', label: 'É Ímpar', type: 'boolean', expr: ref => `Number.IsOdd(${ref})`, newColumnName: 'É Ímpar' },
  { id: 'sign', label: 'Sinal', type: 'integer', expr: ref => `Number.Sign(${ref})`, newColumnName: 'Sinal' },
];

// ---------------------------------------------------------------------------
// Data — a árvore inteira da Rodada 2 (Data/Ano/Mês/Trimestre/Semana/Dia/Idade)
// ---------------------------------------------------------------------------
export const DATE_OP_GROUPS: OpGroup[] = [
  { group: 'Data', ops: [{ id: 'onlyDate', label: 'Somente Data', type: 'date', expr: ref => `DateTime.Date(${ref})`, newColumnName: 'Data' }] },
  {
    group: 'Ano',
    ops: [
      { id: 'year', label: 'Ano', type: 'integer', expr: ref => `Date.Year(${ref})`, newColumnName: 'Ano' },
      { id: 'yearStart', label: 'Início do Ano', type: 'date', expr: ref => `Date.StartOfYear(${ref})`, newColumnName: 'Início do Ano' },
      { id: 'yearEnd', label: 'Fim do Ano', type: 'date', expr: ref => `Date.EndOfYear(${ref})`, newColumnName: 'Fim do Ano' },
    ],
  },
  {
    group: 'Mês',
    ops: [
      { id: 'month', label: 'Mês', type: 'integer', expr: ref => `Date.Month(${ref})`, newColumnName: 'Mês' },
      { id: 'monthName', label: 'Nome do Mês', type: 'text', expr: ref => `Date.MonthName(${ref}, "pt-BR")`, newColumnName: 'Nome do Mês' },
      { id: 'monthStart', label: 'Início do Mês', type: 'date', expr: ref => `Date.StartOfMonth(${ref})`, newColumnName: 'Início do Mês' },
      { id: 'monthEnd', label: 'Fim do Mês', type: 'date', expr: ref => `Date.EndOfMonth(${ref})`, newColumnName: 'Fim do Mês' },
      { id: 'daysInMonth', label: 'Dias no Mês', type: 'integer', expr: ref => `Date.DaysInMonth(${ref})`, newColumnName: 'Dias no Mês' },
    ],
  },
  {
    group: 'Trimestre',
    ops: [
      { id: 'quarter', label: 'Trimestre', type: 'integer', expr: ref => `Date.QuarterOfYear(${ref})`, newColumnName: 'Trimestre' },
      { id: 'quarterStart', label: 'Início do Trimestre', type: 'date', expr: ref => `Date.StartOfQuarter(${ref})`, newColumnName: 'Início do Trimestre' },
      { id: 'quarterEnd', label: 'Fim do Trimestre', type: 'date', expr: ref => `Date.EndOfQuarter(${ref})`, newColumnName: 'Fim do Trimestre' },
    ],
  },
  {
    group: 'Semana',
    ops: [
      { id: 'weekOfYear', label: 'Semana do Ano', type: 'integer', expr: ref => `Date.WeekOfYear(${ref})`, newColumnName: 'Semana do Ano' },
      { id: 'weekOfMonth', label: 'Semana do Mês', type: 'integer', expr: ref => `Date.WeekOfMonth(${ref})`, newColumnName: 'Semana do Mês' },
      { id: 'weekStart', label: 'Início da Semana', type: 'date', expr: ref => `Date.StartOfWeek(${ref})`, newColumnName: 'Início da Semana' },
      { id: 'weekEnd', label: 'Fim da Semana', type: 'date', expr: ref => `Date.EndOfWeek(${ref})`, newColumnName: 'Fim da Semana' },
    ],
  },
  {
    group: 'Dia',
    ops: [
      { id: 'day', label: 'Dia', type: 'integer', expr: ref => `Date.Day(${ref})`, newColumnName: 'Dia' },
      { id: 'dayName', label: 'Nome do Dia', type: 'text', expr: ref => `Date.DayOfWeekName(${ref}, "pt-BR")`, newColumnName: 'Nome do Dia' },
      { id: 'dayOfWeek', label: 'Dia da Semana', type: 'integer', expr: ref => `Date.DayOfWeek(${ref})`, newColumnName: 'Dia da Semana' },
      { id: 'dayOfYear', label: 'Dia do Ano', type: 'integer', expr: ref => `Date.DayOfYear(${ref})`, newColumnName: 'Dia do Ano' },
    ],
  },
  {
    group: 'Idade',
    ops: [{ id: 'age', label: 'Idade', type: 'integer', expr: ref => `Duration.Days(DateTime.Date(DateTime.LocalNow()) - ${ref})`, newColumnName: 'Idade' }],
  },
];

export const DATE_OP_FLAT: OpDef[] = DATE_OP_GROUPS.flatMap(g => g.ops);

// ---------------------------------------------------------------------------
// Builders: EtlStep a partir de um OpDef + seleção (table-driven, testável)
// ---------------------------------------------------------------------------
import { DistributiveOmit, EtlStep } from '../../../types/etl';

export function buildTransformColumnsStep(op: OpDef, columns: string[], params: Record<string, any> = {}): DistributiveOmit<EtlStep, 'id'> {
  return { op: 'transformColumns', columns, expression: op.expr('_', params), type: op.type, name: op.label };
}

export function buildAddCustomColumnStep(op: OpDef, column: string, params: Record<string, any> = {}): DistributiveOmit<EtlStep, 'id'> {
  const newColumn = typeof op.newColumnName === 'function' ? op.newColumnName(params) : op.newColumnName || op.label;
  return { op: 'addCustomColumn', newColumn, expression: op.expr(`[${column}]`, params), type: op.type, name: `${newColumn} Adicionado` };
}

export function buildCombineStep(op: OpDef, a: string, b: string): DistributiveOmit<EtlStep, 'id'> {
  const newColumn = typeof op.newColumnName === 'function' ? op.newColumnName({ a, b }) : op.newColumnName || op.label;
  return { op: 'addCustomColumn', newColumn, expression: op.expr('', { a, b }), type: op.type, name: `${newColumn} Adicionado` };
}
