/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 *
 * Painel "Indicadores" (Bookmarks), aberto pela guia Exibir — paridade com
 * o Power BI Desktop: adicionar/atualizar/renomear/excluir/duplicar,
 * arrastar para reordenar, grupos, modo "Visualizar" e o menu "..." por
 * item (Dados / Exibição / Página atual / Todos os visuais ↔ Visuais
 * selecionados).
 */

import React, { useMemo, useState } from 'react';
import {
  Bookmark as BookmarkIcon,
  Plus,
  RefreshCw,
  Pencil,
  Trash2,
  Copy,
  MoreVertical,
  Play,
  X,
  ChevronLeft,
  ChevronRight,
  FolderPlus,
  FolderMinus,
  GripVertical,
} from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { useUiPrefs } from '../../store/useUiPrefs';
import { useTranslation } from '../../i18n';
import { CollapsedPaneStrip, PaneCollapseButton } from './PaneCollapseShell';
import { sortBookmarks } from '../../store/bookmarks';
import { Bookmark } from '../../types/bi';

const BookmarkMenu: React.FC<{
  bookmark: Bookmark;
  onClose: () => void;
}> = ({ bookmark, onClose }) => {
  const { t } = useTranslation();
  const b = t.bookmarks;
  const { refreshBookmark, renameBookmark, deleteBookmark, duplicateBookmark, updateBookmarkSettings } = useBiStore(
    useShallow(s => ({
      refreshBookmark: s.refreshBookmark,
      renameBookmark: s.renameBookmark,
      deleteBookmark: s.deleteBookmark,
      duplicateBookmark: s.duplicateBookmark,
      updateBookmarkSettings: s.updateBookmarkSettings,
    }))
  );
  const capture = bookmark.capture ?? { data: true, display: true, currentPage: true };
  const scopeIsAll = !Array.isArray(bookmark.scope);

  return (
    <div
      role="menu"
      aria-label={b.optionsOf.replace('{name}', bookmark.name)}
      className="absolute right-0 top-6 z-20 w-56 bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded shadow-lg text-[11px] text-slate-700 dark:text-slate-200 py-1"
      onClick={e => e.stopPropagation()}
    >
      <button
        type="button"
        className="w-full flex items-center gap-1.5 px-2 py-1 hover:bg-amber-50 dark:hover:bg-slate-700"
        onClick={() => { refreshBookmark(bookmark.id); onClose(); }}
      >
        <RefreshCw className="w-3 h-3" /> {b.refresh}
      </button>
      <button
        type="button"
        className="w-full flex items-center gap-1.5 px-2 py-1 hover:bg-amber-50 dark:hover:bg-slate-700"
        onClick={() => { duplicateBookmark(bookmark.id); onClose(); }}
      >
        <Copy className="w-3 h-3" /> {b.duplicate}
      </button>
      <button
        type="button"
        className="w-full flex items-center gap-1.5 px-2 py-1 text-red-600 hover:bg-red-50 dark:hover:bg-red-950/40"
        onClick={() => { deleteBookmark(bookmark.id); onClose(); }}
      >
        <Trash2 className="w-3 h-3" /> {b.delete}
      </button>
      <div className="border-t border-slate-100 dark:border-slate-700 my-1" />
      <label className="w-full flex items-center gap-1.5 px-2 py-1 cursor-pointer">
        <input
          type="checkbox"
          checked={capture.data}
          onChange={e => updateBookmarkSettings(bookmark.id, { capture: { ...capture, data: e.target.checked } })}
          className="accent-amber-500"
        />
        {b.captureData}
      </label>
      <label className="w-full flex items-center gap-1.5 px-2 py-1 cursor-pointer">
        <input
          type="checkbox"
          checked={capture.display}
          onChange={e => updateBookmarkSettings(bookmark.id, { capture: { ...capture, display: e.target.checked } })}
          className="accent-amber-500"
        />
        {b.captureDisplay}
      </label>
      <label className="w-full flex items-center gap-1.5 px-2 py-1 cursor-pointer">
        <input
          type="checkbox"
          checked={capture.currentPage}
          onChange={e => updateBookmarkSettings(bookmark.id, { capture: { ...capture, currentPage: e.target.checked } })}
          className="accent-amber-500"
        />
        {b.captureCurrentPage}
      </label>
      <div className="border-t border-slate-100 dark:border-slate-700 my-1" />
      <label className="w-full flex items-center gap-1.5 px-2 py-1 cursor-pointer">
        <input
          type="radio"
          name={`scope-${bookmark.id}`}
          checked={scopeIsAll}
          onChange={() => updateBookmarkSettings(bookmark.id, { scope: 'all' })}
          className="accent-amber-500"
        />
        {b.scopeAllVisuals}
      </label>
      <label className="w-full flex items-center gap-1.5 px-2 py-1 cursor-pointer">
        <input
          type="radio"
          name={`scope-${bookmark.id}`}
          checked={!scopeIsAll}
          onChange={() => {
            const selected = useBiStore.getState().selectedVisualIds;
            updateBookmarkSettings(bookmark.id, { scope: selected.length > 0 ? [...selected] : [] });
          }}
          className="accent-amber-500"
        />
        {b.scopeSelectedVisuals}
      </label>
    </div>
  );
};

