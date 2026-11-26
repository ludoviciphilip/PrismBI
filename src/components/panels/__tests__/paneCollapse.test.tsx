/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
import { useBiStore } from '../../../store/useBiStore';
import { useUiPrefs, COLLAPSED_PANE_WIDTH } from '../../../store/useUiPrefs';
import { FiltersPane } from '../FiltersPane';
import { VisualizationsPane } from '../VisualizationsPane';
import { FieldsPane } from '../FieldsPane';

function resetUiPrefs() {
  localStorage.clear();
  useUiPrefs.setState({ panes: { filters: 'open', visualizations: 'open', fields: 'open' } });
}

beforeEach(() => {
  useBiStore.getState().newProject('Panes Test');
  resetUiPrefs();
});
afterEach(() => cleanup());

describe('FiltersPane collapse/expand', () => {
  it('shows the full pane when open, and the narrow strip when collapsed', () => {
    render(<FiltersPane />);
    expect(screen.getByText('Filtros')).toBeTruthy();
    expect(screen.queryByTestId('filters-pane-collapsed')).toBeNull();

    fireEvent.click(screen.getByTestId('filters-pane-collapse'));
    expect(useUiPrefs.getState().panes.filters).toBe('collapsed');
  });

  it('renders only the collapsed strip (fixed narrow width) when pane state is collapsed', () => {
    useUiPrefs.getState().setPaneState('filters', 'collapsed');
    render(<FiltersPane />);
    const strip = screen.getByTestId('filters-pane-collapsed');
    expect(strip).toBeTruthy();
    expect(strip.style.width).toBe(`${COLLAPSED_PANE_WIDTH}px`);
    // The full pane's content (e.g. the "+" add-filter button) must not be rendered.
    expect(screen.queryByText('Novo Filtro')).toBeNull();
  });

  it('clicking the collapsed strip expands the pane back to open', () => {
    useUiPrefs.getState().setPaneState('filters', 'collapsed');
    render(<FiltersPane />);
    fireEvent.click(screen.getByTestId('filters-pane-collapsed'));
    expect(useUiPrefs.getState().panes.filters).toBe('open');
  });
});

describe('FieldsPane collapse/expand', () => {
  it('collapses via its header button and expands via the strip', () => {
    render(<FieldsPane />);
    fireEvent.click(screen.getByTestId('fields-pane-collapse'));
    expect(useUiPrefs.getState().panes.fields).toBe('collapsed');
  });
});

describe('VisualizationsPane collapse/expand', () => {
  it('collapses from the "no visual selected" header', () => {
    render(<VisualizationsPane />);
    fireEvent.click(screen.getByTestId('visualizations-pane-collapse'));
    expect(useUiPrefs.getState().panes.visualizations).toBe('collapsed');
  });

  it('collapses from the main header when a visual is selected', () => {
    const table = { id: 't1', name: 'Vendas', originalName: 'Vendas', columns: [], rows: [] } as any;
    useBiStore.getState().setTables([table]);
    const pageId = useBiStore.getState().activePageId;
    const visual = {
      id: 'v1',
      type: 'column',
      title: 'Visual',
      x: 0,
      y: 0,
      w: 400,
      h: 300,
      dataConfig: { tableId: 't1' },
      formatConfig: { showTitle: true },
    } as any;
    useBiStore.setState(s => ({
      pages: s.pages.map(p => (p.id === pageId ? { ...p, visuals: [visual] } : p)),
      selectedVisualId: 'v1',
    }));
    render(<VisualizationsPane />);
    fireEvent.click(screen.getByTestId('visualizations-pane-collapse'));
    expect(useUiPrefs.getState().panes.visualizations).toBe('collapsed');
  });
});

describe('Canvas gains the width freed by collapsed panes', () => {
  // jsdom has no real layout engine, so pixel widths from getBoundingClientRect
  // are always 0. What IS verifiable: the pane row is a flex container whose
  // panes are shrink-0 with a fixed width, and the canvas sibling is flex-1
  // (see src/App.tsx + ReportView.tsx). Collapsing a pane swaps its DOM node
  // from the ~256/272px panel to a strip whose own inline width is exactly
  // COLLAPSED_PANE_WIDTH (32px) — a flex-1 sibling therefore always receives
  // (old width - 32px) more space, regardless of layout engine.
  const OPEN_WIDTH_CLASS_PX: Record<'filters' | 'visualizations' | 'fields', number> = {
    filters: 256, // w-64
    visualizations: 272, // w-68
    fields: 256, // w-64
  };

  it('each collapsed pane is strictly narrower than its open width, freeing space for the canvas', () => {
    (['filters', 'visualizations', 'fields'] as const).forEach(key => {
      expect(COLLAPSED_PANE_WIDTH).toBeLessThan(OPEN_WIDTH_CLASS_PX[key]);
    });
  });

  it('FiltersPane collapsed strip reports the exact freed width in its own layout box', () => {
    useUiPrefs.getState().setPaneState('filters', 'collapsed');
    render(<FiltersPane />);
    const strip = screen.getByTestId('filters-pane-collapsed');
    const freedWidth = OPEN_WIDTH_CLASS_PX.filters - COLLAPSED_PANE_WIDTH;
    expect(parseInt(strip.style.width, 10)).toBe(COLLAPSED_PANE_WIDTH);
    expect(freedWidth).toBe(224);
  });
});
