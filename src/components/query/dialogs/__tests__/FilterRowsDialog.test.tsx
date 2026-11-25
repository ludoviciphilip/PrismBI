// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FilterRowsDialog } from '../FilterRowsDialog';

describe('FilterRowsDialog', () => {
  it('produces an exact Cond JSON for a single basic rule', () => {
    const onApply = vi.fn();
    render(<FilterRowsDialog columns={['Regiao', 'Valor']} onClose={() => {}} onApply={onApply} />);

    fireEvent.change(screen.getByTestId('filter-rule-0').querySelectorAll('select')[0], { target: { value: 'Regiao' } });
    fireEvent.change(screen.getByTestId('filter-rule-0').querySelectorAll('select')[1], { target: { value: 'eq' } });
    fireEvent.change(screen.getByPlaceholderText('Valor'), { target: { value: 'SP' } });

    fireEvent.click(screen.getByText('Aplicar'));

    expect(onApply).toHaveBeenCalledWith({ column: 'Regiao', op: 'eq', value: 'SP' });
  });

  it('wraps multiple rules with "and"/"or" combinators', () => {
    const onApply = vi.fn();
    render(<FilterRowsDialog columns={['Regiao', 'Valor']} onClose={() => {}} onApply={onApply} />);

    fireEvent.click(screen.getByText('+ Adicionar cláusula'));

    const rule0 = screen.getByTestId('filter-rule-0');
    fireEvent.change(rule0.querySelectorAll('select')[0], { target: { value: 'Regiao' } });
    fireEvent.change(rule0.querySelectorAll('select')[1], { target: { value: 'eq' } });
    fireEvent.change(rule0.querySelector('input')!, { target: { value: 'SP' } });

    const rule1 = screen.getByTestId('filter-rule-1');
    const combineSelect = rule1.querySelectorAll('select')[0];
    fireEvent.change(combineSelect, { target: { value: 'or' } });
    fireEvent.change(rule1.querySelectorAll('select')[1], { target: { value: 'Valor' } });
    fireEvent.change(rule1.querySelectorAll('select')[2], { target: { value: 'gt' } });
    fireEvent.change(rule1.querySelector('input')!, { target: { value: '100' } });

    fireEvent.click(screen.getByText('Aplicar'));

    expect(onApply).toHaveBeenCalledWith({
      or: [
        { column: 'Regiao', op: 'eq', value: 'SP' },
        { column: 'Valor', op: 'gt', value: 100 },
      ],
    });
  });

  it('applies the raw JSON when the advanced mode is used', () => {
    const onApply = vi.fn();
    render(<FilterRowsDialog columns={['Regiao']} onClose={() => {}} onApply={onApply} />);
    fireEvent.click(screen.getByText('Usar JSON avançado'));
    fireEvent.change(screen.getByTestId('filter-advanced-json'), {
      target: { value: JSON.stringify({ column: 'Regiao', op: 'isNotNull' }) },
    });
    fireEvent.click(screen.getByText('Aplicar'));
    expect(onApply).toHaveBeenCalledWith({ column: 'Regiao', op: 'isNotNull' });
  });
});
