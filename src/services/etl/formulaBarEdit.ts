// Edição da barra de fórmulas (docs/ETL_V09.md, Rodada 2): troca a expressão de
// UM passo dentro do M completo (etl_to_m), sem tocar nos outros passos, então
// reconverte com etl_from_m e substitui só aquele passo na query original.
import { EtlFromMResult, EtlQuery, EtlStep } from '../../types/etl';
import { etlFromM, etlToM } from './etlClient';

export class FormulaBarEditError extends Error {}

/** Isola a linha `#"Nome" = <corpo>` de `stepName` dentro do M completo e troca o corpo. */
export function spliceStepExpression(fullM: string, stepName: string, newExpression: string): string {
  const escaped = stepName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const lineRe = new RegExp(`(^[ \\t]*#"${escaped}"\\s*=\\s*)([\\s\\S]*?)([,]?\\s*(?:\\r?\\n|$))`, 'm');
  const match = fullM.match(lineRe);
  if (!match) {
    throw new FormulaBarEditError(`Passo "${stepName}" não encontrado no M completo`);
  }
  let expr = newExpression.trim();
  if (expr.startsWith('=')) expr = expr.slice(1).trim();
  return fullM.slice(0, match.index) + match[1] + expr + match[3] + fullM.slice((match.index ?? 0) + match[0].length);
}

export interface FormulaBarEditResult {
  query: EtlQuery;
  warnings: string[];
}

/**
 * Edita a expressão do passo `stepIndex` na barra de fórmulas:
 * 1. pega o M completo (etl_to_m),
 * 2. troca só a linha daquele passo,
 * 3. reconverte (etl_from_m) e substitui só esse passo na query original (por índice),
 *    preservando o `id` para não perder a seleção.
 * Se o parse falhar, lança FormulaBarEditError e a query original não é tocada.
 */
export async function editStepFormula(query: EtlQuery, stepIndex: number, newExpression: string): Promise<FormulaBarEditResult> {
  const step = query.steps[stepIndex];
  if (!step) throw new FormulaBarEditError(`Passo ${stepIndex} não existe`);

  const fullM = await etlToM({ query });
  const splicedM = spliceStepExpression(fullM, step.name, newExpression);

  let parsed: EtlFromMResult;
  try {
    parsed = await etlFromM({ name: query.name, m: splicedM });
  } catch (e: any) {
    throw new FormulaBarEditError(e?.message ?? 'Falha ao reinterpretar o M editado');
  }

  const rebuiltStep = parsed.query.steps[stepIndex];
  if (!rebuiltStep) {
    throw new FormulaBarEditError('A edição mudou o número de passos de forma inesperada');
  }

  const nextSteps: EtlStep[] = query.steps.map((s, i) => (i === stepIndex ? { ...rebuiltStep, id: s.id } : s));
  return { query: { ...query, steps: nextSteps }, warnings: parsed.warnings };
}
