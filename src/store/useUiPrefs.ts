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

export type PaneKey = 'filters' | 'visualizations' | 'fields';

export const COLLAPSED_PANE_WIDTH = 32;

interface PanesState {
  filters: PaneVisibility;
  visualizations: PaneVisibility;
  fields: PaneVisibility;
}

const DEFAULT_PANES: PanesState = {
  filters: 'open',
  visualizations: 'open',
  fields: 'open',
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

const PANE_KEYS: PaneKey[] = ['filters', 'visualizations', 'fields'];

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
      name: 'prismbi.ui',
      storage: createJSONStorage(() => localStorage),
      version: 1,
    }
  )
);
