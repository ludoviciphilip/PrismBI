/**
 * v0.11 Frente F: ponte entre a UI (modais de medida, DaxImportModal,
 * UdfLibraryModal, DaxQueryView) e os comandos Tauri nativos em Rust
 * (`dax_validate`/`dax_query_execute`, src-tauri/src/dax/query_view/**).
 *
 * Fallback TS: **somente no modo navegador sem o bridge Tauri**
 * (`isTauriRuntime() === false`, ex.: `pnpm dev` servindo só o Vite sem o
 * app desktop). Nesse modo usamos o motor legado (`DaxEngine`/
 * `src/engine/**`) diretamente, pois não há processo Rust para consultar.
 * Essa é a ÚNICA razão para o motor TS continuar sendo importado pelos
 * arquivos que usam este módulo — ver o relatório final da Frente F para a
 * lista completa do que ainda chama o motor TS neste território.
 */
import { Table, Measure } from '../types/bi';
import { DaxEngine } from './daxEngine';
import type { DaxValidationResult } from './expr/validator';
import {
  isTauriRuntime,
  daxValidateNative,
  NativeDaxValidateContext,
  NativeDaxDiagnostic,
} from '../bridge/tauriBridge';

export interface UnifiedDaxValidationResult extends DaxValidationResult {
  /** Posição (1-based) do primeiro diagnóstico, quando disponível (motor nativo). */
  line?: number;
  column?: number;
  length?: number;
  diagnostics?: NativeDaxDiagnostic[];
  resultType?: string | null;
  sampleValue?: any;
  /** 'native' = validado pelo comando Tauri `dax_validate` (Rust); 'ts-fallback' = motor TS (sem Tauri). */
  engine: 'native' | 'ts-fallback';
}

/**
 * Evaluates a single DAX expression (e.g. a measure's formula) in the
 * model's unfiltered scope, for places that just want "what's the current
 * value" (AI narrative summaries, etc.) rather than full validation UI.
 * Reuses `dax_validate`'s own evaluation (context `'measure'` already
 * evaluates and returns `sampleValue` when the expression is valid) so
 * there's no separate native "evaluate" path to keep in sync.
 */
export async function evaluateMeasureDax(formula: string, tables: Table[], measures: Measure[]): Promise<any> {
  if (!isTauriRuntime()) {
    // Fallback TS: somente modo navegador sem Tauri.
    return DaxEngine.evaluateWithContext(formula, (await import('./dataEngine')).globalDataEngine);
  }
  const native = await daxValidateNative(formula, 'measure');
  if (!native.ok) {
    throw new Error(native.diagnostics[0]?.message || 'Erro ao avaliar a fórmula DAX.');
  }
  return native.sampleValue;
}

function tsFallbackValidate(formula: string, tables: Table[], measures: Measure[]): UnifiedDaxValidationResult {
  const res = DaxEngine.validate(formula, tables, measures);
  return { ...res, engine: 'ts-fallback' };
}

/**
 * Valida uma expressão DAX (medida/coluna calculada/tabela calculada/
 * cálculo visual). Usa `dax_validate` (Rust) quando o app roda dentro do
 * Tauri; cai para o motor TS só no modo navegador puro (regra 4 da
 * tarefa). `table` dá contexto de linha (RELATED, colunas da própria
 * linha) para `calculatedColumn`.
 */
export async function validateDaxFormula(
  formula: string,
  tables: Table[],
  measures: Measure[],
  context: NativeDaxValidateContext = 'measure',
  table?: string
): Promise<UnifiedDaxValidationResult> {
  if (!formula.trim()) {
    return { isValid: true, valid: true, referencedTables: [], referencedColumns: [], referencedMeasures: [], engine: 'ts-fallback' };
  }
  if (!isTauriRuntime()) {
    // Fallback TS: somente modo navegador sem Tauri (sem processo Rust para consultar).
    return tsFallbackValidate(formula, tables, measures);
  }
  try {
    const native = await daxValidateNative(formula, context, table);
    const first = native.diagnostics[0];
    const errorMessage = first ? `${first.message} (linha ${first.line}, coluna ${first.column})` : undefined;
    return {
      isValid: native.ok,
      valid: native.ok,
      errorMessage,
      error: errorMessage,
      referencedTables: [],
      referencedColumns: [],
      referencedMeasures: [],
      line: first?.line,
      column: first?.column,
      length: first?.length,
      diagnostics: native.diagnostics,
      resultType: native.resultType,
      sampleValue: native.sampleValue,
      engine: 'native',
    };
  } catch (err: any) {
    // Comando Tauri indisponível/erro de infraestrutura (não um erro de
    // validação DAX) — cai para o motor TS em vez de quebrar a UI.
    return tsFallbackValidate(formula, tables, measures);
  }
}
