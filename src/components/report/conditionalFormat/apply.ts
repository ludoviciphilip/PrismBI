/**
 * Camada fina que liga o motor puro (`src/engine/conditionalFormat.ts`) aos
 * visuais (tabela, matriz, cartão, cartão múltiplo e barras/colunas). Toda
 * a lógica de negócio fica no motor; aqui só resolvemos `basedOn` a partir
 * dos dados do visual e convertemos o resultado em estilo/props de React
 * ou em dados do ECharts.
 */
import type { CSSProperties } from 'react';
import { computeCellFormat, type FormatResult } from '../../../engine/conditionalFormat';
import type { ConditionalFormat, Visual } from '../../../types/bi';

/** Resolve o valor (e os valores visíveis) de um campo `basedOn` dentro de um conjunto de linhas. */
export interface BasedOnResolver {
  /** Valor de `field` na linha/ponto atual (ou undefined se não encontrado). */
  value: (field: string) => unknown;
  /** Todos os valores visíveis de `field` no visual (para min/max/percentil). */
  visible: (field: string) => number[];
}

function toNumArray(values: unknown[]): number[] {
  const out: number[] = [];
  for (const v of values) {
    if (v === null || v === undefined || v === '') continue;
    const n = Number(v);
    if (Number.isFinite(n)) out.push(n);
  }
  return out;
}

/** Monta um `BasedOnResolver` simples a partir de uma lista de linhas tipo `Record<string, any>`. */
export function resolverFromRows(rows: Record<string, unknown>[], currentRow: Record<string, unknown>): BasedOnResolver {
  return {
    value: (field) => currentRow[field],
    visible: (field) => toNumArray(rows.map(r => r[field])),
  };
}

/** Monta um `BasedOnResolver` a partir de séries do ECharts/matriz: `{ name -> data[] }`, no índice de linha `rowIdx`. */
export function resolverFromSeries(series: { name: string; data: (number | null)[] }[], rowIdx: number): BasedOnResolver {
  return {
    value: (field) => {
      const s = series.find(s => s.name === field);
      return s ? s.data[rowIdx] : undefined;
    },
    visible: (field) => {
      const s = series.find(s => s.name === field);
      return s ? toNumArray(s.data) : [];
    },
  };
}

/** Calcula o `FormatResult` combinado para `field`, resolvendo `basedOn` via `resolver`. */
export function getFieldFormat(
  formats: ConditionalFormat[] | undefined,
  field: string,
  value: unknown,
  visibleValues: number[],
  resolver?: BasedOnResolver
): FormatResult {
  if (!formats || formats.length === 0) return {};
  return computeCellFormat(
    formats,
    field,
    (format) => (format.basedOn && resolver ? resolver.value(format.basedOn.field) : value),
    (format) => (format.basedOn && resolver ? resolver.visible(format.basedOn.field) : visibleValues)
  );
}

/** `conditionalFormats` ativos (migração + campo novo), nunca undefined. */
export function activeFormats(visual: Visual): ConditionalFormat[] {
  return visual.formatConfig.conditionalFormats || [];
}

/**
 * v0.10 Frente H: formatos que também valem para totais/subtotais da
 * matriz — "Aplicar a: Valores e totais" no painel (ver `ConditionalFormat.applyToTotals`).
 * "Somente valores" (padrão) não inclui o formato aqui.
 */
export function totalFormats(formats: ConditionalFormat[] | undefined): ConditionalFormat[] {
  return (formats || []).filter(f => f.applyToTotals);
}

/**
 * Formatação condicional para uma célula de TOTAL/SUBTOTAL da matriz
 * (coluna "Total" por linha, linha "Total Geral", total de um grupo
 * expandido). Usa só os `ConditionalFormat` com `applyToTotals: true` e
 * calcula o gradiente/percentil sobre `totalsVisibleValues` — os totais
 * visíveis no mesmo nível, não os valores de célula comuns — replicando o
 * Power BI, que usa uma escala própria para os totais.
 *
 * INTEGRAÇÃO (Frente G, matriz N níveis): para cada célula de
 * subtotal/total (qualquer nível) e para o total geral, chamar isto com
 * `field` = o campo da medida daquela coluna de total e
 * `totalsVisibleValues` = todos os totais do mesmo nível/coluna
 * atualmente visíveis (ex.: todos os subtotais de um grupo, ou todos os
 * totais de coluna da linha "Total Geral"). Aplicar o `CSSProperties`
 * retornado na célula; não precisa (e não deve) tocar no motor.
 */
export function styleForTotalCell(
  formats: ConditionalFormat[] | undefined,
  field: string,
  value: number,
  totalsVisibleValues: number[]
): CSSProperties {
  const applicable = totalFormats(formats);
  if (applicable.length === 0) return {};
  const result = computeCellFormat(
    applicable,
    field,
    () => value,
    () => totalsVisibleValues
  );
  return cellStyleFromFormat(result);
}

/** Converte um `FormatResult` (sem a parte de barra/ícone) em estilo inline de célula. */
export function cellStyleFromFormat(result: FormatResult): CSSProperties {
  const style: CSSProperties = {};
  if (result.background) style.backgroundColor = result.background;
  if (result.color) style.color = result.color;
  return style;
}

export type BarSeriesDatum = number | null | { value: number; itemStyle: { color: string } };

/**
 * Colore cada ponto de uma série de barras/colunas do ECharts via
 * `itemStyle` quando há `ConditionalFormat` de `target: 'background'`
 * aplicado ao campo da série (`fieldName`). Pontos sem cor condicional
 * mantêm o número puro (herdam a cor padrão/paleta da série).
 */
export function buildBarSeriesData(formats: ConditionalFormat[] | undefined, fieldName: string, rawData: (number | null)[]): BarSeriesDatum[] {
  if (!formats || formats.length === 0) return rawData;
  const applicable = formats.some(f => f.applyTo === fieldName && f.target === 'background');
  if (!applicable) return rawData;
  const visible = toNumArray(rawData);
  return rawData.map((value, idx) => {
    if (value === null || value === undefined) return null;
    const resolver = resolverFromSeries([{ name: fieldName, data: rawData }], idx);
    const result = getFieldFormat(formats, fieldName, value, visible, resolver);
    if (!result.background) return value;
    return { value, itemStyle: { color: result.background } };
  });
}
