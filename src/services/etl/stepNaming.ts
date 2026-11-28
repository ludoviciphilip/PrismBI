// Nomes únicos de etapa e de coluna — como o Power BI.
//
// O Power BI nunca deixa duas etapas com o mesmo nome (são identificadores no
// M, `#"Nome"`): a segunda etapa "Linhas Filtradas" vira "Linhas Filtradas1".
// O mesmo vale para colunas: adicionar uma coluna com um nome já existente na
// tabela produz "Mesclado.1" (note o PONTO, diferente do sufixo de etapa).
//
// Isso importa sobretudo para consultas IMPORTADAS de M (.pbip): o parser
// (`etl_from_m`) monta as etapas direto, sem passar pelo `addQueryStep` do
// store (que já dedupa nomes de etapa para etapas criadas na UI). Duas
// etapas com o mesmo nome fazem a etapa seguinte se "autorreferenciar" no M
// gerado (`Table.AddColumn(#"Mesclado Adicionado", ...)` dentro da própria
// etapa "Mesclado Adicionado"), e duas colunas-destino iguais fazem a coluna
// mais nova sobrescrever/duplicar a mais antiga na grade.

import { EtlStep } from '../../types/etl';

/** "Linhas Filtradas" -> "Linhas Filtradas1" se já existir (como o Power BI). */
export function uniqueStepName(name: string, existing: string[]): string {
  const taken = new Set(existing.map(n => n.toLowerCase()));
  if (!taken.has(name.toLowerCase()) && name.toLowerCase() !== 'source') return name;
  for (let i = 1; ; i++) {
    const candidate = `${name}${i}`;
    if (!taken.has(candidate.toLowerCase())) return candidate;
  }
}

/** "Mesclado" -> "Mesclado.1" se já existir entre as colunas conhecidas (como o Power BI). */
export function uniqueColumnName(name: string, existing: Iterable<string>): string {
  const taken = new Set(Array.from(existing, n => n.toLowerCase()));
  if (!taken.has(name.toLowerCase())) return name;
  for (let i = 1; ; i++) {
    const candidate = `${name}.${i}`;
    if (!taken.has(candidate.toLowerCase())) return candidate;
  }
}

/** Nome da coluna nova que a etapa introduz, se houver (ops com campo `newColumn`). */
function addedColumnOf(step: EtlStep): string | undefined {
  switch (step.op) {
    case 'duplicateColumn':
    case 'mergeColumns':
    case 'addCustomColumn':
    case 'addConditionalColumn':
    case 'addIndexColumn':
      return step.newColumn;
    default:
      return undefined;
  }
}

/** Reaplica o `newColumn` já deduplicado na etapa (cópia imutável). */
function withAddedColumn(step: EtlStep, newColumn: string): EtlStep {
  switch (step.op) {
    case 'duplicateColumn':
    case 'mergeColumns':
    case 'addCustomColumn':
    case 'addConditionalColumn':
    case 'addIndexColumn':
      return { ...step, newColumn };
    default:
      return step;
  }
}

/** Conjunto de colunas resultante depois da etapa — aproximação suficiente para detectar colisões. */
function columnsAfterStep(cols: string[], step: EtlStep): string[] {
  switch (step.op) {
    case 'renameColumns': {
      const map = new Map(step.renames);
      return cols.map(c => map.get(c) ?? c);
    }
    case 'removeColumns':
      return cols.filter(c => !step.columns.includes(c));
    case 'selectColumns':
      return step.columns.slice();
    case 'duplicateColumn':
      return [...cols, step.newColumn];
    case 'splitColumn':
      return [...cols.filter(c => c !== step.column), ...step.into];
    case 'mergeColumns':
      return [...cols, step.newColumn];
    case 'addCustomColumn':
      return [...cols, step.newColumn];
    case 'addConditionalColumn':
      return [...cols, step.newColumn];
    case 'addIndexColumn':
      return [...cols, step.newColumn];
    case 'unpivotOtherColumns':
      return [...step.keep, step.attributeColumn, step.valueColumn];
    case 'unpivotColumns':
      return [...cols.filter(c => !step.columns.includes(c)), step.attributeColumn, step.valueColumn];
    case 'groupBy':
      return [...step.keys, ...step.aggregations.map(a => a.name)];
    case 'reorderColumns':
      return step.columns.slice();
    default:
      return cols;
  }
}

/**
 * Garante nomes únicos de etapa e de coluna numa lista de etapas IMPORTADA
 * (M de .pbip), na mesma ordem em que o Power BI as aplicaria.
 * `baseColumns` são as colunas da tabela bruta antes de qualquer etapa.
 */
export function dedupeImportedSteps(steps: EtlStep[], baseColumns: string[]): EtlStep[] {
  const usedStepNames: string[] = [];
  let cols = baseColumns.slice();
  const result: EtlStep[] = [];
  for (const raw of steps) {
    const name = uniqueStepName(raw.name, usedStepNames);
    usedStepNames.push(name);
    let step: EtlStep = name === raw.name ? raw : ({ ...raw, name } as EtlStep);

    const added = addedColumnOf(step);
    if (added !== undefined) {
      const uniqued = uniqueColumnName(added, cols);
      if (uniqued !== added) step = withAddedColumn(step, uniqued);
    }

    result.push(step);
    cols = columnsAfterStep(cols, step);
  }
  return result;
}
