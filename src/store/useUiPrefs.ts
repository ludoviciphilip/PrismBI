/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';

/**
 * Visibility of a side pane, Power BI Desktop style:
 * - 'open': full pane, as before.
 * - 'collapsed': a ~32px vertical strip (title + icon), click to expand.
 * - 'hidden': not rendered at all (same as the old show/hide toggle).
 */
export type PaneVisibility = 'open' | 'collapsed' | 'hidden';

export type PaneKey = 'filters' | 'visualizations' | 'fields' | 'bookmarks' | 'selection';

export const COLLAPSED_PANE_WIDTH = 32;

interface PanesState {
  filters: PaneVisibility;
  visualizations: PaneVisibility;
  fields: PaneVisibility;
  /** v0.10: painel "Indicadores" (Bookmarks), aberto pela guia Exibir. */
  bookmarks: PaneVisibility;
  /** v0.10: painel "Seleção" (mostrar/ocultar visuais), aberto pela guia Exibir. */
  selection: PaneVisibility;
}

const DEFAULT_PANES: PanesState = {
  filters: 'open',
  visualizations: 'open',
  fields: 'open',
  bookmarks: 'hidden',
  selection: 'hidden',
};

interface UiPrefsState {
  panes: PanesState;
  /** Sets the exact visibility of one pane. */
  setPaneState: (pane: PaneKey, state: PaneVisibility) => void;
  /** Toggles between 'open' and 'collapsed' (never touches 'hidden'). */
  togglePaneCollapsed: (pane: PaneKey) => void;
  /** Shows ('open', restoring the previous non-hidden state) or hides a pane. */
  setPaneHidden: (pane: PaneKey, hidden: boolean) => void;
  /**
   * Ctrl+F1: collapses every visible pane that is currently open, or — if
   * none is open — expands every visible collapsed pane back to open.
   * Hidden panes are left untouched.
   */
  toggleAllPanesCollapsed: () => void;
}

const PANE_KEYS: PaneKey[] = ['filters', 'visualizations', 'fields', 'bookmarks', 'selection'];

export const useUiPrefs = create<UiPrefsState>()(
  persist(
    (set, get) => ({
      panes: { ...DEFAULT_PANES },

      setPaneState: (pane, state) =>
        set(s => ({ panes: { ...s.panes, [pane]: state } })),

      togglePaneCollapsed: pane =>
        set(s => {
          const current = s.panes[pane];
          if (current === 'hidden') return s;
          return {
            panes: { ...s.panes, [pane]: current === 'collapsed' ? 'open' : 'collapsed' },
          };
        }),

      setPaneHidden: (pane, hidden) =>
        set(s => ({
          panes: { ...s.panes, [pane]: hidden ? 'hidden' : 'open' },
        })),

      toggleAllPanesCollapsed: () => {
        const { panes } = get();
        const visibleKeys = PANE_KEYS.filter(k => panes[k] !== 'hidden');
        const anyOpen = visibleKeys.some(k => panes[k] === 'open');
        const nextVisible: PaneVisibility = anyOpen ? 'collapsed' : 'open';
        set(s => {
          const next = { ...s.panes };
          for (const k of visibleKeys) next[k] = nextVisible;
          return { panes: next };
        });
      },
    }),
    {
      name: 'vetrobi.ui',
      storage: createJSONStorage(() => ({
        getItem: (key: string) =>
          typeof localStorage === 'undefined'
            ? null
            : localStorage.getItem(key) ?? localStorage.getItem('prismbi.ui'),
        setItem: (key: string, value: string) => {
          if (typeof localStorage !== 'undefined') localStorage.setItem(key, value);
        },
        removeItem: (key: string) => {
          if (typeof localStorage !== 'undefined') {
            localStorage.removeItem(key);
            localStorage.removeItem('prismbi.ui');
          }
        },
      })),
      version: 2,
      // v0.10: panes persisted before "bookmarks"/"selection" existed won't
      // have those keys; backfill them (hidden by default) instead of
      // leaving `panes.bookmarks`/`panes.selection` undefined after rehydrate.
      migrate: (persistedState: any) => ({
        ...persistedState,
        panes: { ...DEFAULT_PANES, ...(persistedState?.panes ?? {}) },
      }),
      merge: (persistedState: any, currentState) => ({
        ...currentState,
        ...persistedState,
        panes: { ...DEFAULT_PANES, ...currentState.panes, ...(persistedState?.panes ?? {}) },
      }),
    }
  )
);
