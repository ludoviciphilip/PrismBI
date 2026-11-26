/**
 * Motor puro de formatação condicional (v0.10, Frente B).
 *
 * Dada uma lista de `ConditionalFormat` (src/types/bi.ts) e os valores
 * visíveis de um campo num visual, calcula o estilo (cor de fundo, cor da
 * fonte, barra de dados ou ícone) que deve ser aplicado a uma célula/barra.
 *
 * Sem dependências de React/DOM — é chamado tanto pela UI de tabela/matriz/
 * cartão quanto pela config do ECharts (barras/colunas).
 */
import type { CondFormatStop, ConditionalFormat, ConditionalFormatRule } from '../types/bi';

export type IconKey =
  | 'arrow-up' | 'arrow-right' | 'arrow-down'
  | 'circle-green' | 'circle-yellow' | 'circle-red'
  | 'flag-green' | 'flag-yellow' | 'flag-red';

export interface DataBarMetrics {
  /** Posição do eixo zero, em % da largura da célula (0 quando min >= 0). */
  axisPercent: number;
  /** Início da barra, em % da largura da célula. */
  startPercent: number;
  /** Largura da barra, em % da largura da célula. */
  widthPercent: number;
  isNegative: boolean;
  color: string;
}

export interface FormatResult {
  background?: string;
  color?: string;
  dataBar?: DataBarMetrics;
  icon?: { key: IconKey; color: string };
}

const ICON_SETS: Record<'arrows' | 'traffic' | 'flags', { key: IconKey; color: string }[]> = {
  arrows: [
    { key: 'arrow-down', color: '#dc2626' },
    { key: 'arrow-right', color: '#d97706' },
    { key: 'arrow-up', color: '#16a34a' },
  ],
  traffic: [
    { key: 'circle-red', color: '#dc2626' },
    { key: 'circle-yellow', color: '#d97706' },
    { key: 'circle-green', color: '#16a34a' },
  ],
  flags: [
    { key: 'flag-red', color: '#dc2626' },
    { key: 'flag-yellow', color: '#d97706' },
    { key: 'flag-green', color: '#16a34a' },
  ],
};

/** BLANK no sentido do Power BI: null/undefined/'' ou NaN numérico. */
export function isBlank(value: unknown): boolean {
  if (value === null || value === undefined) return true;
  if (typeof value === 'number' && Number.isNaN(value)) return true;
  if (typeof value === 'string' && value.trim() === '') return true;
  return false;
}

function toNumber(value: unknown): number | null {
  if (isBlank(value)) return null;
  const n = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(n) ? n : null;
}

/**
 * Percentil simples (interpolação linear, igual ao método "linear" do
 * numpy/Excel PERCENTILE.INC). `p` vai de 0 a 100.
 */
export function percentile(values: number[], p: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  if (sorted.length === 1) return sorted[0];
  const clamped = Math.max(0, Math.min(100, p));
  const rank = (clamped / 100) * (sorted.length - 1);
  const lo = Math.floor(rank);
  const hi = Math.ceil(rank);
  if (lo === hi) return sorted[lo];
  const frac = rank - lo;
  return sorted[lo] + (sorted[hi] - sorted[lo]) * frac;
}

/** Resolve o valor numérico de um `CondFormatStop` contra os valores visíveis. */
export function resolveStopValue(stop: CondFormatStop, visibleValues: number[]): number {
  switch (stop.kind) {
    case 'lowest':
      return visibleValues.length ? Math.min(...visibleValues) : 0;
    case 'highest':
      return visibleValues.length ? Math.max(...visibleValues) : 0;
    case 'percent':
      return percentile(visibleValues, stop.value ?? 0);
    case 'number':
    default:
      return stop.value ?? 0;
  }
}

/** Resolve um limiar de regra, em número ou percentil dos valores visíveis. */
function resolveRuleThreshold(raw: number, valueKind: 'number' | 'percent' | undefined, visibleValues: number[]): number {
  if (valueKind === 'percent') return percentile(visibleValues, raw);
  return raw;
}

