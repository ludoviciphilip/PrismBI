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

  it('Bug 2: editing an existing filterRows step opens prefilled with the saved rule', () => {
    const onApply = vi.fn();
    render(
      <FilterRowsDialog
        columns={['Regiao', 'Valor']}
        initialCondition={{ column: 'Regiao', op: 'eq', value: 'SP' }}
        onClose={() => {}}
        onApply={onApply}
      />,
    );
    expect((screen.getByTestId('filter-rule-0').querySelectorAll('select')[0] as HTMLSelectElement).value).toBe('Regiao');
    expect((screen.getByTestId('filter-rule-0').querySelectorAll('select')[1] as HTMLSelectElement).value).toBe('eq');
    expect((screen.getByPlaceholderText('Valor') as HTMLInputElement).value).toBe('SP');
    fireEvent.click(screen.getByText('Aplicar'));
    expect(onApply).toHaveBeenCalledWith({ column: 'Regiao', op: 'eq', value: 'SP' });
  });

  it('Bug 2: editing a multi-clause "or" filterRows step prefills every clause', () => {
    const onApply = vi.fn();
    render(
      <FilterRowsDialog
        columns={['Regiao', 'Valor']}
        initialCondition={{ or: [{ column: 'Regiao', op: 'eq', value: 'SP' }, { column: 'Valor', op: 'gt', value: 100 }] }}
        onClose={() => {}}
        onApply={onApply}
      />,
    );
    expect(screen.getByTestId('filter-rule-1')).toBeTruthy();
    fireEvent.click(screen.getByText('Aplicar'));
    expect(onApply).toHaveBeenCalledWith({
      or: [
        { column: 'Regiao', op: 'eq', value: 'SP' },
        { column: 'Valor', op: 'gt', value: 100 },
      ],
    });
  });

  it('falls back to advanced JSON (never a blank clause) when editing a "not"/nested condition', () => {
    const onApply = vi.fn();
    render(
      <FilterRowsDialog
        columns={['Regiao']}
        initialCondition={{ not: { column: 'Regiao', op: 'isNull' } }}
        onClose={() => {}}
        onApply={onApply}
      />,
    );
    expect(screen.getByTestId('filter-advanced-json')).toBeTruthy();
    fireEvent.click(screen.getByText('Aplicar'));
    expect(onApply).toHaveBeenCalledWith({ not: { column: 'Regiao', op: 'isNull' } });
  });
});
