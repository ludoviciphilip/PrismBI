import { Page, Visual } from '../../../types/bi';

/**
 * Campos do visual usados para casar automaticamente com uma página de dica
 * (eixo X, legenda, medidas e os campos extras de `tooltipFields`).
 */
export function getVisualFieldSet(visual: Visual): string[] {
  const dc = visual.dataConfig;
  const fields = [dc.xAxisField, dc.legendField, ...(dc.yAxisFields || []), ...(dc.tooltipFields || [])];
  return fields.filter((f): f is string => Boolean(f));
}

/** Páginas marcadas como dica de ferramenta (Page.tooltipPage.enabled). */
export function getTooltipPages(pages: Page[]): Page[] {
  return pages.filter(p => p.tooltipPage?.enabled);
}

/**
 * Modo "Automático": escolhe, entre as páginas de dica, a que tem mais campos
 * (`tooltipPage.fields`) em comum com os campos do visual. Páginas de dica
 * sem campos declarados não entram na disputa automática (só via seleção
 * explícita), para não escolher uma página "genérica" sem relação nenhuma.
 */
export function resolveAutoTooltipPage(visual: Visual, pages: Page[]): Page | null {
  const candidates = getTooltipPages(pages);
  if (!candidates.length) return null;
  const visualFields = new Set(getVisualFieldSet(visual));
  if (!visualFields.size) return null;

  let best: Page | null = null;
  let bestScore = 0;
  for (const page of candidates) {
    const pageFields = page.tooltipPage?.fields || [];
    if (!pageFields.length) continue;
    const score = pageFields.filter(f => visualFields.has(f)).length;
    if (score > bestScore) {
      bestScore = score;
      best = page;
    }
  }
  return best;
}

/**
 * Resolve a página de dica efetiva para um visual, seguindo
 * `VisualFormatConfig.tooltip`:
 * - `type !== 'page'` (ou ausente): sem dica de página (tooltip padrão do ECharts/tabela).
 * - `pageId` explícito: usa aquela página (se ainda existir e estiver marcada como dica).
 * - `pageId === 'auto'` ou ausente: resolve pelo casamento de campos.
 */
export function resolveTooltipPage(visual: Visual, pages: Page[]): Page | null {
  const tooltip = visual.formatConfig?.tooltip;
  if (!tooltip || tooltip.type !== 'page') return null;

  if (tooltip.pageId && tooltip.pageId !== 'auto') {
    const explicit = pages.find(p => p.id === tooltip.pageId);
    return explicit && explicit.tooltipPage?.enabled ? explicit : null;
  }

  return resolveAutoTooltipPage(visual, pages);
}