function matchRule(
  value: number,
  rule: ConditionalFormatRule & { valueKind?: 'number' | 'percent' },
  visibleValues: number[]
): boolean {
  const a = resolveRuleThreshold(rule.value, rule.valueKind, visibleValues);
  switch (rule.operator) {
    case '>':
      return value > a;
    case '<':
      return value < a;
    case '>=':
      return value >= a;
    case '<=':
      return value <= a;
    case '==':
      return value === a;
    case 'between': {
      const b = resolveRuleThreshold(rule.value2 ?? rule.value, rule.valueKind, visibleValues);
      const lo = Math.min(a, b);
      const hi = Math.max(a, b);
      return value >= lo && value <= hi;
    }
    default:
      return false;
  }
}

/** Primeira regra que casa (ordem da lista) vence, como no Power BI. */
export function firstMatchingRule<T extends ConditionalFormatRule & { valueKind?: 'number' | 'percent' }>(
  rules: T[],
  value: number,
  visibleValues: number[]
): { rule: T; index: number } | null {
  for (let i = 0; i < rules.length; i++) {
    if (matchRule(value, rules[i], visibleValues)) return { rule: rules[i], index: i };
  }
  return null;
}

function clamp01(t: number): number {
  return Math.max(0, Math.min(1, t));
}

function hexToRgb(hex: string): [number, number, number] | null {
  const m = hex.trim().match(/^#?([0-9a-f]{3}|[0-9a-f]{6})$/i);
  if (!m) return null;
  let h = m[1];
  if (h.length === 3) h = h.split('').map(c => c + c).join('');
  const num = parseInt(h, 16);
  return [(num >> 16) & 255, (num >> 8) & 255, num & 255];
}

function rgbToHex(r: number, g: number, b: number): string {
  const c = (v: number) => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, '0');
  return `#${c(r)}${c(g)}${c(b)}`;
}

/** Interpola linearmente entre duas cores hex (`#RRGGBB`/`#RGB`). t em [0,1]. */
export function interpolateColor(colorA: string, colorB: string, t: number): string {
  const rgbA = hexToRgb(colorA);
  const rgbB = hexToRgb(colorB);
  const tt = clamp01(t);
  if (!rgbA || !rgbB) return tt < 0.5 ? colorA : colorB;
  return rgbToHex(
    rgbA[0] + (rgbB[0] - rgbA[0]) * tt,
    rgbA[1] + (rgbB[1] - rgbA[1]) * tt,
    rgbA[2] + (rgbB[2] - rgbA[2]) * tt
  );
}

/** Cor do gradiente (2 ou 3 paradas) para um valor, dados os limites visíveis. */
export function gradientColor(
  gradient: { min: CondFormatStop; mid?: CondFormatStop; max: CondFormatStop },
  value: number,
  visibleValues: number[]
): string {
  const min = resolveStopValue(gradient.min, visibleValues);
  const max = resolveStopValue(gradient.max, visibleValues);
  if (max <= min) return gradient.min.color;

  if (gradient.mid) {
    const mid = resolveStopValue(gradient.mid, visibleValues);
    if (value <= mid) {
      if (mid <= min) return gradient.mid.color;
      return interpolateColor(gradient.min.color, gradient.mid.color, (value - min) / (mid - min));
    }
    if (max <= mid) return gradient.mid.color;
    return interpolateColor(gradient.mid.color, gradient.max.color, (value - mid) / (max - mid));
  }

  return interpolateColor(gradient.min.color, gradient.max.color, (value - min) / (max - min));
}

/** Resolve a cor do modo "valor do campo": hex literal ou nome CSS, usados como estão. */
export function fieldValueColor(value: unknown): string | undefined {
  if (isBlank(value)) return undefined;
  const str = String(value).trim();
  if (!str) return undefined;
  if (/^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(str)) return str;
  // Nome de cor CSS (ex.: "red", "steelblue") — passado adiante como está;
  // o navegador resolve nomes válidos e ignora inválidos.
  return str;
}

