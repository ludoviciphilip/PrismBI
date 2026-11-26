/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import { useBiStore } from '../../../store/useBiStore';
import { VisualCard } from '../VisualCard';
import { Table, Visual } from '../../../types/bi';

afterEach(() => cleanup());

const table: Table = {
  id: 't1',
  name: 'Vendas',
  originalName: 'Vendas',
  columns: [{ id: 'c1', name: 'Valor', originalName: 'Valor', dataType: 'number' }],
  rows: [{ Valor: 10 }],
} as Table;

function seed() {
  const st = useBiStore.getState();
  st.newProject('Bookmark Button Test');
  st.setTables([table]);
  const pageId = useBiStore.getState().activePageId;

  const otherVisual: Visual = {
    id: 'v_hidden_target',
    type: 'card',
    title: 'Alvo',
    x: 0, y: 0, w: 100, h: 100,
    dataConfig: { tableId: 't1', yAxisFields: ['Valor'], aggregation: 'SUM' },
    formatConfig: { showTitle: true, hidden: false },
  };
  useBiStore.setState(s => ({
    pages: s.pages.map(p => (p.id === pageId ? { ...p, visuals: [otherVisual] } : p)),
    selectedVisualIds: ['v_hidden_target'],
  }));

  // Captura um indicador com o visual alvo VISÍVEL (scope = visuais selecionados).
  useBiStore.getState().addBookmark('Mostrar Alvo');
  const bookmarkId = useBiStore.getState().bookmarks.find(b => b.name === 'Mostrar Alvo')!.id;

  // Agora oculta de novo e cria o botão de ação apontando pro indicador.
  useBiStore.setState(s => ({
    pages: s.pages.map(p =>
      p.id === pageId
        ? {
            ...p,
            visuals: [
              { ...p.visuals[0], formatConfig: { ...p.visuals[0].formatConfig, hidden: true } },
              {
                id: 'v_button',
                type: 'action-button',
                title: 'Mostrar',
                x: 0, y: 0, w: 100, h: 40,
                dataConfig: { tableId: 't1' },
                formatConfig: { showTitle: true, actionType: 'bookmark', actionBookmarkId: bookmarkId },
              } as Visual,
            ],
          }
        : p
    ),
  }));

  return { pageId, bookmarkId };
}

describe('Botão de ação com actionType "bookmark" aplica o indicador', () => {
  it('aplica em Modo Apresentação com um clique simples', () => {
    seed();
    useBiStore.setState({ isPresentationMode: true });
    const button = useBiStore.getState().pages[0].visuals.find(v => v.type === 'action-button')!;
    render(<VisualCard visual={button} isSelected={false} onSelect={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /Mostrar/i }));

    const target = useBiStore.getState().pages[0].visuals.find(v => v.id === 'v_hidden_target');
    expect(target?.formatConfig.hidden).toBe(false);
  });

  it('em edição, um clique simples NÃO aplica; precisa de Ctrl+clique', () => {
    seed();
    useBiStore.setState({ isPresentationMode: false });
    const button = useBiStore.getState().pages[0].visuals.find(v => v.type === 'action-button')!;
    render(<VisualCard visual={button} isSelected={false} onSelect={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /Mostrar/i }));
    expect(useBiStore.getState().pages[0].visuals.find(v => v.id === 'v_hidden_target')?.formatConfig.hidden).toBe(true);

    fireEvent.click(screen.getByRole('button', { name: /Mostrar/i }), { ctrlKey: true });
    expect(useBiStore.getState().pages[0].visuals.find(v => v.id === 'v_hidden_target')?.formatConfig.hidden).toBe(false);
  });
});
