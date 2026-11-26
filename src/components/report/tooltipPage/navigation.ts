import { Page } from '../../../types/bi';

/**
 * Páginas de dica de ferramenta (Page.tooltipPage.enabled) ficam ocultas na
 * navegação do Modo Apresentação — elas só existem para serem renderizadas
 * em miniatura dentro do popover de hover, nunca como um destino de
 * navegação normal.
 */
export function visiblePagesForPresentation(pages: Page[], isPresentationMode: boolean): Page[] {
  if (!isPresentationMode) return pages;
  return pages.filter(p => !p.tooltipPage?.enabled);
}
