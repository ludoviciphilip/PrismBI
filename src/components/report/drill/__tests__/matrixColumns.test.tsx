// @vitest-environment jsdom
/**
 * v0.11 Rodada 2 (Frente I): matriz com N níveis nas COLUNAS — cabeçalho
 * multinível (células mescladas por grupo), +/− por grupo, "expandir
 * tudo"/"recolher tudo" nas colunas e indicadores (Bookmarks) restaurando a
 * expansão. Mesma base de dados/medidas do teste de linhas N níveis
 * (`drillUi.test.tsx`), com `Regiao`/`Cidade` também nas colunas.
 */
import React from 'react';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { render, screen, fireEvent, cleanup, within, act } from '@testing-library/react';

beforeAll(() => {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1366 });
  Object.defineProperty(window, 'innerHeight', { configurable: true, value: 768 });
});
afterEach(() => cleanup());

import { useBiStore } from '../../../../store/useBiStore';
import { globalDataEngine } from '../../../../engine/dataEngine';
import { VisualCard } from '../../VisualCard';
import { Measure, Table, Visual } from '../../../../types/bi';

const lojas: Table = {
  id: 'fv',
  name: 'fVendas',
  originalName: 'fVendas',
  columns: ['Regiao', 'Cidade', 'Loja', 'Cliente'].map((n, i) => ({ id: `k${i}`, name: n, originalName: n, dataType: 'text' as const })),
  rows: [
    ['Sul', 'POA', 'L1', 'c1'],
    ['Sul', 'POA', 'L1', 'c2'],
    ['Sul', 'POA', 'L2', 'c1'],
    ['Sul', 'Curitiba', 'L3', 'c1'],
    ['Sul', 'Curitiba', 'L3', 'c3'],
    ['Norte', 'Manaus', 'L4', 'c2'],
    ['Norte', 'Manaus', 'L4', 'c4'],
    ['Norte', 'Belem', 'L5', 'c1'],
  ].map(([Regiao, Cidade, Loja, Cliente]) => ({ Regiao, Cidade, Loja, Cliente })),
} as Table;

const measures: Measure[] = [{ id: 'm2', name: 'Clientes', formula: 'DISTINCTCOUNT(fVendas[Cliente])', tableName: 'fVendas', format: 'number' }];

function seed(visual: Visual) {
  const st = useBiStore.getState();
  st.newProject('Matrix Columns UI');
  st.setTables([lojas]);
  useBiStore.setState({ measures, filters: [] });
  globalDataEngine.setSchema([lojas], [], measures);
  const pageId = useBiStore.getState().activePageId;
  useBiStore.setState(s => ({ pages: [{ ...s.pages[0], id: pageId, visuals: [visual] }] }));
}

const LiveCard: React.FC<{ id: string }> = ({ id }) => {
  const v = useBiStore(s => s.pages.flatMap(p => p.visuals).find(x => x.id === id))!;
  return <VisualCard visual={v} isSelected={false} onSelect={() => {}} />;
};
const getVisual = () => useBiStore.getState().pages.flatMap(p => p.visuals).find(v => v.id === 'v1')!;

const matrixVisual = (over: Partial<Visual['dataConfig']> = {}): Visual => ({
  id: 'v1',
  type: 'matrix',
  title: 'Clientes por regiao/cidade',
  x: 0,
  y: 0,
  w: 700,
  h: 420,
  dataConfig: {
    tableId: 'fv',
    xAxisField: 'Loja',
    matrixRowFields: ['Loja'],
    matrixColumnFields: ['Regiao', 'Cidade'],
    yAxisFields: ['Clientes'],
    aggregation: 'NONE',
    ...over,
  },
  formatConfig: { showTitle: true },
});

const matrixEl = () => screen.getByTestId('hierarchical-matrix');
const headerRows = () => Array.from(matrixEl().querySelectorAll('thead tr'));
const headerTexts = (tr: Element) => Array.from(tr.querySelectorAll('th')).map(th => th.textContent?.trim().replace(/\s+/g, ' '));

