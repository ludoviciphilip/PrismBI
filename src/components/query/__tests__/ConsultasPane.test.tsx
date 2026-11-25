// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { ConsultasPane } from '../ConsultasPane';

const queries = [
  { id: 't1', name: 'f_Vendas', stepCount: 3 },
  { id: 't2', name: 'd_Produto', stepCount: 0 },
  { id: 't3', name: 'd_Cliente', stepCount: 1 },
];

describe('<ConsultasPane />', () => {
  it('lists every query with its step-count badge and highlights the selected one', () => {
    render(<ConsultasPane queries={queries} selectedId="t1" onSelect={vi.fn()} />);
    expect(screen.getByTestId('consulta-item-t1').textContent).toContain('f_Vendas');
    expect(screen.getByTestId('consulta-item-t1').textContent).toContain('3');
    expect(screen.getByTestId('consulta-item-t1').className).toMatch(/amber/);
    expect(screen.getByTestId('consulta-item-t2').className).not.toMatch(/amber/);
  });

  it('filters the list via the search box', () => {
    render(<ConsultasPane queries={queries} selectedId="t1" onSelect={vi.fn()} />);
    fireEvent.change(screen.getByTestId('consultas-search'), { target: { value: 'produto' } });
    expect(screen.queryByTestId('consulta-item-t1')).toBeNull();
    expect(screen.getByTestId('consulta-item-t2')).toBeTruthy();
  });

  it('calls onSelect with the clicked query id', () => {
    const onSelect = vi.fn();
    render(<ConsultasPane queries={queries} selectedId="t1" onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('consulta-item-t3'));
    expect(onSelect).toHaveBeenCalledWith('t3');
  });

  it('collapses and expands via the « / » buttons', () => {
    render(<ConsultasPane queries={queries} selectedId="t1" onSelect={vi.fn()} />);
    fireEvent.click(screen.getByTestId('consultas-collapse'));
    expect(screen.queryByTestId('consultas-pane')).toBeNull();
    fireEvent.click(screen.getByTestId('consultas-expand'));
    expect(screen.getByTestId('consultas-pane')).toBeTruthy();
  });
});
