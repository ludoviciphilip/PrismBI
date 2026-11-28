// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import React from 'react';
import { CustomColumnDialog } from '../CustomColumnDialog';

describe('CustomColumnDialog', () => {
  it('Bug 2: opens EMPTY for a brand-new column (no stray "each" baked in)', () => {
    const onApply = vi.fn();
    render(<CustomColumnDialog columns={['Nome']} onClose={() => {}} onApply={onApply} />);
    const textarea = screen.getByTestId('custom-column-expression') as HTMLTextAreaElement;
    expect(textarea.value).toBe('');
    fireEvent.change(textarea, { target: { value: '[Nome] & "!"' } });
    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith({ newColumn: 'Coluna_Personalizada', expression: '[Nome] & "!"' });
  });

  it('Bug 2: editing a Custom Column step opens prefilled (name, formula without "each", type)', () => {
    const onApply = vi.fn();
    render(
      <CustomColumnDialog
        columns={['Ano', 'Nome do Mês']}
        initial={{ newColumn: 'Mesclado', expression: 'each Text.Combine({Text.From([Ano]), [Nome do Mês]}, "-")', type: 'text' }}
        onClose={() => {}}
        onApply={onApply}
      />,
    );
    expect(screen.getByDisplayValue('Mesclado')).toBeTruthy();
    const textarea = screen.getByTestId('custom-column-expression') as HTMLTextAreaElement;
    // A fórmula aparece SEM o 'each' do M.
    expect(textarea.value).toBe('Text.Combine({Text.From([Ano]), [Nome do Mês]}, "-")');

    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith({
      newColumn: 'Mesclado',
      expression: 'Text.Combine({Text.From([Ano]), [Nome do Mês]}, "-")',
      type: 'text',
    });
  });
});