describe('v0.11 Rodada 2 (Frente I): cabeçalho multinível das colunas da matriz', () => {
  it('cabeçalho recolhido tem 1 linha; expandir um grupo acrescenta a 2ª linha (células mescladas por grupo)', () => {
    seed(matrixVisual());
    render(<LiveCard id="v1" />);
    const rows = headerRows();
    // Por padrão a matriz abre recolhida: só 1 linha de cabeçalho é necessária
    // (as células recolhidas usam `rowSpan` para "alcançar" as linhas de baixo).
    expect(rows).toHaveLength(1);
    expect(headerTexts(rows[0])).toEqual(['Loja', 'Norte', 'Sul', 'Total']);
    const norteTh = Array.from(rows[0].querySelectorAll('th')).find(th => th.textContent?.includes('Norte'))!;
    expect(norteTh.getAttribute('rowspan')).toBe('2'); // recolhido: ocupa as 2 linhas do cabeçalho
    const rowFieldTh = rows[0].querySelector('th')!;
    expect(rowFieldTh.getAttribute('rowspan')).toBe('2');

    // Expande "Norte": ganha uma 2ª linha de cabeçalho com as cidades de Norte.
    fireEvent.click(screen.getByRole('button', { name: /Expandir coluna Norte/ }));
    const rows2 = headerRows();
    expect(rows2).toHaveLength(2);
    expect(headerTexts(rows2[0])).toEqual(['Loja', 'Norte', 'Sul', 'Total']);
    const norteExpanded = Array.from(rows2[0].querySelectorAll('th')).find(th => th.textContent?.includes('Norte'))!;
    expect(norteExpanded.getAttribute('colspan')).toBe('2'); // Belem + Manaus
    expect(norteExpanded.getAttribute('rowspan')).toBe('1'); // expandido: só a 1ª linha
    expect(headerTexts(rows2[1])).toEqual(['Belem', 'Manaus']); // "Sul" continua recolhido: sem entrada na 2ª linha
  });

  it('subtotal de coluna NUNCA é soma dos filhos (DISTINCTCOUNT): Norte=3 recolhido ou expandido', () => {
    seed(matrixVisual());
    render(<LiveCard id="v1" />);
    // Recolhido: coluna "Norte" já é o subtotal pelo motor.
    expect(screen.getByTestId('matrix-total-Norte').textContent).toBe('3'); // c1,c2,c4 — não 2 (Manaus) + 1 (Belem) = 3 (coincide aqui, mas vem do motor)
    expect(screen.getByTestId('matrix-total-Sul').textContent).toBe('3'); // c1,c2,c3
  });

  it('expandir/recolher tudo nas colunas (cabeçalho ao passar o mouse) e alternar Linhas/Colunas', () => {
    seed(matrixVisual());
    render(<LiveCard id="v1" />);
    const card = document.querySelector('[data-visual-id="v1"]') as HTMLElement;
    fireEvent.mouseEnter(card);

    const toggle = screen.getByTestId('matrix-axis-toggle');
    expect(toggle.textContent).toBe('Linhas'); // padrão: controles afetam as LINHAS
    fireEvent.click(toggle);
    expect(toggle.textContent).toBe('Colunas');

    fireEvent.click(screen.getByRole('button', { name: 'Expandir tudo' }));
    expect(headerTexts(headerRows()[1])).toEqual(['Belem', 'Manaus', 'Curitiba', 'POA']);

    fireEvent.click(screen.getByRole('button', { name: 'Recolher tudo' }));
    expect(headerRows()).toHaveLength(1);
    expect(headerTexts(headerRows()[0])).toEqual(['Loja', 'Norte', 'Sul', 'Total']);
  });

  it('seams E↔G: um indicador grava dataConfig.matrixColumnExpansion e a matriz se re-expande', () => {
    seed(matrixVisual());
    render(<LiveCard id="v1" />);
    expect(headerTexts(headerRows()[0])).toEqual(['Loja', 'Norte', 'Sul', 'Total']);

    act(() => {
      useBiStore.getState().updateVisual('v1', {
        dataConfig: { ...getVisual().dataConfig, matrixColumnExpansion: { depth: 0, expanded: [JSON.stringify(['Norte'])], collapsed: [] } },
      });
    });
    const rows = headerRows();
    const norteExpanded = Array.from(rows[0].querySelectorAll('th')).find(th => th.textContent?.includes('Norte'))!;
    expect(norteExpanded.getAttribute('colspan')).toBe('2');
  });

  it('formatação condicional nos subtotais de coluna (Norte/Sul) usa a própria escala', () => {
    seed(matrixVisual());
    const v = getVisual();
    v.formatConfig = {
      showTitle: true,
      conditionalFormats: [
        { id: 'f1', target: 'background', applyTo: 'Clientes', mode: 'rules', applyToTotals: true, rules: [{ operator: '>=', value: 3, color: '#00ff00' }] } as any,
      ],
    };
    useBiStore.setState(s => ({ pages: s.pages.map(p => ({ ...p, visuals: p.visuals.map(x => (x.id === 'v1' ? v : x)) })) }));
    render(<LiveCard id="v1" />);
    expect(screen.getByTestId('matrix-total-Norte').style.backgroundColor).toBe('rgb(0, 255, 0)');
    expect(screen.getByTestId('matrix-total-Sul').style.backgroundColor).toBe('rgb(0, 255, 0)');
  });
});
