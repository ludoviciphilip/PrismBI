// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ConditionalColumnDialog } from '../ConditionalColumnDialog';

describe('ConditionalColumnDialog', () => {
  it('accepts typed input in every clause field and produces the exact contract JSON', () => {
    const onApply = vi.fn();
    render(
      <ConditionalColumnDialog
        columns={[{ name: 'Regiao', type: 'text' }, { name: 'Valor', type: 'decimal' }, { name: 'ValorAjustado', type: 'decimal' }]}
        onClose={() => {}}
        onApply={onApply}
      />
    );

    fireEvent.change(screen.getByDisplayValue('Coluna_Condicional'), { target: { value: 'Faixa' } });

    // Cláusula 0: Regiao é igual a "Sul" então "Sul-OK" (valor literal)
    fireEvent.change(screen.getByTestId('clause-0-column'), { target: { value: 'Regiao' } });
    fireEvent.change(screen.getByTestId('clause-0-op'), { target: { value: 'eq' } });
    fireEvent.change(screen.getByTestId('clause-0-value').querySelector('input')!, { target: { value: 'Sul' } });
    fireEvent.change(screen.getByTestId('clause-0-then').querySelector('input')!, { target: { value: 'Sul-OK' } });

    // Adiciona 2ª cláusula: Valor > 100 então coluna ValorAjustado
    fireEvent.click(screen.getByText('Adicionar cláusula'));
    fireEvent.change(screen.getByTestId('clause-1-column'), { target: { value: 'Valor' } });
    fireEvent.change(screen.getByTestId('clause-1-op'), { target: { value: 'gt' } });
    fireEvent.change(screen.getByTestId('clause-1-value').querySelector('input')!, { target: { value: '100' } });
    fireEvent.change(screen.getByTestId('clause-1-then').querySelector('select[aria-label="clause-1-then-kind"]')!, { target: { value: 'column' } });
    fireEvent.change(screen.getByTestId('clause-1-then').querySelector('select[aria-label="clause-1-then-column"]')!, { target: { value: 'ValorAjustado' } });

    // Caso contrário: 0
    fireEvent.change(screen.getByTestId('otherwise').querySelector('input')!, { target: { value: '0' } });

    fireEvent.click(screen.getByText('OK'));

    expect(onApply).toHaveBeenCalledWith({
      newColumn: 'Faixa',
      rules: [
        { when: { column: 'Regiao', op: 'eq', value: 'Sul' }, then: 'Sul-OK' },
        { when: { column: 'Valor', op: 'gt', value: 100 }, then: { column: 'ValorAjustado' } },
      ],
      otherwise: '0',
    });
  });

  it('reorders and removes clauses', () => {
    const onApply = vi.fn();
    render(<ConditionalColumnDialog columns={['A', 'B']} onClose={() => {}} onApply={onApply} />);
    fireEvent.click(screen.getByText('Adicionar cláusula'));
    expect(screen.getByTestId('clause-0-column')).toBeTruthy();
    expect(screen.getByTestId('clause-1-column')).toBeTruthy();
    fireEvent.click(screen.getAllByLabelText('Remover cláusula')[1]);
    expect(screen.queryByTestId('clause-1-column')).toBeNull();
  });
});
