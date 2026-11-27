import { VisualDataConfig, VisualType } from '../../../types/bi';
import { DRILLABLE_VISUAL_TYPES } from '../../../engine/drill';

/**
 * v0.10 Frente G: na importação PBIR, várias projeções em Category (gráficos) ou
 * Rows (matriz) representam uma hierarquia. O Rust devolve a primeira em
 * `categoryField` e as seguintes em `drillFields`; aqui elas viram `drillFields`
 * (gráficos) ou `matrixRowFields` (matriz). `drillMode` não é gravado: no Power BI
 * uma coluna de data usada "crua" (sem a variação Date Hierarchy) aparece crua, e
 * a variação já chega como colunas explícitas (Ano, Mês...) em `drillFields`.
 *
 * v0.11 Rodada 2 (Frente I): o mesmo para as COLUNAS de uma matriz — projeções
 * extras em Columns/Series/Legend (`legendDrillFields`, depois de `legendField`)
 * viram `matrixColumnFields`.
 */
export function drillConfigFromPbiMeta(
  meta: { categoryField?: string | null; drillFields?: string[] | null; legendField?: string | null; legendDrillFields?: string[] | null },
  type: VisualType
): Partial<VisualDataConfig> {
  const extra = (meta.drillFields || []).filter(Boolean);
  const rowPatch: Partial<VisualDataConfig> =
    meta.categoryField && extra.length > 0
      ? type === 'matrix'
        ? { matrixRowFields: [meta.categoryField, ...extra] }
        : DRILLABLE_VISUAL_TYPES.has(type)
          ? { drillFields: extra }
          : {}
      : {};
  if (type !== 'matrix') return rowPatch;
  const colExtra = (meta.legendDrillFields || []).filter(Boolean);
  const columnPatch: Partial<VisualDataConfig> = meta.legendField && colExtra.length > 0 ? { matrixColumnFields: [meta.legendField, ...colExtra] } : {};
  return { ...rowPatch, ...columnPatch };
}
