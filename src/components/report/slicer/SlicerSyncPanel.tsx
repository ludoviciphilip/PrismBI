import React from 'react';
import { Page, Visual } from '../../../types/bi';
import { useTranslation } from '../../../i18n';

/**
 * Format-pane "Sincronizar segmentações" section (mirrors Power BI's
 * "Sync slicers" pane): a sync group name, plus a checkbox per report page
 * controlling which pages receive this slicer's selection. Empty/no pages
 * checked means "all pages" (see `pageInSyncScope`).
 */
export const SlicerSyncPanel: React.FC<{
  visual: Visual;
  pages: Page[];
  onChange: (patch: { syncGroup?: string; syncPages?: string[] }) => void;
}> = ({ visual, pages, onChange }) => {
  const { t } = useTranslation();
  const syncGroup = visual.dataConfig.slicerConfig?.syncGroup || '';
  const syncPages = visual.dataConfig.slicerConfig?.syncPages || [];
  const allPagesMode = syncPages.length === 0;

  const togglePage = (pageId: string) => {
    const next = syncPages.includes(pageId) ? syncPages.filter(p => p !== pageId) : [...syncPages, pageId];
    onChange({ syncPages: next });
  };

  return (
    <div className="space-y-1.5 border-t border-slate-100 pt-2" data-testid="slicer-sync-panel">
      <label className="text-[11px] font-semibold text-slate-700 block">{t.slicer.syncSlicersTitle}</label>
      <div>
        <label className="text-[10px] text-slate-500 dark:text-slate-400 font-semibold block mb-0.5">{t.slicer.syncGroup}</label>
        <input
          type="text"
          value={syncGroup}
          placeholder={t.slicer.syncGroupPlaceholder}
          onChange={e => onChange({ syncGroup: e.target.value })}
          className="w-full min-w-0 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-[11px] text-slate-800 dark:text-slate-100 placeholder-slate-400"
        />
      </div>
      {syncGroup && (
        <div>
          <label className="text-[10px] text-slate-500 dark:text-slate-400 font-semibold block mb-0.5">{t.slicer.syncPages}</label>
          <label className="flex items-center gap-1.5 text-[11px] text-slate-600 dark:text-slate-300 px-0.5 py-0.5">
            <input type="checkbox" checked={allPagesMode} onChange={() => onChange({ syncPages: [] })} className="accent-amber-500 w-3 h-3" />
            {t.slicer.syncAllPages}
          </label>
          <ul className="max-h-24 overflow-y-auto">
            {pages.map(p => (
              <li key={p.id}>
                <label className="flex items-center gap-1.5 text-[11px] text-slate-600 dark:text-slate-300 px-0.5 py-0.5">
                  <input
                    type="checkbox"
                    checked={!allPagesMode && syncPages.includes(p.id)}
                    onChange={() => togglePage(p.id)}
                    className="accent-amber-500 w-3 h-3"
                  />
                  {p.name}
                </label>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
};
