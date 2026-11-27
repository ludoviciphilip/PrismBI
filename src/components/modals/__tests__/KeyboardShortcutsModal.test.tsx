// @vitest-environment jsdom
/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, within } from '@testing-library/react';
import { useBiStore } from '../../../store/useBiStore';
import { KeyboardShortcutsModal } from '../KeyboardShortcutsModal';
import { SHORTCUTS } from '../../../services/shortcuts/registry';

describe('KeyboardShortcutsModal (v0.11)', () => {
  beforeEach(() => {
    useBiStore.getState().newProject('Shortcuts Modal Test');
    useBiStore.setState({ language: 'pt-BR' as any });
  });

  afterEach(() => {
    cleanup();
    useBiStore.getState().setShortcutsModalOpen(false);
  });

  it('renders nothing when closed', () => {
    render(<KeyboardShortcutsModal />);
    expect(screen.queryByTestId('shortcuts-list')).toBeNull();
  });

  it('lists every shortcut from the registry when open', () => {
    useBiStore.getState().setShortcutsModalOpen(true);
    render(<KeyboardShortcutsModal />);

    const list = screen.getByTestId('shortcuts-list');
    for (const def of SHORTCUTS) {
      const label = def.label['pt-BR'];
      expect(within(list).getAllByText(label).length).toBeGreaterThan(0);
    }
  });

  it('search filters by label', () => {
    useBiStore.getState().setShortcutsModalOpen(true);
    render(<KeyboardShortcutsModal />);

    const input = screen.getByTestId('shortcuts-search-input');
    fireEvent.change(input, { target: { value: 'salvar como' } });

    const list = screen.getByTestId('shortcuts-list');
    expect(within(list).getByText('Salvar como')).toBeTruthy();
    expect(within(list).queryByText('Abrir projeto')).toBeNull();
  });

  it('search filters by combo text (ex.: "F5")', () => {
    useBiStore.getState().setShortcutsModalOpen(true);
    render(<KeyboardShortcutsModal />);

    const input = screen.getByTestId('shortcuts-search-input');
    fireEvent.change(input, { target: { value: 'F5' } });

    const list = screen.getByTestId('shortcuts-list');
    expect(within(list).getByText('Atualizar dados')).toBeTruthy();
  });

  it('shows the empty state when nothing matches', () => {
    useBiStore.getState().setShortcutsModalOpen(true);
    render(<KeyboardShortcutsModal />);

    const input = screen.getByTestId('shortcuts-search-input');
    fireEvent.change(input, { target: { value: 'xyz-nao-existe' } });

    expect(screen.getByText('Nenhum atalho encontrado.')).toBeTruthy();
  });

  it('closes via the close button', () => {
    useBiStore.getState().setShortcutsModalOpen(true);
    render(<KeyboardShortcutsModal />);
    fireEvent.click(screen.getByLabelText('Fechar'));
    expect(useBiStore.getState().shortcutsModalOpen).toBe(false);
  });
});
