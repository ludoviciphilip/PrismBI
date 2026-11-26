/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { useUiPrefs, COLLAPSED_PANE_WIDTH } from '../useUiPrefs';

const DEFAULT_PANES = {
  filters: 'open',
  visualizations: 'open',
  fields: 'open',
  bookmarks: 'hidden',
  selection: 'hidden',
} as const;

function resetStore() {
  localStorage.clear();
  useUiPrefs.setState({ panes: { ...DEFAULT_PANES } });
}

describe('useUiPrefs', () => {
  beforeEach(() => resetStore());

  it('defaults every pane to open (bookmarks/selection start hidden)', () => {
    expect(useUiPrefs.getState().panes).toEqual(DEFAULT_PANES);
  });

  it('setPaneState sets an exact visibility', () => {
    useUiPrefs.getState().setPaneState('filters', 'collapsed');
    expect(useUiPrefs.getState().panes.filters).toBe('collapsed');
    expect(useUiPrefs.getState().panes.visualizations).toBe('open');
  });

  it('togglePaneCollapsed flips between open and collapsed only', () => {
    const { togglePaneCollapsed } = useUiPrefs.getState();
    togglePaneCollapsed('fields');
    expect(useUiPrefs.getState().panes.fields).toBe('collapsed');
    togglePaneCollapsed('fields');
    expect(useUiPrefs.getState().panes.fields).toBe('open');
  });

  it('togglePaneCollapsed never collapses a hidden pane', () => {
    useUiPrefs.getState().setPaneHidden('visualizations', true);
    useUiPrefs.getState().togglePaneCollapsed('visualizations');
    expect(useUiPrefs.getState().panes.visualizations).toBe('hidden');
  });

  it('setPaneHidden toggles hidden <-> open (ribbon "Exibir" buttons)', () => {
    const { setPaneHidden } = useUiPrefs.getState();
    setPaneHidden('filters', true);
    expect(useUiPrefs.getState().panes.filters).toBe('hidden');
    setPaneHidden('filters', false);
    expect(useUiPrefs.getState().panes.filters).toBe('open');
  });

  it('setPaneHidden also opens the new "bookmarks" and "selection" panes', () => {
    const { setPaneHidden } = useUiPrefs.getState();
    setPaneHidden('bookmarks', false);
    expect(useUiPrefs.getState().panes.bookmarks).toBe('open');
    setPaneHidden('selection', false);
    expect(useUiPrefs.getState().panes.selection).toBe('open');
  });

  it('toggleAllPanesCollapsed (Ctrl+F1) collapses every visible open pane, then expands them back', () => {
    const { toggleAllPanesCollapsed } = useUiPrefs.getState();
    toggleAllPanesCollapsed();
    expect(useUiPrefs.getState().panes).toEqual({
      filters: 'collapsed',
      visualizations: 'collapsed',
      fields: 'collapsed',
      bookmarks: 'hidden',
      selection: 'hidden',
    });
    toggleAllPanesCollapsed();
    expect(useUiPrefs.getState().panes).toEqual(DEFAULT_PANES);
  });

  it('toggleAllPanesCollapsed skips hidden panes and collapses only the visible ones', () => {
    useUiPrefs.getState().setPaneHidden('fields', true);
    useUiPrefs.getState().toggleAllPanesCollapsed();
    expect(useUiPrefs.getState().panes).toEqual({
      filters: 'collapsed',
      visualizations: 'collapsed',
      fields: 'hidden',
      bookmarks: 'hidden',
      selection: 'hidden',
    });
  });

  it('persists to localStorage under the "prismbi.ui" key', () => {
    useUiPrefs.getState().setPaneState('filters', 'collapsed');
    const raw = localStorage.getItem('prismbi.ui');
    expect(raw).toBeTruthy();
    const parsed = JSON.parse(raw!);
    expect(parsed.state.panes.filters).toBe('collapsed');
  });

  it('rehydrates a previously persisted state on a fresh store creation', async () => {
    localStorage.setItem(
      'prismbi.ui',
      JSON.stringify({ state: { panes: { filters: 'collapsed', visualizations: 'hidden', fields: 'open' } }, version: 1 })
    );
    // Zustand persist rehydrates asynchronously; re-trigger it explicitly for a deterministic test.
    await useUiPrefs.persist.rehydrate();
    expect(useUiPrefs.getState().panes.filters).toBe('collapsed');
    expect(useUiPrefs.getState().panes.visualizations).toBe('hidden');
    expect(useUiPrefs.getState().panes.fields).toBe('open');
  });

  it('exposes the collapsed strip width used by the pane shell', () => {
    expect(COLLAPSED_PANE_WIDTH).toBe(32);
  });
});