/** Calcula a métrica de uma barra de dados para um valor dentro de [min,max]. */
export function computeDataBarMetrics(
  value: number,
  visibleValues: number[],
  colors: { positiveColor: string; negativeColor: string }
): DataBarMetrics {
  const min = visibleValues.length ? Math.min(...visibleValues) : Math.min(0, value);
  const max = visibleValues.length ? Math.max(...visibleValues) : Math.max(0, value);
  const range = max - min;
  const axisPercent = min < 0 && max > 0 ? (-min / range) * 100 : min >= 0 ? 0 : 100;

  if (range <= 0) {
    return { axisPercent, startPercent: axisPercent, widthPercent: 0, isNegative: value < 0, color: colors.positiveColor };
  }

  const isNegative = value < 0;
  if (!isNegative) {
    const posMax = max > 0 ? max : 1;
    const widthPercent = (Math.min(value, posMax) / posMax) * (100 - axisPercent);
    return { axisPercent, startPercent: axisPercent, widthPercent, isNegative, color: colors.positiveColor };
  }
  const negMax = min < 0 ? Math.abs(min) : 1;
  const widthPercent = (Math.min(Math.abs(value), negMax) / negMax) * axisPercent;
  return { axisPercent, startPercent: axisPercent - widthPercent, widthPercent, isNegative, color: colors.negativeColor };
}

function iconColorForRule(
  rule: ConditionalFormatRule,
  index: number,
  setIcons: { key: IconKey; color: string }[]
): { key: IconKey; color: string } {
  const fallback = setIcons[Math.min(index, setIcons.length - 1)];
  return { key: fallback.key, color: rule.color || fallback.color };
}

/**
 * Calcula o estilo (de um único `target`) de um `ConditionalFormat` para um
 * valor já resolvido (campo ou `basedOn`). Retorna `null` quando o valor é
 * BLANK ou nenhuma regra/gradiente/valor se aplica.
 */
export function computeFormatForValue(format: ConditionalFormat, value: unknown, visibleValues: number[]): FormatResult | null {
  if (isBlank(value)) return null;

  if (format.mode === 'fieldValue') {
    const color = fieldValueColor(value);
    if (!color) return null;
    if (format.target === 'fontColor') return { color };
    if (format.target === 'background') return { background: color };
    if (format.target === 'dataBar') {
      const num = toNumber(value);
      if (num === null) return null;
      const metrics = computeDataBarMetrics(num, visibleValues, format.dataBar || { positiveColor: color, negativeColor: color });
      return { dataBar: { ...metrics, color } };
    }
    return null;
  }

  if (format.target === 'dataBar') {
    const num = toNumber(value);
    if (num === null) return null;
    const colors = format.dataBar || { positiveColor: '#3b82f6', negativeColor: '#ef4444' };
    return { dataBar: computeDataBarMetrics(num, visibleValues, colors) };
  }

  if (format.target === 'icon') {
    const num = toNumber(value);
    if (num === null || !format.icons) return null;
    const match = firstMatchingRule(format.icons.rules, num, visibleValues);
    if (!match) return null;
    const setIcons = ICON_SETS[format.icons.set];
    return { icon: iconColorForRule(match.rule, match.index, setIcons) };
  }

  // target === 'background' | 'fontColor', mode === 'rules' | 'gradient'
  const num = toNumber(value);
  if (num === null) return null;

  let color: string | undefined;
  if (format.mode === 'gradient' && format.gradient) {
    color = gradientColor(format.gradient, num, visibleValues);
  } else if (format.mode === 'rules' && format.rules) {
    const match = firstMatchingRule(format.rules, num, visibleValues);
    color = match?.rule.color;
  }
  if (!color) return null;
  return format.target === 'fontColor' ? { color } : { background: color };
}

