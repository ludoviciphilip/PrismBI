/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 *
 * Operações puras (testáveis sem React/zustand) sobre a lista de
 * indicadores (Bookmarks) do painel "Indicadores": criar, renomear, excluir,
 * duplicar, reordenar (arrastar) e agrupar/desagrupar.
 *
 * Grupos são modelados como uma tag de texto livre compartilhada
 * (`Bookmark.groupId`): indicadores com o mesmo `groupId` aparecem sob o
 * mesmo cabeçalho de grupo no painel. Não há um registro de grupos separado
 * a persistir — agrupar/desagrupar só lê e grava esse campo.
 */

import { Bookmark } from '../types/bi';
import { BookmarkCapture, BookmarkScope } from '../components/report/bookmarks/capture';

/** Próximo valor de `order` (o maior atual + 1; começa em 0). */
export function nextBookmarkOrder(bookmarks: Bookmark[]): number {
  if (bookmarks.length === 0) return 0;
  return Math.max(...bookmarks.map(b => b.order ?? 0)) + 1;
}

/** Ordena a lista pela posição manual (`order`), como aparece no painel. */
export function sortBookmarks(bookmarks: Bookmark[]): Bookmark[] {
  return [...bookmarks].sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
}

export function createBookmarkId(): string {
  return 'bm_' + Math.random().toString(36).slice(2, 8) + Date.now().toString(36).slice(-4);
}

/** Monta um novo Bookmark a partir dos campos capturados, com id/nome/ordem. */
export function createBookmarkEntry(
  name: string,
  captured: Pick<Bookmark, 'activePageId' | 'filters' | 'visualState' | 'slicerSelections' | 'scope' | 'capture'>,
  existing: Bookmark[]
): Bookmark {
  return {
    id: createBookmarkId(),
    name,
    order: nextBookmarkOrder(existing),
    ...captured,
  };
}

export function renameBookmarkEntry(bookmarks: Bookmark[], id: string, name: string): Bookmark[] {
  return bookmarks.map(b => (b.id === id ? { ...b, name } : b));
}

export function deleteBookmarkEntry(bookmarks: Bookmark[], id: string): Bookmark[] {
  return bookmarks.filter(b => b.id !== id);
}

/** Duplica um indicador (Power BI: "<Nome> - Cópia"), inserido logo após o original na ordem. */
export function duplicateBookmarkEntry(bookmarks: Bookmark[], id: string): Bookmark[] {
  const source = bookmarks.find(b => b.id === id);
  if (!source) return bookmarks;
  const copy: Bookmark = {
    ...source,
    id: createBookmarkId(),
    name: `${source.name} - Cópia`,
  };
  const sorted = sortBookmarks(bookmarks);
  const idx = sorted.findIndex(b => b.id === id);
  const withCopy = [...sorted.slice(0, idx + 1), copy, ...sorted.slice(idx + 1)];
  return withCopy.map((b, i) => ({ ...b, order: i }));
}

/**
 * Move o indicador `id` para a posição de `overId` (arrastar para reordenar).
 * Reatribui `order` sequencialmente (0..n-1) para toda a lista.
 */
export function reorderBookmarkEntry(bookmarks: Bookmark[], id: string, overId: string): Bookmark[] {
  if (id === overId) return bookmarks;
  const sorted = sortBookmarks(bookmarks);
  const fromIdx = sorted.findIndex(b => b.id === id);
  const toIdx = sorted.findIndex(b => b.id === overId);
  if (fromIdx === -1 || toIdx === -1) return bookmarks;
  const next = [...sorted];
  const [moved] = next.splice(fromIdx, 1);
  next.splice(toIdx, 0, moved);
  return next.map((b, i) => ({ ...b, order: i }));
}

/** Agrupa os indicadores `ids` sob `groupId` (nome do grupo, exibido no painel). */
export function setBookmarkGroup(bookmarks: Bookmark[], ids: string[], groupId: string): Bookmark[] {
  const idSet = new Set(ids);
  return bookmarks.map(b => (idSet.has(b.id) ? { ...b, groupId } : b));
}

/** Remove o grupo dos indicadores `ids` (Power BI: "Desagrupar"). */
export function ungroupBookmarkEntries(bookmarks: Bookmark[], ids: string[]): Bookmark[] {
  const idSet = new Set(ids);
  return bookmarks.map(b => {
    if (!idSet.has(b.id)) return b;
    const { groupId, ...rest } = b;
    return rest;
  });
}

/** Atualiza só `capture`/`scope` de um indicador (sem recapturar); "Atualizar" recaptura depois. */
export function updateBookmarkSettings(
  bookmarks: Bookmark[],
  id: string,
  patch: Partial<{ capture: BookmarkCapture; scope: BookmarkScope }>
): Bookmark[] {
  return bookmarks.map(b => (b.id === id ? { ...b, ...patch } : b));
}
