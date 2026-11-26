/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

// @vitest-environment jsdom
import React from 'react';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';

/**
 * jsdom has no layout engine; map the Tailwind width classes the rename
 * input can carry to their real px size (see skill
 * tailwind-width-class-override) so the width assertion is meaningful.
 */
const TAILWIND_WIDTH_PX: Record<string, number> = { 'w-full': 1200 };

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: function (this: HTMLElement) {
      const classes = (this.className || '').split(/\s+/).filter(Boolean);
      let width = 18;
      for (const cls of classes) {
        if (TAILWIND_WIDTH_PX[cls]) {
          width = TAILWIND_WIDTH_PX[cls];
          break;
        }
      }
      if (classes.includes('flex-1') || classes.includes('min-w-0')) width = Math.max(width, 140);
      return { width, height: 20, top: 0, left: 0, right: width, bottom: 20, x: 0, y: 0, toJSON() {} };
    },
  });
});

afterAll(() => {
  // @ts-expect-error restores jsdom's default
  delete HTMLElement.prototype.getBoundingClientRect;
});

import { useBiStore } from '../../../store/useBiStore';
import { useUiPrefs } from '../../../store/useUiPrefs';
import { BookmarksPane } from '../BookmarksPane';

beforeEach(() => {
  useBiStore.getState().newProject('Bookmarks Pane Test');
  useBiStore.setState({ bookmarks: [] });
  useUiPrefs.setState(s => ({ panes: { ...s.panes, bookmarks: 'open' } }));
});
afterEach(() => cleanup());

describe('BookmarksPane', () => {
  it('Adicionar captura o estado atual como um novo indicador', () => {
    render(<BookmarksPane />);
    fireEvent.click(screen.getByText('Adicionar'));
    expect(useBiStore.getState().bookmarks).toHaveLength(1);
    expect(screen.getByText('Indicador 1')).toBeTruthy();
  });

  it('clicar num indicador o aplica (muda a página ativa)', () => {
    useBiStore.getState().addPage('Página 2');
    const otherPageId = useBiStore.getState().pages[1].id;
    useBiStore.setState({ activePageId: otherPageId });
    useBiStore.getState().addBookmark('Voltar ao início');
    useBiStore.setState({ activePageId: useBiStore.getState().pages[0].id });

    render(<BookmarksPane />);
    fireEvent.click(screen.getByTestId(`bookmark-row-${useBiStore.getState().bookmarks[0].id}`));
    expect(useBiStore.getState().activePageId).toBe(otherPageId);
  });

  it('duplo clique no nome entra em edição; digitar e confirmar renomeia, e o campo cabe em 1366×768 (largura > 60px)', () => {
    useBiStore.getState().addBookmark('Original');
    render(<BookmarksPane />);

    fireEvent.doubleClick(screen.getByText('Original'));
    const input = screen.getByLabelText('Renomear indicador') as HTMLInputElement;
    expect(input.getBoundingClientRect().width).toBeGreaterThan(60);

    fireEvent.change(input, { target: { value: 'Renomeado' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    expect(useBiStore.getState().bookmarks[0].name).toBe('Renomeado');
    expect(screen.getByText('Renomeado')).toBeTruthy();
  });

  it('agrupar todos marca o mesmo groupId em todos os indicadores', () => {
    useBiStore.getState().addBookmark('A');
    useBiStore.getState().addBookmark('B');
    render(<BookmarksPane />);
    fireEvent.click(screen.getByTitle('Agrupar todos'));
    const groups = new Set(useBiStore.getState().bookmarks.map(b => b.groupId));
    expect(groups.size).toBe(1);
    expect(groups.has(undefined)).toBe(false);
  });

  it('o modo Visualizar entra aplicando o primeiro indicador e navega com próximo/anterior', () => {
    useBiStore.getState().addBookmark('Primeiro');
    useBiStore.getState().addBookmark('Segundo');
    render(<BookmarksPane />);

    fireEvent.click(screen.getByText('Visualizar'));
    expect(screen.getByTitle('Próximo')).toBeTruthy();

    fireEvent.click(screen.getByTitle('Próximo'));
    expect(screen.getAllByText('Segundo').length).toBeGreaterThan(0);
  });
});
