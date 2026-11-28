/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 *
 * Painel "Seleção" (básico): lista os visuais da página ativa com um olho
 * para ocultar/mostrar (`formatConfig.hidden`); o canvas (ReportView)
 * respeita esse campo. Útil junto com Indicadores para alternar entre dois
 * visuais na mesma posição, como no Power BI Desktop.
 */

import React from 'react';
import { MousePointer2, Eye, EyeOff } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { useUiPrefs } from '../../store/useUiPrefs';
import { useTranslation } from '../../i18n';
import { CollapsedPaneStrip, PaneCollapseButton } from './PaneCollapseShell';

export const SelectionPane: React.FC = React.memo(() => {
  const { t } = useTranslation();
  const s0 = t.selection;
  const paneState = useUiPrefs(s => s.panes.selection);
  const togglePaneCollapsed = useUiPrefs(s => s.togglePaneCollapsed);
  const { pages, activePageId, updateVisual, selectedVisualId, setSelectedVisualId } = useBiStore(
    useShallow(s => ({
      pages: s.pages,
      activePageId: s.activePageId,
      updateVisual: s.updateVisual,
      selectedVisualId: s.selectedVisualId,
      setSelectedVisualId: s.setSelectedVisualId,
    }))
  );

  if (paneState === 'collapsed') {
    return (
      <CollapsedPaneStrip
        icon={MousePointer2}
        iconClassName="w-4 h-4 text-amber-500"
        title={s0.title}
        expandLabel={s0.expandPanel}
        onExpand={() => togglePaneCollapsed('selection')}
        testId="selection-pane-collapsed"
      />
    );
  }

  const activePage = pages.find(p => p.id === activePageId);
  const visuals = activePage?.visuals ?? [];

  return (
    <div className="w-56 border-l border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 flex flex-col h-full text-xs select-none text-slate-800 dark:text-slate-200">
      <div className="p-2.5 border-b border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 flex items-center justify-between font-semibold text-slate-700 dark:text-slate-200">
        <div className="flex items-center gap-1.5">
          <MousePointer2 className="w-4 h-4 text-amber-500" />
          <span>{s0.title}</span>
        </div>
        <PaneCollapseButton
          collapseLabel={s0.collapsePanel}
          onCollapse={() => togglePaneCollapsed('selection')}
          testId="selection-pane-collapse"
        />
      </div>

      <div className="flex-1 overflow-y-auto p-2 space-y-0.5">
        {visuals.length === 0 ? (
          <div className="p-4 text-center text-slate-400 dark:text-slate-500 text-[11px]">{s0.empty}</div>
        ) : (
          visuals.map(v => {
            const hidden = v.formatConfig?.hidden ?? false;
            return (
              <div
                key={v.id}
                onClick={() => setSelectedVisualId(v.id)}
                data-testid={`selection-row-${v.id}`}
                className={`flex items-center gap-1.5 px-1.5 py-1 rounded cursor-pointer text-[11px] ${
                  selectedVisualId === v.id
                    ? 'bg-amber-100 dark:bg-amber-950/60 text-amber-900 dark:text-amber-300 font-semibold'
                    : 'hover:bg-slate-100 dark:hover:bg-slate-800'
                }`}
              >
                <span className={`flex-1 min-w-0 truncate ${hidden ? 'opacity-50 italic' : ''}`}>{v.title || v.type}</span>
                <button
                  type="button"
                  title={hidden ? s0.showVisual : s0.hideVisual}
                  aria-label={`${hidden ? s0.showVisual : s0.hideVisual}: ${v.title}`}
                  onClick={e => {
                    e.stopPropagation();
                    updateVisual(v.id, { formatConfig: { ...v.formatConfig, hidden: !hidden } });
                  }}
                  className="p-0.5 rounded hover:bg-slate-200 dark:hover:bg-slate-700"
                >
                  {hidden ? <EyeOff className="w-3.5 h-3.5 text-slate-400" /> : <Eye className="w-3.5 h-3.5 text-slate-600 dark:text-slate-300" />}
                </button>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
});