/**
 * Converte a formatação condicional lida do PBIR (ver
 * `PbiConditionalFormatMeta` em src/bridge/tauriBridge.ts, espelhando
 * `PbiConditionalFormatMeta` em src-tauri/src/pbi/mod.rs) para
 * `ConditionalFormat[]` do modelo PrismBI. Usado pelo importador (ex.:
 * PowerBiImportModal.tsx) ao montar `VisualFormatConfig.conditionalFormats`.
 */
export function conditionalFormatsFromPbiMeta(
  pbiFormats: Array<{
    target: string;
    applyTo: string;
    mode: string;
    rules?: Array<{ operator: string; value: number; value2?: number | null; color: string }>;
    gradientMin?: { kind: string; value?: number | null; color: string } | null;
    gradientMid?: { kind: string; value?: number | null; color: string } | null;
    gradientMax?: { kind: string; value?: number | null; color: string } | null;
    dataBarPositive?: string | null;
    dataBarNegative?: string | null;
  }> | undefined,
  idPrefix: string
): ConditionalFormat[] {
  if (!pbiFormats || pbiFormats.length === 0) return [];
  const out: ConditionalFormat[] = [];
  pbiFormats.forEach((pf, idx) => {
    const target = pf.target as ConditionalFormat['target'];
    const id = `${idPrefix}_${idx}`;
    if (target === 'dataBar') {
      if (!pf.dataBarPositive && !pf.dataBarNegative) return;
      out.push({
        id,
        target,
        applyTo: pf.applyTo,
        mode: 'rules',
        dataBar: {
          positiveColor: pf.dataBarPositive || '#3b82f6',
          negativeColor: pf.dataBarNegative || '#ef4444',
        },
      });
      return;
    }
    if (pf.mode === 'gradient' && pf.gradientMin && pf.gradientMax) {
      out.push({
        id,
        target,
        applyTo: pf.applyTo,
        mode: 'gradient',
        gradient: {
          min: { kind: pf.gradientMin.kind as CondFormatStop['kind'], value: pf.gradientMin.value ?? undefined, color: pf.gradientMin.color },
          mid: pf.gradientMid
            ? { kind: pf.gradientMid.kind as CondFormatStop['kind'], value: pf.gradientMid.value ?? undefined, color: pf.gradientMid.color }
            : undefined,
          max: { kind: pf.gradientMax.kind as CondFormatStop['kind'], value: pf.gradientMax.value ?? undefined, color: pf.gradientMax.color },
        },
      });
      return;
    }
    if (pf.mode === 'rules' && pf.rules && pf.rules.length > 0) {
      out.push({
        id,
        target,
        applyTo: pf.applyTo,
        mode: 'rules',
        rules: pf.rules.map(r => ({
          operator: r.operator as ConditionalFormatRule['operator'],
          value: r.value,
          value2: r.value2 ?? undefined,
          color: r.color,
        })),
      });
    }
    // Formatos sem rules nem gradiente reconhecidos (ex.: cor fixa) já são
    // filtrados no lado Rust e não chegam aqui.
  });
  return out;
}

/**
 * Combina todos os `ConditionalFormat` aplicáveis a `field`, um por `target`
 * (o primeiro da lista que casar vence). `resolveValue` dá, para cada
 * formato, o valor usado para decidir a cor (resolve `basedOn` quando
 * presente) e `resolveVisibleValues` dá os valores visíveis equivalentes
 * para min/max/percentil.
 */
export function computeCellFormat(
  formats: ConditionalFormat[],
  field: string,
  resolveValue: (format: ConditionalFormat) => unknown,
  resolveVisibleValues: (format: ConditionalFormat) => number[]
): FormatResult {
  const applicable = formats.filter(f => f.applyTo === field);
  const seenTargets = new Set<string>();
  const result: FormatResult = {};
  for (const format of applicable) {
    if (seenTargets.has(format.target)) continue;
    seenTargets.add(format.target);
    const value = resolveValue(format);
    const visible = resolveVisibleValues(format);
    const r = computeFormatForValue(format, value, visible);
    if (!r) continue;
    Object.assign(result, r);
  }
  return result;
}
