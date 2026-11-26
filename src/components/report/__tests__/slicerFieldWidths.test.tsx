// @vitest-environment jsdom
import React from 'react';
import { afterAll, afterEach, beforeAll, describe, expect, it } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';

/**
 * jsdom has no real layout engine, so getBoundingClientRect() always
 * returns 0. Mirroring the pattern used by the condfmt dialog's width
 * regression test (see skill tailwind-width-class-override), we map the
 * Tailwind width-affecting classes actually used by the new slicer inputs
 * to their real px size, with an 18px "collapsed" default. Any input that
 * loses its width class (or inherits a conflicting `w-full`/`flex-1`
 * baked into a shared class string) falls back to 18px and fails here —
 * at 1366×768 every new field here must stay above 60px.
 */
const TAILWIND_WIDTH_PX: Record<string, number> = {
  'w-10': 40,
  'w-16': 64,
  'w-20': 80,
  'w-full': 1200,
};

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: function (this: HTMLElement) {
      const classes = (this.className || '').split(/\s+/).filter(Boolean);
      let width = 18;
      for (const cls of classes) {
        if (TAILWIND_WIDTH_PX[cls]) {
          width = TAILWIND_WIDTH_PX[cls];
          break;
        }
      }
      if (classes.includes('flex-1') || classes.includes('min-w-0')) width = Math.max(width, 140);
      return { width, height: 28, top: 0, left: 0, right: width, bottom: 28, x: 0, y: 0, toJSON() {} };
    },
  });
});

afterAll(() => {
  // @ts-expect-error restores jsdom's default
  delete HTMLElement.prototype.getBoundingClientRect;
});

import { useBiStore } from '../../../store/useBiStore';
import { VisualCard } from '../VisualCard';
import { Table, Visual } from '../../../types/bi';

const table: Table = {
  id: 'tv',
  name: 'Vendas',
  originalName: 'Vendas',
  columns: [
    { id: 'c1', name: 'Valor', originalName: 'Valor', dataType: 'number' },
    { id: 'c2', name: 'Data', originalName: 'Data', dataType: 'date' },
  ],
  rows: [{ Valor: 10, Data: '2024-01-01' }],
} as Table;

function seed(visual: Visual) {
  const st = useBiStore.getState();
  st.newProject('Width Test');
  st.setTables([table]);
  const pageId = useBiStore.getState().activePageId;
  useBiStore.setState(s => ({ pages: [{ ...s.pages[0], id: pageId, visuals: [visual] }], filters: [] }));
}

const baseVisual = (over: Partial<Visual>): Visual => ({
  id: 'v1',
  type: 'slicer',
  title: 'Segmentador',
  x: 0,
  y: 0,
  w: 260,
  h: 170,
  dataConfig: { tableId: 'tv' },
  formatConfig: { showTitle: true },
  ...over,
});

afterEach(() => cleanup());

describe('Novos campos do segmentador cabem em 1366×768 (largura > 60px)', () => {
  it('between (número): min e max', () => {
    seed(baseVisual({ dataConfig: { tableId: 'tv', xAxisField: 'Valor', slicerConfig: { mode: 'between' } } }));
    render(<VisualCard visual={useBiStore.getState().pages[0].visuals[0]} isSelected={false} onSelect={() => {}} />);
    const min = screen.getByLabelText('Mín.') as HTMLInputElement;
    const max = screen.getByLabelText('Máx.') as HTMLInputElement;
    fireEvent.change(min, { target: { value: '5' } });
    fireEvent.change(max, { target: { value: '50' } });
    expect(min.getBoundingClientRect().width).toBeGreaterThan(60);
    expect(max.getBoundingClientRect().width).toBeGreaterThan(60);
  });

  it('between (data): início e fim', () => {
    seed(baseVisual({ dataConfig: { tableId: 'tv', xAxisField: 'Data', slicerConfig: { mode: 'between' } } }));
    render(<VisualCard visual={useBiStore.getState().pages[0].visuals[0]} isSelected={false} onSelect={() => {}} />);
    const start = screen.getByLabelText('Início') as HTMLInputElement;
    const end = screen.getByLabelText('Fim') as HTMLInputElement;
    fireEvent.change(start, { target: { value: '2026-09-01' } });
    fireEvent.change(end, { target: { value: '2026-11-30' } });
    expect(start.getBoundingClientRect().width).toBeGreaterThan(60);
    expect(end.getBoundingClientRect().width).toBeGreaterThan(60);
  });

  it('data relativa: contador numérico e seletores de âncora/unidade', () => {
    seed(
      baseVisual({
        dataConfig: {
          tableId: 'tv',
          xAxisField: 'Data',
          slicerConfig: { mode: 'relative-date', relative: { anchor: 'last', count: 3, unit: 'months' } },
        },
      })
    );
    render(<VisualCard visual={useBiStore.getState().pages[0].visuals[0]} isSelected={false} onSelect={() => {}} />);
    const count = screen.getByLabelText('count') as HTMLInputElement;
    fireEvent.change(count, { target: { value: '5' } });
    expect(count.getBoundingClientRect().width).toBeGreaterThan(60);
    expect(screen.getByLabelText('anchor').getBoundingClientRect().width).toBeGreaterThan(60);
    expect(screen.getByLabelText('unit').getBoundingClientRect().width).toBeGreaterThan(60);
  });

  it('busca do segmentador (showSearch)', () => {
    seed(baseVisual({ dataConfig: { tableId: 'tv', xAxisField: 'Valor', slicerConfig: { mode: 'list', showSearch: true } } }));
    render(<VisualCard visual={useBiStore.getState().pages[0].visuals[0]} isSelected={false} onSelect={() => {}} />);
    const search = screen.getByPlaceholderText('Buscar item') as HTMLInputElement;
    fireEvent.change(search, { target: { value: '10' } });
    expect(search.getBoundingClientRect().width).toBeGreaterThan(60);
  });
});
