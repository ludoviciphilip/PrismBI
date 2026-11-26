import { describe, expect, it } from 'vitest';
import { visiblePagesForPresentation } from '../navigation';
import type { Page } from '../../../../types/bi';

const pages: Page[] = [
  { id: 'p_home', name: 'Início', order: 1, visuals: [] },
  { id: 'p_tooltip', name: 'Dica Região', order: 2, visuals: [], tooltipPage: { enabled: true, fields: ['Regiao'] } },
  { id: 'p_tooltip_disabled', name: 'Dica Antiga', order: 3, visuals: [], tooltipPage: { enabled: false } },
];

describe('visiblePagesForPresentation', () => {
  it('fora do Modo Apresentação, mostra todas as páginas (inclusive dicas)', () => {
    expect(visiblePagesForPresentation(pages, false)).toHaveLength(3);
  });

  it('no Modo Apresentação, oculta só as páginas de dica habilitadas', () => {
    const visible = visiblePagesForPresentation(pages, true);
    expect(visible.map(p => p.id)).toEqual(['p_home', 'p_tooltip_disabled']);
  });
});
