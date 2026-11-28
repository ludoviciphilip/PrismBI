// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import React from 'react';
import { MergeColumnsDialog } from '../MergeColumnsDialog';

describe('MergeColumnsDialog', () => {
  it('Bug 1: pre-checks the columns already selected in the grid, in SELECTION order', () => {
    // Usuário selecionou 'Nome do Mês' primeiro e depois Ctrl+clicou em 'Ano':
    // a ordem de seleção chega como ['Nome do Mês', 'Ano'], invertida da
    // ordem física da tabela (['Ano', 'Nome do Mês'] em `columns`).
    const onApply = vi.fn();
    render(
      <MergeColumnsDialog
        columns={['Data', 'Valor', 'Ano', 'Nome do Mês']}
        initialSelected={['Nome do Mês', 'Ano']}
        onClose={() => {}}
        onApply={onApply}
      />,
    );
    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith({ columns: ['Nome do Mês', 'Ano'], separator: ' - ', newColumn: 'Mesclado' });
  });

  it('Bug 2: editing an existing mergeColumns step opens prefilled (columns, separator, name)', () => {
    const onApply = vi.fn();
    render(
      <MergeColumnsDialog
        columns={['Data', 'Valor', 'Ano', 'Nome do Mês']}
        initialSelected={['Ano', 'Nome do Mês']}
        initialSeparator="/"
        initialNewColumn="Período"
        onClose={() => {}}
        onApply={onApply}
      />,
    );
    expect(screen.getByDisplayValue('/')).toBeTruthy();
    expect(screen.getByDisplayValue('Período')).toBeTruthy();
    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith({ columns: ['Ano', 'Nome do Mês'], separator: '/', newColumn: 'Período' });
  });

  it('with no pre-selection, opens empty and lets the user build the order by clicking', () => {
    const onApply = vi.fn();
    render(<MergeColumnsDialog columns={['Ano', 'Nome do Mês', 'Valor']} onClose={() => {}} onApply={onApply} />);
    fireEvent.click(screen.getByText('Nome do Mês'));
    fireEvent.click(screen.getByText('Ano'));
    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith({ columns: ['Nome do Mês', 'Ano'], separator: ' - ', newColumn: 'Mesclado' });
  });
});
