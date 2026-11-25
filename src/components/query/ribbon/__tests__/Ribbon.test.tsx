// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { Ribbon } from '../Ribbon';
import { homeTab, transformTab, addColumnTab, viewTab, RibbonCtx } from '../menus';

function makeCtx(selectedColumns: string[] = []): RibbonCtx {
  return {
    selectedColumns,
    columnTypes: { Nome: 'text' },
    allColumns: ['Nome'],
    addStep: vi.fn(),
    openDialog: vi.fn(),
    openParamDialog: vi.fn(),
  };
}

describe('<Ribbon /> tabbed layout', () => {
  const tabs = [
    homeTab({ onCloseAndApply: vi.fn(), onApply: vi.fn(), onClose: vi.fn(), onRefreshPreview: vi.fn() }),
    transformTab(),
    addColumnTab(),
    viewTab({ formulaBar: true, quality: true, distribution: true, profile: true }, vi.fn()),
  ];

  it('renders the four documented tabs', () => {
    render(<Ribbon tabs={tabs} activeTabId="home" onTabChange={vi.fn()} ctx={makeCtx()} />);
    expect(screen.getByTestId('ribbon-tab-home')).toBeTruthy();
    expect(screen.getByTestId('ribbon-tab-transform')).toBeTruthy();
    expect(screen.getByTestId('ribbon-tab-addColumn')).toBeTruthy();
    expect(screen.getByTestId('ribbon-tab-view')).toBeTruthy();
  });

  it('switches which groups are shown when a tab is clicked', () => {
    const onTabChange = vi.fn();
    render(<Ribbon tabs={tabs} activeTabId="home" onTabChange={onTabChange} ctx={makeCtx()} />);
    expect(screen.getByText('Reduzir Linhas')).toBeTruthy();
    fireEvent.click(screen.getByTestId('ribbon-tab-transform'));
    expect(onTabChange).toHaveBeenCalledWith('transform');
  });

  it('shows the group content for whichever tab is marked active', () => {
    render(<Ribbon tabs={tabs} activeTabId="addColumn" onTabChange={vi.fn()} ctx={makeCtx()} />);
    expect(screen.getByText('Geral')).toBeTruthy();
    expect(screen.queryByText('Reduzir Linhas')).toBeNull();
  });

  it('disables column-dependent buttons and exposes the reason as a tooltip', () => {
    render(<Ribbon tabs={tabs} activeTabId="home" onTabChange={vi.fn()} ctx={makeCtx([])} />);
    const removeColumnsMenu = screen.getByTestId('ribbon-item-removeColumnsMenu');
    fireEvent.mouseEnter(removeColumnsMenu);
    const removeBtn = screen.getByTestId('ribbon-item-removeColumns') as HTMLButtonElement;
    expect(removeBtn.disabled).toBe(true);
    expect(removeBtn.getAttribute('title')).toBe('Selecione uma coluna');
  });

  it('enables the same button once a column is selected', () => {
    render(<Ribbon tabs={tabs} activeTabId="home" onTabChange={vi.fn()} ctx={makeCtx(['Nome'])} />);
    fireEvent.mouseEnter(screen.getByTestId('ribbon-item-removeColumnsMenu'));
    const removeBtn = screen.getByTestId('ribbon-item-removeColumns') as HTMLButtonElement;
    expect(removeBtn.disabled).toBe(false);
  });

  it('looks and behaves like real buttons: cursor-pointer, disabled chrome, focus ring', () => {
    render(<Ribbon tabs={tabs} activeTabId="home" onTabChange={vi.fn()} ctx={makeCtx([])} />);
    const closeApply = screen.getByTestId('ribbon-item-closeApply');
    expect(closeApply.className).toContain('cursor-pointer');
    expect(closeApply.className).toContain('focus-visible:ring');

    const removeColumnsMenu = screen.getByTestId('ribbon-item-removeColumnsMenu');
    fireEvent.mouseEnter(removeColumnsMenu);
    const disabledBtn = screen.getByTestId('ribbon-item-removeColumns') as HTMLButtonElement;
    expect(disabledBtn.className).toContain('disabled:opacity-40');
    expect(disabledBtn.className).toContain('disabled:cursor-not-allowed');
  });

  it('shows a ▸ chevron on dropdown (submenu) top-level buttons', () => {
    render(<Ribbon tabs={tabs} activeTabId="home" onTabChange={vi.fn()} ctx={makeCtx(['Nome'])} />);
    // "Remover Colunas" is a submenu (dropdown) at the top level of the Geral group.
    expect(screen.getByTestId('ribbon-chevron-removeColumnsMenu')).toBeTruthy();
  });

  it('renders icons for the primary Home/Geral actions', () => {
    render(<Ribbon tabs={tabs} activeTabId="home" onTabChange={vi.fn()} ctx={makeCtx([])} />);
    const closeApply = screen.getByTestId('ribbon-item-closeApply');
    expect(closeApply.querySelector('svg')).toBeTruthy();
  });
});
