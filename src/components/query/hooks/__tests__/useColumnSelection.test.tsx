// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import React from 'react';
import { useColumnSelection, computeNextSelection } from '../useColumnSelection';

describe('computeNextSelection (pure logic)', () => {
  const order = ['A', 'B', 'C', 'D'];

  it('plain click selects only that column', () => {
    expect(computeNextSelection(order, ['A'], 'A', 'C', {})).toEqual({ selected: ['C'], anchor: 'C' });
  });

  it('ctrl+click toggles membership, keeping the rest', () => {
    expect(computeNextSelection(order, ['A'], 'A', 'B', { ctrlKey: true })).toEqual({ selected: ['A', 'B'], anchor: 'B' });
    expect(computeNextSelection(order, ['A', 'B'], 'B', 'A', { ctrlKey: true })).toEqual({ selected: ['B'], anchor: 'A' });
  });

  it('shift+click selects the range from the anchor', () => {
    expect(computeNextSelection(order, ['A'], 'A', 'C', { shiftKey: true })).toEqual({ selected: ['A', 'B', 'C'], anchor: 'A' });
    // da direita para a esquerda, a ordem segue o gesto (âncora primeiro)
    expect(computeNextSelection(order, ['C'], 'C', 'A', { shiftKey: true })).toEqual({ selected: ['C', 'B', 'A'], anchor: 'C' });
  });

  it('keeps click order for merges: Nome do Mês, then Shift/Ctrl+click Ano gives [Nome do Mês, Ano]', () => {
    const cols = ['Data', 'Valor', 'Ano', 'Nome do Mês'];
    expect(computeNextSelection(cols, ['Nome do Mês'], 'Nome do Mês', 'Ano', { shiftKey: true }).selected).toEqual(['Nome do Mês', 'Ano']);
    expect(computeNextSelection(cols, ['Nome do Mês'], 'Nome do Mês', 'Ano', { ctrlKey: true }).selected).toEqual(['Nome do Mês', 'Ano']);
  });
});

const Harness: React.FC<{ columns: string[] }> = ({ columns }) => {
  const { selected, isSelected, handleColumnClick } = useColumnSelection(columns);
  return (
    <div>
      <span data-testid="selected">{selected.join(',')}</span>
      {columns.map(c => (
        <button
          key={c}
          data-testid={`col-${c}`}
          data-selected={isSelected(c)}
          onClick={e => handleColumnClick(c, { ctrlKey: e.ctrlKey, shiftKey: e.shiftKey })}
        >
          {c}
        </button>
      ))}
    </div>
  );
};

describe('useColumnSelection (React hook)', () => {
  it('click / ctrl+click / shift+click drive the selection as in the preview grid', () => {
    render(<Harness columns={['A', 'B', 'C', 'D']} />);
    fireEvent.click(screen.getByTestId('col-A'));
    expect(screen.getByTestId('selected').textContent).toBe('A');

    fireEvent.click(screen.getByTestId('col-C'), { shiftKey: true });
    expect(screen.getByTestId('selected').textContent).toBe('A,B,C');

    fireEvent.click(screen.getByTestId('col-D'), { ctrlKey: true });
    expect(screen.getByTestId('selected').textContent).toBe('A,B,C,D');
  });
});
