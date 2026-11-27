import React from 'react';
import { Page, Visual } from '../../../types/bi';
import { getTooltipPages } from './resolve';
import { useTranslation } from '../../../i18n';

interface TooltipCardProps {
  visual: Visual;
  pages: Page[];
  onChange: (tooltip: NonNullable<Visual['formatConfig']['tooltip']>) => void;
}

/**
 * Cartão "Dica de ferramenta" do painel de formato do visual: tipo Padrão /
 * Página do relatório, e a página (ou Automático) quando for Página.
 */
export const TooltipCard: React.FC<TooltipCardProps> = ({ visual, pages, onChange }) => {
  const { t } = useTranslation();
  const tt = t.pageTooltip;
  const tooltip = visual.formatConfig.tooltip || { type: 'default' as const };
  const tooltipPages = getTooltipPages(pages);

  return (
    <div className="space-y-2 border-t border-slate-100 dark:border-slate-800 pt-2" data-testid="tooltip-card">
      <label className="text-[11px] font-semibold text-slate-700 dark:text-slate-300 block">{tt.tooltipLabel}</label>

      <div className="flex items-center justify-between">
        <span className="text-[11px] text-slate-600 dark:text-slate-300">{tt.type}</span>
        <select
          data-testid="tooltip-type-select"
          value={tooltip.type}
          onChange={(e) => {
            const type = e.target.value as 'default' | 'page';
            onChange(type === 'default' ? { type } : { type, pageId: tooltip.pageId || 'auto' });
          }}
          className="bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-0.5 text-[11px] text-slate-700 dark:text-slate-200"
        >
          <option value="default">{tt.typeDefault}</option>
          <option value="page">{tt.typeReportPage}</option>
        </select>
      </div>

      {tooltip.type === 'page' && (
        <div className="flex items-center justify-between">
          <span className="text-[11px] text-slate-600 dark:text-slate-300">{tt.pageLabel}</span>
          <select
            data-testid="tooltip-page-select"
            value={tooltip.pageId || 'auto'}
            onChange={(e) => onChange({ type: 'page', pageId: e.target.value })}
            className="bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-0.5 text-[11px] text-slate-700 dark:text-slate-200 max-w-[140px]"
          >
            <option value="auto">{tt.automatic}</option>
            {tooltipPages.map(p => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
        </div>
      )}

      {tooltip.type === 'page' && tooltipPages.length === 0 && (
        <p className="text-[10px] text-amber-600 dark:text-amber-400">
          {tt.noTooltipPagesYet}
        </p>
      )}
    </div>
  );
};
