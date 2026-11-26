/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 *
 * Seção "Ação do botão" do painel de formato do actionButton: escolhe o
 * actionType (Voltar / Navegação de página / Indicador) e, quando é um
 * indicador, qual Bookmark aplicar (actionBookmarkId).
 */

import React from 'react';
import { Bookmark, Page, VisualFormatConfig } from '../../../types/bi';

export const ActionButtonBookmarkConfig: React.FC<{
  formatConfig: VisualFormatConfig;
  pages: Page[];
  bookmarks: Bookmark[];
  onChange: (patch: Partial<VisualFormatConfig>) => void;
}> = ({ formatConfig, pages, bookmarks, onChange }) => {
  const actionType = formatConfig.actionType ?? 'back';

  return (
    <div className="space-y-2 border-t border-slate-100 dark:border-slate-800 pt-2">
      <label className="text-[11px] font-semibold text-slate-700 dark:text-slate-300 block">Ação do botão</label>
      <select
        aria-label="Tipo de ação"
        value={actionType}
        onChange={e => onChange({ actionType: e.target.value as VisualFormatConfig['actionType'] })}
        className="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded p-1 text-[11px] text-slate-800 dark:text-slate-100"
      >
        <option value="back">Voltar</option>
        <option value="page-navigation">Navegação de página</option>
        <option value="bookmark">Indicador</option>
      </select>

      {actionType === 'page-navigation' && (
        <select
          aria-label="Página de destino"
          value={formatConfig.actionTargetPageId ?? ''}
          onChange={e => onChange({ actionTargetPageId: e.target.value || undefined })}
          className="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded p-1 text-[11px] text-slate-800 dark:text-slate-100"
        >
          <option value="">(Selecione a página)</option>
          {pages.map(p => (
            <option key={p.id} value={p.id}>{p.name}</option>
          ))}
        </select>
      )}

      {actionType === 'bookmark' && (
        <select
          aria-label="Indicador"
          value={formatConfig.actionBookmarkId ?? ''}
          onChange={e => onChange({ actionBookmarkId: e.target.value || undefined })}
          className="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded p-1 text-[11px] text-slate-800 dark:text-slate-100"
        >
          <option value="">(Selecione o indicador)</option>
          {bookmarks.map(b => (
            <option key={b.id} value={b.id}>{b.name}</option>
          ))}
        </select>
      )}
    </div>
  );
};
