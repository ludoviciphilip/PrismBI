import { describe, expect, it } from 'vitest';
import { resolveTooltipPage, resolveAutoTooltipPage } from '../resolve';
import type { Page, Visual } from '../../../../types/bi';

function makeVisual(overrides: Partial<Visual> = {}): Visual {
  return {
    id: 'vis_1',
    type: 'column',
    title: 'Vendas',
    x: 0, y: 0, w: 100, h: 100,
    dataConfig: { tableId: 'f_Vendas', xAxisField: 'Regiao', legendField: 'Categoria', yAxisFields: ['ValorTotal'] },
    formatConfig: { showTitle: true },
    ...overrides,
  };
}

function makeTooltipPage(id: string, fields: string[]): Page {
  return { id, name: id, order: 1, visuals: [], tooltipPage: { enabled: true, fields } };
}

describe('resolveAutoTooltipPage', () => {
  it('escolhe a página de dica com mais campos em comum com o visual', () => {
    const visual = makeVisual();
    const pages = [
      makeTooltipPage('page_weak', ['Regiao']),
      makeTooltipPage('page_strong', ['Regiao', 'Categoria']),
      makeTooltipPage('page_unrelated', ['Vendedor']),
    ];
    expect(resolveAutoTooltipPage(visual, pages)?.id).toBe('page_strong');
  });

  it('ignora páginas de dica sem campos declarados', () => {
    const visual = makeVisual();
    const pages = [makeTooltipPage('page_no_fields', [])];
    expect(resolveAutoTooltipPage(visual, pages)).toBeNull();
  });

  it('sem páginas marcadas como dica -> null', () => {
    expect(resolveAutoTooltipPage(makeVisual(), [{ id: 'p1', name: 'P1', order: 1, visuals: [] }])).toBeNull();
  });
});

describe('resolveTooltipPage', () => {
  const pages = [
    makeTooltipPage('page_auto_match', ['Regiao']),
    makeTooltipPage('page_explicit', ['Vendedor']),
  ];

  it('tipo "default" (ou ausente) -> sem dica de página', () => {
    expect(resolveTooltipPage(makeVisual(), pages)).toBeNull();
    expect(resolveTooltipPage(makeVisual({ formatConfig: { showTitle: true, tooltip: { type: 'default' } } }), pages)).toBeNull();
  });

  it('tipo "page" com pageId explícito -> usa exatamente aquela página', () => {
    const visual = makeVisual({ formatConfig: { showTitle: true, tooltip: { type: 'page', pageId: 'page_explicit' } } });
    expect(resolveTooltipPage(visual, pages)?.id).toBe('page_explicit');
  });

  it('pageId explícito apontando para página que não existe mais -> null (projeto não quebra)', () => {
    const visual = makeVisual({ formatConfig: { showTitle: true, tooltip: { type: 'page', pageId: 'page_deleted' } } });
    expect(resolveTooltipPage(visual, pages)).toBeNull();
  });

  it('pageId explícito apontando para página que não está mais marcada como dica -> null', () => {
    const notTooltip: Page = { id: 'page_plain', name: 'Plain', order: 1, visuals: [] };
    const visual = makeVisual({ formatConfig: { showTitle: true, tooltip: { type: 'page', pageId: 'page_plain' } } });
    expect(resolveTooltipPage(visual, [...pages, notTooltip])).toBeNull();
  });

  it('pageId "auto" ou ausente -> resolve pelo casamento automático de campos', () => {
    const visual = makeVisual({ formatConfig: { showTitle: true, tooltip: { type: 'page', pageId: 'auto' } } });
    expect(resolveTooltipPage(visual, pages)?.id).toBe('page_auto_match');

    const visualNoPageId = makeVisual({ formatConfig: { showTitle: true, tooltip: { type: 'page' } } });
    expect(resolveTooltipPage(visualNoPageId, pages)?.id).toBe('page_auto_match');
  });
});