const BookmarkRow: React.FC<{
  bookmark: Bookmark;
  isActiveInView: boolean;
  onDragStart: (id: string) => void;
  onDragOver: (id: string) => void;
}> = ({ bookmark, isActiveInView, onDragStart, onDragOver }) => {
  const { t } = useTranslation();
  const b = t.bookmarks;
  const { applyBookmark, renameBookmark } = useBiStore(useShallow(s => ({
    applyBookmark: s.applyBookmark,
    renameBookmark: s.renameBookmark,
  })));
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(bookmark.name);
  const [menuOpen, setMenuOpen] = useState(false);

  const commitRename = () => {
    const name = draft.trim();
    if (name) renameBookmark(bookmark.id, name);
    setEditing(false);
  };

  return (
    <div
      draggable
      onDragStart={() => onDragStart(bookmark.id)}
      onDragOver={e => { e.preventDefault(); onDragOver(bookmark.id); }}
      onClick={() => !editing && applyBookmark(bookmark.id)}
      data-testid={`bookmark-row-${bookmark.id}`}
      className={`relative group flex items-center gap-1.5 px-1.5 py-1 rounded cursor-pointer text-[11px] ${
        isActiveInView
          ? 'bg-amber-100 dark:bg-amber-950/60 text-amber-900 dark:text-amber-300 font-semibold'
          : 'hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200'
      }`}
    >
      <GripVertical className="w-3 h-3 text-slate-400 shrink-0 cursor-grab" />
      {editing ? (
        <input
          autoFocus
          value={draft}
          aria-label={b.renameAria}
          onChange={e => setDraft(e.target.value)}
          onBlur={commitRename}
          onKeyDown={e => {
            if (e.key === 'Enter') commitRename();
            if (e.key === 'Escape') setEditing(false);
          }}
          onClick={e => e.stopPropagation()}
          className="flex-1 min-w-0 bg-white dark:bg-slate-900 border border-amber-400 rounded px-1 py-0.5 text-[11px] text-slate-800 dark:text-slate-100"
        />
      ) : (
        <span
          className="flex-1 min-w-0 truncate"
          onDoubleClick={e => { e.stopPropagation(); setEditing(true); }}
        >
          {bookmark.name}
        </span>
      )}
      <button
        type="button"
        title={b.moreOptions}
        aria-label={`${b.moreOptions}: ${bookmark.name}`}
        onClick={e => { e.stopPropagation(); setMenuOpen(v => !v); }}
        className="p-0.5 rounded hover:bg-slate-200 dark:hover:bg-slate-700 opacity-0 group-hover:opacity-100 focus:opacity-100"
      >
        <MoreVertical className="w-3 h-3" />
      </button>
      {menuOpen && <BookmarkMenu bookmark={bookmark} onClose={() => setMenuOpen(false)} />}
    </div>
  );
};

