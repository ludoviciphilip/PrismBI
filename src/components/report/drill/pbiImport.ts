import { VisualDataConfig, VisualType } from '../../../types/bi';
import { DRILLABLE_VISUAL_TYPES } from '../../../engine/drill';

/**
 * v0.10 Frente G: na importação PBIR, várias projeções em Category (gráficos) ou
 * Rows (matriz) representam uma hierarquia. O Rust devolve a primeira em
 * `categoryField` e as seguintes em `drillFields`; aqui elas viram `drillFields`
 * (gráficos) ou `matrixRowFields` (matriz). `drillMode` não é gravado: no Power BI
 * uma coluna de data usada "crua" (sem a variação Date Hierarchy) aparece crua, e
 * a variação já chega como colunas explícitas (Ano, Mês...) em `drillFields`.
 */
export function drillConfigFromPbiMeta(
  meta: { categoryField?: string | null; drillFields?: string[] | null },
  type: VisualType
): Partial<VisualDataConfig> {
  const extra = (meta.drillFields || []).filter(Boolean);
  if (!meta.categoryField || extra.length === 0) return {};
  if (type === 'matrix') return { matrixRowFields: [meta.categoryField, ...extra] };
  if (DRILLABLE_VISUAL_TYPES.has(type)) return { drillFields: extra };
  return {};
}
