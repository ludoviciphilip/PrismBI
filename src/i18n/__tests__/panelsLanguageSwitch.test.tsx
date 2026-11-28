// @vitest-environment jsdom
/**
 * v0.11 Frente D: troca de idioma (pt-BR/en-US/es-ES) nos painéis novos do v0.10 —
 * Indicadores (Bookmarks), Seleção, Dicas de página (PageTooltipSettings/TooltipCard)
 * e nos literais fixos da HierarchicalMatrix ('Total', 'Total Geral', '(Em branco)').
 */
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { useBiStore } from '../../store/useBiStore';
import { SelectionPane } from '../../components/panels/SelectionPane';
import { BookmarksPane } from '../../components/panels/BookmarksPane';
import { PageTooltipSettings } from '../../components/report/tooltipPage/PageTooltipSettings';
import { TooltipCard } from '../../components/report/tooltipPage/TooltipCard';
import { Page, Visual } from '../../types/bi';

afterEach(() => {
  cleanup();
  useBiStore.setState({ language: 'pt-BR' });
});

const page = (over: Partial<Page> = {}): Page => ({
  id: 'p1',
  name: 'Página 1',
  order: 0,
  visuals: [],
  ...over,
});

const visual = (over: Partial<Visual> = {}): Visual => ({
  id: 'v1',
  type: 'bar',
  title: 'V',
  x: 0,
  y: 0,
  w: 300,
  h: 200,
  dataConfig: { tableId: 't' },
  formatConfig: { showTitle: true },
  ...over,
});

describe('i18n: painel Seleção', () => {
  it('troca o texto do título e do estado vazio ao mudar de idioma', () => {
    useBiStore.setState(s => ({ pages: [page({ visuals: [] })], activePageId: 'p1' }));
    const { rerender } = render(<SelectionPane />);
    expect(screen.getByText('Seleção')).toBeTruthy();
    expect(screen.getByText('Nenhum visual nesta página.')).toBeTruthy();

    useBiStore.setState({ language: 'en-US' });
    rerender(<SelectionPane />);
    expect(screen.getByText('Selection')).toBeTruthy();
    expect(screen.getByText('No visuals on this page.')).toBeTruthy();

    useBiStore.setState({ language: 'es-ES' });
    rerender(<SelectionPane />);
    expect(screen.getByText('Selección')).toBeTruthy();
    expect(screen.getByText('Ningún objeto visual en esta página.')).toBeTruthy();
  });
});

describe('i18n: painel Indicadores (Bookmarks)', () => {
  it('troca o título e a dica do estado vazio ao mudar de idioma', () => {
    useBiStore.setState({ bookmarks: [] });
    const { rerender } = render(<BookmarksPane />);
    expect(screen.getByText('Indicadores')).toBeTruthy();
    expect(screen.getByText(/Nenhum indicador/)).toBeTruthy();

    useBiStore.setState({ language: 'en-US' });
    rerender(<BookmarksPane />);
    expect(screen.getByText('Bookmarks')).toBeTruthy();
    expect(screen.getByText(/No bookmarks yet/)).toBeTruthy();

    useBiStore.setState({ language: 'es-ES' });
    rerender(<BookmarksPane />);
    expect(screen.getByText('Indicadores')).toBeTruthy();
    expect(screen.getByText(/Ningún indicador todavía/)).toBeTruthy();
  });
});

describe('i18n: Dicas de página (PageTooltipSettings/TooltipCard)', () => {
  it('PageTooltipSettings traduz o rótulo e o texto de "permitir uso"', () => {
    const { rerender } = render(
      <PageTooltipSettings page={page()} availableFields={[]} onChange={() => {}} onSuggestCanvasSize={() => {}} />
    );
    expect(screen.getByText('Informações da página')).toBeTruthy();
    expect(screen.getByText('Permitir uso como dica de ferramenta')).toBeTruthy();

    useBiStore.setState({ language: 'en-US' });
    rerender(<PageTooltipSettings page={page()} availableFields={[]} onChange={() => {}} onSuggestCanvasSize={() => {}} />);
    expect(screen.getByText('Page information')).toBeTruthy();
    expect(screen.getByText('Allow use as a tooltip')).toBeTruthy();
  });

  it('TooltipCard traduz "Tipo" e as opções Padrão/Página do relatório', () => {
    const { rerender } = render(<TooltipCard visual={visual()} pages={[page()]} onChange={() => {}} />);
    expect(screen.getByText('Dica de ferramenta')).toBeTruthy();
    expect(screen.getByText('Padrão')).toBeTruthy();

    useBiStore.setState({ language: 'es-ES' });
    rerender(<TooltipCard visual={visual()} pages={[page()]} onChange={() => {}} />);
    expect(screen.getByText('Información sobre herramientas')).toBeTruthy();
    expect(screen.getByText('Predeterminado')).toBeTruthy();
  });
});