export const BookmarksPane: React.FC = React.memo(() => {
  const { t } = useTranslation();
  const b = t.bookmarks;
  const paneState = useUiPrefs(s => s.panes.bookmarks);
  const togglePaneCollapsed = useUiPrefs(s => s.togglePaneCollapsed);
  const { bookmarks, addBookmark, reorderBookmark, groupBookmarks, ungroupBookmarks } = useBiStore(
    useShallow(s => ({
      bookmarks: s.bookmarks,
      addBookmark: s.addBookmark,
      reorderBookmark: s.reorderBookmark,
      groupBookmarks: s.groupBookmarks,
      ungroupBookmarks: s.ungroupBookmarks,
    }))
  );
  const [dragId, setDragId] = useState<string | null>(null);
  const [viewing, setViewing] = useState(false);
  const [viewIndex, setViewIndex] = useState(0);
  const applyBookmark = useBiStore(s => s.applyBookmark);

  const sorted = useMemo(() => sortBookmarks(bookmarks), [bookmarks]);
  const groups = useMemo(() => {
    const byGroup = new Map<string | undefined, Bookmark[]>();
    for (const bm of sorted) {
      const key = bm.groupId;
      byGroup.set(key, [...(byGroup.get(key) ?? []), bm]);
    }
    return byGroup;
  }, [sorted]);

  if (paneState === 'collapsed') {
    return (
      <CollapsedPaneStrip
        icon={BookmarkIcon}
        iconClassName="w-4 h-4 text-amber-500"
        title={b.title}
        expandLabel={b.expandPanel}
        onExpand={() => togglePaneCollapsed('bookmarks')}
        testId="bookmarks-pane-collapsed"
      />
    );
  }

  const enterView = () => {
    if (sorted.length === 0) return;
    setViewing(true);
    setViewIndex(0);
    applyBookmark(sorted[0].id);
  };

  const stepView = (delta: number) => {
    if (sorted.length === 0) return;
    const next = (viewIndex + delta + sorted.length) % sorted.length;
    setViewIndex(next);
    applyBookmark(sorted[next].id);
  };

  return (
    <div className="w-64 border-l border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 flex flex-col h-full text-xs select-none text-slate-800 dark:text-slate-200">
      <div className="p-2.5 border-b border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 flex items-center justify-between font-semibold text-slate-700 dark:text-slate-200">
        <div className="flex items-center gap-1.5">
          <BookmarkIcon className="w-4 h-4 text-amber-500" />
          <span>{b.title}</span>
        </div>
        <PaneCollapseButton
          collapseLabel={b.collapsePanel}
          onCollapse={() => togglePaneCollapsed('bookmarks')}
          testId="bookmarks-pane-collapse"
        />
      </div>

      <div className="flex items-center gap-1 px-2 py-1.5 border-b border-slate-200 dark:border-slate-800">
        <button
          type="button"
          onClick={() => addBookmark(`${b.newBookmarkPrefix} ${bookmarks.length + 1}`)}
          className="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-[11px] font-medium"
          title={b.add}
        >
          <Plus className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" /> {b.add}
        </button>
        <button
          type="button"
          onClick={() => groupBookmarks(sorted.map(bm => bm.id), b.newGroupName)}
          className="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-[11px]"
          title={b.groupAll}
          disabled={sorted.length === 0}
        >
          <FolderPlus className="w-3.5 h-3.5" />
        </button>
        <button
          type="button"
          onClick={() => ungroupBookmarks(sorted.map(bm => bm.id))}
          className="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-[11px]"
          title={b.ungroupAll}
          disabled={sorted.length === 0}
        >
          <FolderMinus className="w-3.5 h-3.5" />
        </button>
        <button
          type="button"
          onClick={enterView}
          className="ml-auto flex items-center gap-1 px-1.5 py-1 rounded bg-amber-500 hover:bg-amber-600 text-slate-950 font-semibold text-[11px]"
          title={b.play}
          disabled={sorted.length === 0}
        >
          <Play className="w-3 h-3" /> {b.play}
        </button>
      </div>

      {viewing && (
        <div className="flex items-center justify-between gap-1 px-2 py-1.5 bg-amber-50 dark:bg-amber-950/30 border-b border-amber-200 dark:border-amber-900/50 text-[11px]">
          <button type="button" onClick={() => stepView(-1)} title={b.previous} className="p-1 rounded hover:bg-amber-100 dark:hover:bg-amber-900/40">
            <ChevronLeft className="w-3.5 h-3.5" />
          </button>
          <span className="truncate font-semibold text-amber-900 dark:text-amber-300">{sorted[viewIndex]?.name}</span>
          <button type="button" onClick={() => stepView(1)} title={b.next} className="p-1 rounded hover:bg-amber-100 dark:hover:bg-amber-900/40">
            <ChevronRight className="w-3.5 h-3.5" />
          </button>
          <button type="button" onClick={() => setViewing(false)} title={b.exit} className="p-1 rounded hover:bg-amber-100 dark:hover:bg-amber-900/40">
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      <div className="flex-1 overflow-y-auto p-2 space-y-1">
        {sorted.length === 0 ? (
          <div className="p-4 text-center text-slate-400 dark:text-slate-500">
            <BookmarkIcon className="w-6 h-6 mb-2 mx-auto text-slate-300 dark:text-slate-600" />
            <p className="text-[11px]">{b.empty}</p>
          </div>
        ) : (
          [...groups.entries()].map(([groupId, items]) => (
            <div key={groupId ?? '__ungrouped__'} className="space-y-0.5">
              {groupId && (
                <div className="text-[10px] font-bold uppercase tracking-wider text-slate-400 dark:text-slate-500 px-1 pt-1">
                  {groupId}
                </div>
              )}
              {items.map(bm => (
                <BookmarkRow
                  key={bm.id}
                  bookmark={bm}
                  isActiveInView={viewing && sorted[viewIndex]?.id === bm.id}
                  onDragStart={id => setDragId(id)}
                  onDragOver={overId => {
                    if (dragId && dragId !== overId) {
                      reorderBookmark(dragId, overId);
                    }
                  }}
                />
              ))}
            </div>
          ))
        )}
      </div>
    </div>
  );
});
