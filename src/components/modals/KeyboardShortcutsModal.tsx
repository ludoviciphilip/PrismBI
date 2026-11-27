/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useMemo, useState } from 'react';
import { X, Search, Keyboard } from 'lucide-react';
import { useBiStore } from '../../store/useBiStore';
import { useTranslation } from '../../i18n';
import {
  SHORTCUTS,
  SHORTCUT_CATEGORY_LABEL,
  formatCombos,
  isMacPlatform,
} from '../../services/shortcuts/registry';

/**
 * Diálogo "Atalhos de teclado" (Ctrl+/ ou ?). Gerado 100% a partir do
 * registro central (`src/services/shortcuts/registry.ts`) para nunca
 * ficar desatualizado — qualquer atalho novo adicionado lá aparece aqui
 * automaticamente, com busca por nome ou combinação de teclas.
 */
export const KeyboardShortcutsModal: React.FC = () => {
  const open = useBiStore(s => s.shortcutsModalOpen);
  const setOpen = useBiStore(s => s.setShortcutsModalOpen);
  const { t, language } = useTranslation();
  const [search, setSearch] = useState('');
  const isMac = isMacPlatform();

  const groups = useMemo(() => {
    const term = search.trim().toLowerCase();
    const bucket = new Map<string, { id: string; label: string; combo: string }[]>();

    for (const def of SHORTCUTS) {
      const label = def.label[language] || def.label['pt-BR'];
      const combo = formatCombos(def.combos, isMac);
      if (term) {
        const haystack = `${label} ${combo}`.toLowerCase();
        if (!haystack.includes(term)) continue;
      }
      const categoryLabel = SHORTCUT_CATEGORY_LABEL[def.category]?.[language] || def.category;
      const list = bucket.get(categoryLabel) || [];
      list.push({ id: def.id, label, combo });
      bucket.set(categoryLabel, list);
    }

    return Array.from(bucket.entries());
  }, [search, language, isMac]);

  const totalResults = groups.reduce((acc, [, items]) => acc + items.length, 0);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 bg-black/60 z-[10001] flex items-center justify-center p-4"
      onClick={() => setOpen(false)}
    >
      <div
        className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-2xl max-w-2xl w-full max-h-[85vh] flex flex-col overflow-hidden animate-in fade-in zoom-in-95 duration-150"
        onClick={e => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-100 dark:border-slate-800">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-xl bg-amber-100 dark:bg-amber-950/60 text-amber-700 dark:text-amber-400">
              <Keyboard className="w-5 h-5" />
            </div>
            <div>
              <h3 className="font-bold text-slate-900 dark:text-white text-base">{t.shortcuts.dialogTitle}</h3>
              <p className="text-xs text-slate-500 dark:text-slate-400">{t.shortcuts.dialogSubtitle}</p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => setOpen(false)}
            className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 cursor-pointer"
            aria-label={t.common.close}
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="px-5 py-3 border-b border-slate-100 dark:border-slate-800">
          <div className="relative">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
            <input
              autoFocus
              type="text"
              value={search}
              onChange={e => setSearch(e.target.value)}
              placeholder={t.shortcuts.searchPlaceholder}
              data-testid="shortcuts-search-input"
              className="w-full pl-9 pr-3 py-2 text-sm rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-amber-400"
            />
          </div>
        </div>

        <div className="overflow-y-auto px-5 py-4 space-y-5" data-testid="shortcuts-list">
          {totalResults === 0 && (
            <p className="text-sm text-slate-500 dark:text-slate-400 text-center py-6">{t.shortcuts.noResults}</p>
          )}
          {groups.map(([categoryLabel, items]) => (
            <div key={categoryLabel}>
              <h4 className="text-[11px] font-bold uppercase tracking-wide text-slate-500 dark:text-slate-400 mb-1.5">
                {categoryLabel}
              </h4>
              <table className="w-full text-sm">
                <tbody>
                  {items.map(item => (
                    <tr key={item.id} className="border-b border-slate-100 dark:border-slate-800 last:border-0">
                      <td className="py-1.5 text-slate-700 dark:text-slate-200">{item.label}</td>
                      <td className="py-1.5 text-right">
                        <kbd className="px-1.5 py-0.5 rounded bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 font-mono text-[11px]">
                          {item.combo}
                        </kbd>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
