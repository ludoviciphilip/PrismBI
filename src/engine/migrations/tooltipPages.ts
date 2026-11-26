import { Page } from '../../types/bi';

/**
 * Normaliza `Page.tooltipPage` e `Visual.formatConfig.tooltip` (v0.10 Rodada 2,
 * frente F) para projetos salvos antes desses campos existirem — ambos são
 * opcionais, então nenhum dado é perdido; isto só garante formas consistentes
 * (ex.: `fields` sempre array quando `enabled`, `pageId` padrão `'auto'`
 * quando `type === 'page'`) para o resto do código não precisar checar undefined
 * em todo canto.
 */
export function migrateTooltipPages(pages: Page[]): Page[] {
  if (!Array.isArray(pages)) return pages;

  return pages.map(page => {
    const alreadyNormalized =
      !page.tooltipPage || (typeof page.tooltipPage.enabled === 'boolean' && Array.isArray(page.tooltipPage.fields));
    const tooltipPage = alreadyNormalized
      ? page.tooltipPage
      : { enabled: Boolean(page.tooltipPage!.enabled), fields: Array.isArray(page.tooltipPage!.fields) ? page.tooltipPage!.fields : [] };

    const visuals = Array.isArray(page.visuals) && page.visuals.length > 0
      ? page.visuals.map(visual => {
          const tooltip = visual.formatConfig?.tooltip;
          if (!tooltip || tooltip.type !== 'page') return visual;
          if (tooltip.pageId) return visual;
          return {
            ...visual,
            formatConfig: { ...visual.formatConfig, tooltip: { ...tooltip, pageId: 'auto' as const } },
          };
        })
      : page.visuals;

    if (tooltipPage === page.tooltipPage && visuals === page.visuals) return page;
    return { ...page, tooltipPage, visuals };
  });
}
