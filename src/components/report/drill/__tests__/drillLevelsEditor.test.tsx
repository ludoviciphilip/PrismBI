// @vitest-environment jsdom
import React, { useState } from 'react';
import { afterAll, afterEach, beforeAll, describe, expect, it } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
import { DrillLevelsEditor } from '../DrillLevelsEditor';
import { Table, Visual, VisualDataConfig } from '../../../../types/bi';

// Mesmo padrão do teste de larguras do segmentador (skill tailwind-width-class-override):
// jsdom não mede; mapeamos as classes de largura para px e o padrão "colapsado" é 18px.
const TAILWIND_WIDTH_PX: Record<string, number> = { 'w-6': 24, 'w-full': 260 };
beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: function (this: HTMLElement) {
      const classes = (this.className || '').toString().split(/\s+/).filter(Boolean);
      let width = 18;
      for (const cls of classes) if (TAILWIND_WIDTH_PX[cls]) width = TAILWIND_WIDTH_PX[cls];
      if (classes.includes('flex-1') || classes.includes('min-w-0')) width = Math.max(width, 140);
      return { width, height: 26, top: 0, left: 0, right: width, bottom: 26, x: 0, y: 0, toJSON() {} };
    },
  });
});
afterAll(() => {
  // @ts-expect-error restaura o padrão do jsdom
  delete HTMLElement.prototype.getBoundingClientRect;
});
afterEach(() => cleanup());

const table: Table = {
  id: 'fv',
  name: 'fVendas',
  originalName: 'fVendas',
  columns: [
    { id: '1', name: 'Regiao', originalName: 'Regiao', dataType: 'text' },
    { id: '2', name: 'Cidade', originalName: 'Cidade', dataType: 'text' },
    { id: '3', name: 'Loja', originalName: 'Loja', dataType: 'text' },
    { id: '4', name: 'Data', originalName: 'Data', dataType: 'date' },
  ],
  rows: [],
} as Table;
const dProd: Table = {
  id: 'dp',
  name: 'dProduto',
  originalName: 'dProduto',
  columns: [{ id: 'x', name: 'Categoria', originalName: 'Categoria', dataType: 'text' }],
  rows: [],
} as Table;

const visual = (type: Visual['type'], dc: Partial<VisualDataConfig>): Visual => ({
  id: 'v',
  type,
  title: 'V',
  x: 0,
  y: 0,
  w: 300,
  h: 200,
  dataConfig: { tableId: 'fv', ...dc },
  formatConfig: { showTitle: true },
});

let last: VisualDataConfig | null = null;
const Harness: React.FC<{ initial: Visual }> = ({ initial }) => {
  const [v, setV] = useState(initial);
  return (
    <div className="w-full">
      <DrillLevelsEditor
        visual={v}
        table={table}
        relatedTables={[dProd]}
        onChange={patch => {
          setV(prev => {
            const next = { ...prev, dataConfig: { ...prev.dataConfig, ...patch } };
            last = next.dataConfig;
            return next;
          });
        }}
      />
    </div>
  );
};

describe('DrillLevelsEditor (painel Visualizações)', () => {
  it('matriz: adiciona, troca e remove níveis em matrixRowFields (N níveis)', () => {
    render(<Harness initial={visual('matrix', { xAxisField: 'Regiao' })} />);
    expect(screen.getByText('Linhas: níveis da hierarquia')).toBeTruthy();
    const add = screen.getByLabelText('Adicionar nível') as HTMLSelectElement;
    fireEvent.change(add, { target: { value: 'Cidade' } });
    expect(last?.matrixRowFields).toEqual(['Regiao', 'Cidade']);
    fireEvent.change(screen.getByLabelText('Adicionar nível'), { target: { value: 'dProduto.Categoria' } });
    expect(last?.matrixRowFields).toEqual(['Regiao', 'Cidade', 'dProduto.Categoria']);
    // trocar o nível 2
    fireEvent.change(screen.getByLabelText('Nível 2'), { target: { value: 'Loja' } });
    expect(last?.matrixRowFields).toEqual(['Regiao', 'Loja', 'dProduto.Categoria']);
    // remover o nível 2
    fireEvent.click(screen.getByRole('button', { name: 'Remover nível 2' }));
    expect(last?.matrixRowFields).toEqual(['Regiao', 'dProduto.Categoria']);
    fireEvent.click(screen.getByRole('button', { name: 'Remover nível 2' }));
    expect(last?.matrixRowFields).toBeUndefined();
    expect(last?.drillFields).toBeUndefined();
  });

  it('matriz antiga (drillFields[0]) mostra a sub-linha existente como nível 2', () => {
    render(<Harness initial={visual('matrix', { xAxisField: 'Regiao', drillFields: ['Cidade'] })} />);
    expect((screen.getByLabelText('Nível 2') as HTMLSelectElement).value).toBe('Cidade');
  });

  it('gráfico: níveis abaixo do eixo vão para drillFields; hierarquia de data automática liga/desliga', () => {
    render(<Harness initial={visual('column', { xAxisField: 'Data', drillMode: 'drill' })} />);
    const auto = screen.getByRole('checkbox', { name: /Hierarquia de data automática/ }) as HTMLInputElement;
    expect(auto.checked).toBe(true);
    fireEvent.click(auto);
    expect(last?.drillFields).toEqual([]);
    fireEvent.click(screen.getByRole('checkbox', { name: /Hierarquia de data automática/ }));
    expect(last?.drillFields).toBeUndefined();
    expect(last?.drillMode).toBe('drill');
    fireEvent.change(screen.getByLabelText('Adicionar nível'), { target: { value: 'Loja' } });
    expect(last?.drillFields).toEqual(['Loja']);
    // com níveis explícitos a opção automática some (eles têm prioridade)
    expect(screen.queryByRole('checkbox', { name: /Hierarquia de data automática/ })).toBeNull();
  });

  it('sem campo no eixo: desabilitado com dica', () => {
    render(<Harness initial={visual('bar', {})} />);
    const add = screen.getByLabelText('Adicionar nível') as HTMLSelectElement;
    expect(add.disabled).toBe(true);
    expect(add.title).toMatch(/Escolha primeiro/);
  });

  it('campos têm largura real (não colapsam) em 1366×768', () => {
    render(<Harness initial={visual('matrix', { xAxisField: 'Regiao', matrixRowFields: ['Regiao', 'Cidade', 'Loja'] })} />);
    for (const el of [screen.getByLabelText('Nível 2'), screen.getByLabelText('Nível 3'), screen.getByLabelText('Adicionar nível')]) {
      expect(el.getBoundingClientRect().width).toBeGreaterThanOrEqual(120);
    }
    for (const b of screen.getAllByRole('button', { name: /Remover nível/ })) {
      expect(b.getBoundingClientRect().width).toBe(24);
    }
  });
});
