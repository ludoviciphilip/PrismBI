import { Page, Visual } from '../../types/bi';

/**
 * v0.10 Frente G: a matriz de 2 níveis do v0.4 guardava as linhas em
 * `xAxisField` (nível 1) + `drillFields[0]` (sub-linha). A matriz com N níveis lê
 * `matrixRowFields`; a migração copia exatamente esses 2 níveis para lá (a matriz
 * antiga ignorava `drillFields[1..]`, então a migrada também), preservando os
 * campos antigos. Gráficos não mudam: a hierarquia de data automática só vale para
 * visuais com `drillMode` (criados no v0.10+), então os antigos abrem iguais.
 */
function migrateVisual(visual: Visual): Visual {
  const dc = visual.dataConfig;
  if (visual.type !== 'matrix' || !dc) return visual;
  if (Array.isArray(dc.matrixRowFields) && dc.matrixRowFields.length > 0) return visual;
  const child = dc.drillFields?.[0];
  if (!dc.xAxisField || !child) return visual;
  return { ...visual, dataConfig: { ...dc, matrixRowFields: [dc.xAxisField, child] } };
}

/**
 * v0.11 Rodada 2 (Frente I): a matriz só tinha 1 nível nas colunas — a legenda
 * (`legendField`). Vira o 1º nível de `matrixColumnFields`, preservando
 * `legendField` (exportação/outros leitores continuam funcionando). Só roda
 * quando `matrixColumnFields` ainda não existe (idempotente: não sobrescreve
 * uma matriz já editada no v0.11+, mesmo que só tenha 1 nível de coluna).
 */
function migrateMatrixColumns(visual: Visual): Visual {
  const dc = visual.dataConfig;
  if (visual.type !== 'matrix' || !dc) return visual;
  if (Array.isArray(dc.matrixColumnFields)) return visual;
  if (!dc.legendField) return visual;
  return { ...visual, dataConfig: { ...dc, matrixColumnFields: [dc.legendField] } };
}

export function migrateDrillConfigs(pages: Page[]): Page[] {
  if (!Array.isArray(pages)) return pages;
  return pages.map(page => {
    if (!Array.isArray(page.visuals)) return page;
    const visuals = page.visuals.map(migrateVisual).map(migrateMatrixColumns);
    return visuals.every((v, i) => v === page.visuals[i]) ? page : { ...page, visuals };
  });
}
