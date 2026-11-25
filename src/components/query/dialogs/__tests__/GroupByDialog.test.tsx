// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { GroupByDialog } from '../GroupByDialog';

describe('GroupByDialog', () => {
  it('produces the exact { keys, aggregations } contract payload', () => {
    const onApply = vi.fn();
    render(<GroupByDialog columns={['Regiao', 'Valor', 'Qtd']} onClose={() => {}} onApply={onApply} />);

    // 'Regiao' já vem marcada como padrão (primeira coluna)

    const aggRow = screen.getByTestId('agg-row-0');
    fireEvent.change(aggRow.querySelector('input')!, { target: { value: 'Total Vendido' } });
    fireEvent.change(aggRow.querySelectorAll('select')[0], { target: { value: 'sum' } });
    fireEvent.change(aggRow.querySelectorAll('select')[1], { target: { value: 'Valor' } });

    fireEvent.click(screen.getByText('Nova agregação'));
    const aggRow2 = screen.getByTestId('agg-row-1');
    fireEvent.change(aggRow2.querySelector('input')!, { target: { value: 'Linhas' } });
    fireEvent.change(aggRow2.querySelectorAll('select')[0], { target: { value: 'countRows' } });

    fireEvent.click(screen.getByText('OK'));

    expect(onApply).toHaveBeenCalledWith({
      keys: ['Regiao'],
      aggregations: [
        { name: 'Total Vendido', op: 'sum', column: 'Valor' },
        { name: 'Linhas', op: 'countRows' },
      ],
    });
  });
});
